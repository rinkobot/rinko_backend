use anyhow::{Context, Result};
use chrono::Local;
use headless_chrome::{Browser, LaunchOptions};
use tokio::fs;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

use super::dx_world_parser::{DxWorldParser, DxWorldTimeline};

const DX_WORLD_URL: &str = "https://www.hamradiotimeline.com/timeline/dxw_timeline_1_1.php";
const DEFAULT_SAVE_DIR: &str = "data/dx_world";
const IMAGE_CACHE_DIR: &str = "data/image_cache";

/// DX World Scraper
pub struct DxWorldScraper {
    file_save_dir: PathBuf,
    image_cache_dir: PathBuf,
}

impl DxWorldScraper {
    /// Create a new scraper instance
    pub fn new(save_dir: Option<PathBuf>, image_cache_dir: Option<PathBuf>) -> Self {
        let file_save_dir = save_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_SAVE_DIR));
        let image_cache_dir = image_cache_dir.unwrap_or_else(|| PathBuf::from(IMAGE_CACHE_DIR));

        Self {
            file_save_dir,
            image_cache_dir,
        }
    }

    /// Ensure the save directory exists
    async fn ensure_save_dir(&self) -> Result<()> {
        if !self.file_save_dir.exists() {
            fs::create_dir_all(&self.file_save_dir).await
                .with_context(|| format!("Failed to create directory: {:?}", self.file_save_dir))?;
            info!("Created save directory: {:?}", self.file_save_dir);
        }
        if !self.image_cache_dir.exists() {
            fs::create_dir_all(&self.image_cache_dir).await
                .with_context(|| format!("Failed to create image cache directory: {:?}", self.image_cache_dir))?;
            info!("Created image cache directory: {:?}", self.image_cache_dir);
        }
        Ok(())
    }

    /// Fetch the page and save as screenshot
    /// 
    /// # Returns
    /// Returns the saved file paths (screenshot path) on success, Error on failure
    pub async fn fetch_and_save(&self) -> Result<()> {
        self.ensure_save_dir().await?;

        info!("Starting DX World page fetch from: {}", DX_WORLD_URL);

        // Configure headless Chrome launch options
        let launch_options = LaunchOptions {
            headless: true,
            sandbox: false,
            ..Default::default()
        };

        // Launch the browser
        let browser = Browser::new(launch_options)
            .context("Failed to launch headless browser")?;

        // Open a new tab
        let tab = browser.new_tab()
            .context("Failed to create new tab")?;

        // Navigate to the target URL and wait for it to load
        info!("Navigating to: {}", DX_WORLD_URL);
        tab.navigate_to(DX_WORLD_URL)
            .context("Failed to navigate to URL")?;

        // Wait for the page to finish loading
        tab.wait_until_navigated()
            .context("Failed to wait for page navigation")?;

        // Additional wait to ensure dynamic content is loaded
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Get HTML content
        let html_content = tab.get_content()
            .context("Failed to get page content")?;

        let timestamp = Local::now().format("%Y%m%d_%H%M%S");

        let html_filename = format!("dxw_timeline_{}.html", timestamp);
        let html_path = self.file_save_dir.join(&html_filename);
        fs::write(&html_path, html_content).await
            .with_context(|| format!("Failed to write HTML to: {:?}", html_path))?;

        let screenshot_filename = format!("dxw_timeline_{}.png", timestamp);
        let screenshot_path = self.file_save_dir.join(&screenshot_filename);
        
        match tab.capture_screenshot(
            headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
            None,
            None,
            true,
        ) {
            Ok(screenshot_data) => {
                fs::write(&screenshot_path, &screenshot_data).await
                    .with_context(|| format!("Failed to write screenshot to: {:?}", screenshot_path))?;
                info!("Saved screenshot to: {:?} ({} bytes)", screenshot_path, screenshot_data.len());
            }
            Err(e) => {
                warn!("Failed to capture screenshot: {}", e);
            }
        };

        // copy latest screenshot to "dxw_latest.png" for easy access
        let latest_path = self.image_cache_dir.join("dxw_latest.png");
        if let Err(e) = fs::copy(&screenshot_path, &latest_path).await {
            warn!("Failed to copy latest screenshot to {:?}: {}", latest_path, e);
        } else {
            info!("Copied latest screenshot to {:?}", latest_path);
        }

        Ok(())
    }

    /// Fetch, save and parse the DX World timeline
    /// 
    /// # Returns
    /// Returns the parsed timeline data
    pub async fn fetch_and_parse(&self) -> Result<DxWorldTimeline> {
        // First, fetch and save the page
        self.fetch_and_save().await?;

        // Find the most recent HTML file
        let html_files = self.find_html_files().await?;
        
        if html_files.is_empty() {
            anyhow::bail!("No HTML files found in {:?}", self.file_save_dir);
        }

        // Parse the most recent file
        let latest_html = &html_files[0];
        info!("Parsing latest HTML file: {:?}", latest_html);
        
        let timeline = DxWorldParser::parse_file(latest_html).await?;

        // Save as JSON
        let json_path = latest_html.with_extension("json");
        DxWorldParser::save_as_json(&timeline, &json_path).await?;

        Ok(timeline)
    }

    /// Find all HTML files in the save directory, sorted by modification time (newest first)
    async fn find_html_files(&self) -> Result<Vec<PathBuf>> {
        let mut html_files = Vec::new();

        if !self.file_save_dir.exists() {
            return Ok(html_files);
        }

        let mut entries = fs::read_dir(&self.file_save_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "html") {
                html_files.push(path);
            }
        }

        // Sort by modification time (newest first)
        html_files.sort_by(|a, b| {
            let a_time = std::fs::metadata(a)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let b_time = std::fs::metadata(b)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            b_time.cmp(&a_time)
        });

        Ok(html_files)
    }
}

pub async fn cleanup_old_dx_world_files(file_save_dir: &PathBuf, days_to_keep: i64) -> Result<usize> {
    if !file_save_dir.exists() {
        return Ok(0);
    }

    let mut deleted_count = 0;
    let cutoff_time = chrono::Utc::now() - chrono::Duration::days(days_to_keep);

    let mut entries = fs::read_dir(file_save_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.is_file() && (path.extension().map_or(false, |ext| ext == "png") || path.extension().map_or(false, |ext| ext == "html")) {
            if let Ok(metadata) = entry.metadata().await {
                if let Ok(modified) = metadata.modified() {
                    let modified_time: chrono::DateTime<chrono::Utc> = modified.into();
                    if modified_time < cutoff_time {
                        if let Err(e) = fs::remove_file(&path).await {
                            warn!("Failed to delete old file {:?}: {}", path, e);
                        } else {
                            deleted_count += 1;
                            info!("Deleted old file: {:?}", path);
                        }
                    }
                }
            }
        }
    }

    if deleted_count > 0 {
        info!("Cleaned up {} old files from {:?}", deleted_count, file_save_dir);
    }

    Ok(deleted_count)
}