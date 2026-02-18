use crate::module::sat::SearchResult;
use super::manager::{AmsatSearchResult, SatelliteManager};

///! Satellite status renderer - Multi-transponder support
///!
///! Generate images from satellite data with transponder information

use super::types::{Transponder, ReportStatus, SatelliteDataBlock};
use anyhow::{Context, Result};
use chrono::{DateTime, Timelike, Utc};
use std::path::{Path, PathBuf};

// Reuse from V1
const SVG_TEMPLATE: &str = include_str!("../../../resources/sat_template.svg");
const BLOCK_TITLE_HEIGHT: f32 = 45.0;
const HEADER_HEIGHT: f32 = 40.0;
const ROW_HEIGHT: f32 = 38.0;
const BLOCK_SPACING: f32 = 30.0;
const TOP_PADDING: f32 = 20.0;
const FOOTER_HEIGHT: f32 = 32.0;
const MAX_REPORTS_PER_SATELLITE: usize = 5;
const TRANSPONDER_INFO_HEIGHT: f32 = 24.0;

const X_CALLSIGN: f32 = 20.0;
const X_GRIDS: f32 = 170.0;
const X_REPORT: f32 = 280.0;
const X_TIME: f32 = 540.0;
const COLOR_BLOCK_WIDTH: f32 = 12.0;
const COLOR_BLOCK_HEIGHT: f32 = 18.0;
const COLOR_BLOCK_TEXT_SPACING: f32 = 8.0;

/// Map time difference to color gradient
fn map_time_to_color(target_time: &str, now_utc: &DateTime<Utc>, min_hours: f64, max_hours: f64) -> Result<String> {
    let target = DateTime::parse_from_rfc3339(target_time)
        .context("Failed to parse target time")?;
    let target_utc = target.with_timezone(&Utc);

    let delta_hours = (now_utc.signed_duration_since(target_utc)).num_seconds().abs() as f64 / 3600.0;

    let green = (125u8, 227u8, 61u8);
    let yellow = (255u8, 255u8, 0u8);
    let red = (255u8, 0u8, 0u8);

    let (r, g, b) = if delta_hours <= min_hours {
        green
    } else if delta_hours >= max_hours {
        red
    } else {
        let mid = (min_hours + max_hours) / 2.0;
        if delta_hours <= mid {
            let t = (delta_hours - min_hours) / (mid - min_hours);
            lerp_color(green, yellow, t)
        } else {
            let t = (delta_hours - mid) / (max_hours - mid);
            lerp_color(yellow, red, t)
        }
    };

    Ok(format!("#{:02X}{:02X}{:02X}", r, g, b))
}

fn lerp_color(c1: (u8, u8, u8), c2: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let r = (c1.0 as f64 + (c2.0 as f64 - c1.0 as f64) * t).round() as u8;
    let g = (c1.1 as f64 + (c2.1 as f64 - c1.1 as f64) * t).round() as u8;
    let b = (c1.2 as f64 + (c2.2 as f64 - c1.2 as f64) * t).round() as u8;
    (r, g, b)
}

/// Satellite renderer
pub struct SatelliteRenderer {
    output_dir: PathBuf,
}

impl SatelliteRenderer {
    /// Create a new renderer
    pub fn new(output_dir: impl AsRef<Path>) -> Self {
        Self {
            output_dir: output_dir.as_ref().to_path_buf(),
        }
    }
    
    /// Render transponders to image (legacy API)
    pub async fn render_transponders(&self, search_results: &Vec<SearchResult>) -> Result<PathBuf> {
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .context("Failed to create output directory")?;
        
        let filename = self.generate_filename(search_results);
        let output_path = self.output_dir.join(&filename);
        
        if output_path.exists() {
            tracing::debug!("Using cached image: {:?}", output_path);
            return Ok(output_path);
        }
        
        self.generate_image(search_results, &output_path).await?;
        
        tracing::info!("Generated satellite status image: {:?}", output_path);
        
        Ok(output_path)
    }

    /// Render AMSAT search results to image (new dual-store API)
    pub async fn render_amsat_results(
        &self,
        results: &[AmsatSearchResult],
        manager: &SatelliteManager,
    ) -> Result<PathBuf> {
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .context("Failed to create output directory")?;

        let filename = self.generate_amsat_filename(results);
        let output_path = self.output_dir.join(&filename);

        if output_path.exists() {
            tracing::debug!("Using cached image: {:?}", output_path);
            return Ok(output_path);
        }

        let svg_content = self.generate_amsat_svg(results, manager)?;
        self.render_svg_to_png(&svg_content, &output_path).await?;

        tracing::info!("Generated satellite status image: {:?}", output_path);
        Ok(output_path)
    }

    /// Generate filename for AMSAT results
    fn generate_amsat_filename(&self, results: &[AmsatSearchResult]) -> String {
        let now = chrono::Utc::now();
        let minute = (now.minute() / 15) * 15;
        let floored = now
            .with_minute(minute)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();

        let time_str = floored.format("%Y%m%d_%H%M").to_string();

        let names: Vec<String> = results
            .iter()
            .take(5)
            .map(|r| Self::normalize_sat_name(&r.entry.api_name))
            .collect();

        let name_part = if names.is_empty() {
            "empty".to_string()
        } else if names.len() > 3 {
            format!("{}_and_{}_more", names[..2].join("_"), names.len() - 2)
        } else {
            names.join("_")
        };

        format!("sat_{}_{}.png", time_str, name_part)
    }

    /// Generate SVG for AMSAT results
    fn generate_amsat_svg(
        &self,
        results: &[AmsatSearchResult],
        manager: &SatelliteManager,
    ) -> Result<String> {
        let mut current_y = TOP_PADDING;
        let mut content = String::new();
        let now_utc = Utc::now();

        if results.is_empty() {
            content.push_str(
                r#"<text x="410" y="100" text-anchor="middle" class="table-text">No satellite data available.</text>"#,
            );
            current_y = 120.0;
        } else {
            for result in results {
                let metadata = manager.lookup_metadata(&result.entry);
                let block = self.generate_amsat_block(&result.entry, metadata.as_ref(), &mut current_y, &now_utc)?;
                content.push_str(&block);
            }
        }

        let footer = self.generate_footer(current_y);
        let total_height = current_y + FOOTER_HEIGHT;

        let svg = SVG_TEMPLATE
            .replace("{{SVG_HEIGHT}}", &total_height.to_string())
            .replace("{{CONTENT}}", &content)
            .replace("{{FOOTER}}", &footer);

        Ok(svg)
    }

    /// Generate a block for a single AMSAT entry
    fn generate_amsat_block(
        &self,
        entry: &super::amsat_types::AmsatEntry,
        metadata: Option<&super::manager::TransponderMetadata>,
        current_y: &mut f32,
        now_utc: &DateTime<Utc>,
    ) -> Result<String> {
        let mut block = String::new();

        // Title: API name (+ NORAD ID if known from metadata)
        let title = if let Some(meta) = metadata {
            format!("{} (NORAD {})", entry.api_name, meta.norad_id)
        } else {
            entry.api_name.clone()
        };
        block.push_str(&format!(
            r#"<text x="{}" y="{}" class="satellite-title">{}</text>"#,
            X_CALLSIGN,
            *current_y + BLOCK_TITLE_HEIGHT / 2.0,
            Self::escape_xml(&title)
        ));
        block.push('\n');
        *current_y += BLOCK_TITLE_HEIGHT;

        // Transponder info from metadata (if available)
        if let Some(meta) = metadata {
            block.push_str(&format!(
                r#"<text x="{}" y="{}" class="table-text" style="font-size:14px;fill:#666;">↑{} ↓{} | {}</text>"#,
                X_CALLSIGN,
                *current_y + TRANSPONDER_INFO_HEIGHT / 2.0,
                Self::escape_xml(&meta.uplink),
                Self::escape_xml(&meta.downlink),
                Self::escape_xml(&meta.mode)
            ));
            block.push('\n');
            *current_y += TRANSPONDER_INFO_HEIGHT;
        }

        // AMSAT update status
        let (status_class, status_text) = if entry.update_success {
            ("amsat-update-success", "AMSAT Update: Success")
        } else {
            ("amsat-update-failure", "AMSAT Update: Failed")
        };

        let logo_size = ROW_HEIGHT * 0.6;
        let logo_x = X_TIME;
        let logo_y = *current_y + (ROW_HEIGHT - logo_size) / 2.0;

        block.push_str(&format!(
            r#"<image x="{}" y="{}" width="{}" height="{}" href="resources/amsat.png"/><text x="{}" y="{}" class="{}">{}</text>
"#,
            logo_x, logo_y, logo_size, logo_size,
            logo_x + logo_size + 10.0,
            *current_y + ROW_HEIGHT / 2.0,
            status_class,
            status_text
        ));

        // Last update time
        let last_update_str = entry.last_updated.format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let hours_ago = (now_utc.signed_duration_since(entry.last_updated)).num_hours();

        block.push_str(&format!(
            r#"<text x="{}" y="{}" class="table-text">Last update: {} ({}h ago)</text>"#,
            X_CALLSIGN,
            *current_y + ROW_HEIGHT / 2.0,
            last_update_str,
            hours_ago
        ));
        block.push('\n');
        *current_y += ROW_HEIGHT;

        // Reports
        let total_reports = entry.total_reports();
        if total_reports == 0 {
            block.push_str(&format!(
                r#"<text x="410" y="{}" text-anchor="middle" class="table-text">No reports available.</text>"#,
                *current_y + 40.0
            ));
            block.push('\n');
            *current_y += 80.0;
            *current_y += BLOCK_SPACING;
            return Ok(block);
        }

        block.push_str(&self.generate_reports_section(
            &entry.api_name,
            &entry.reports,
            current_y,
            now_utc,
        )?);

        *current_y += BLOCK_SPACING;
        Ok(block)
    }
    
    /// Generate filename
    fn generate_filename(&self, search_results: &Vec<SearchResult>) -> String {
        let now = chrono::Utc::now();
        let minute = (now.minute() / 15) * 15;
        let floored = now
            .with_minute(minute)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();
        
        let time_str = floored.format("%Y%m%d_%H%M").to_string();
        
        let sat_names: Vec<String> = search_results
            .iter()
            .take(5)
            .map(|s| Self::normalize_sat_name(&s.transponder.amsat_api_name))
            .collect();
        
        let sat_part = if sat_names.is_empty() {
            "empty".to_string()
        } else if sat_names.len() > 3 {
            format!("{}_and_{}_more", sat_names[..2].join("_"), sat_names.len() - 2)
        } else {
            sat_names.join("_")
        };
        
        format!("sat_v2_{}_{}.png", time_str, sat_part)
    }
    
    fn normalize_sat_name(name: &str) -> String {
        name.chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase()
    }
    
    /// Generate image
    async fn generate_image(&self, search_results: &Vec<SearchResult>, output_path: &Path) -> Result<()> {
        let svg_content = self.generate_svg(search_results)?;
        self.render_svg_to_png(&svg_content, output_path).await?;
        Ok(())
    }
    
    /// Generate SVG content
    fn generate_svg(&self, search_results: &Vec<SearchResult>) -> Result<String> {
        let mut current_y = TOP_PADDING;
        let mut content = String::new();
        let now_utc = Utc::now();
        
        if search_results.is_empty() {
            content.push_str(&format!(
                r#"<text x="410" y="100" text-anchor="middle" class="table-text">No satellite data available.</text>"#
            ));
            current_y = 120.0;
        } else {
            for result in search_results {
                let block = self.generate_satellite_block(&result.transponder, &mut current_y, &now_utc)?;
                content.push_str(&block);
            }
        }
        
        let footer = self.generate_footer(current_y);
        let total_height = current_y + FOOTER_HEIGHT;
        
        let svg = SVG_TEMPLATE
            .replace("{{SVG_HEIGHT}}", &total_height.to_string())
            .replace("{{CONTENT}}", &content)
            .replace("{{FOOTER}}", &footer);
        
        Ok(svg)
    }
    
    /// Generate satellite block
    fn generate_satellite_block(
        &self,
        transponder: &Transponder,
        current_y: &mut f32,
        now_utc: &DateTime<Utc>,
    ) -> Result<String> {
        let mut block = String::new();
        
        // Transponder title with NORAD ID
        block.push_str(&format!(
            r#"<text x="{}" y="{}" class="satellite-title">{} (NORAD {})</text>"#,
            X_CALLSIGN,
            *current_y + BLOCK_TITLE_HEIGHT / 2.0,
            &transponder.amsat_api_name,
            &transponder.norad_id.map_or("Unknown".to_string(), |id| id.to_string())
        ));
        block.push('\n');
        *current_y += BLOCK_TITLE_HEIGHT;
        
        // Transponder information (up/down link & mode)
        block.push_str(&self.generate_transponder_info(&[transponder.clone()], current_y));
        
        // AMSAT update status
        let (status_class, status_text) = if transponder.amsat_update_success {
            ("amsat-update-success", "AMSAT Update: Success")
        } else {
            ("amsat-update-failure", "AMSAT Update: Failed")
        };
        
        let logo_size = ROW_HEIGHT * 0.6;
        let logo_x = X_TIME;
        let logo_y = *current_y + (ROW_HEIGHT - logo_size) / 2.0;
        
        block.push_str(&format!(
            r#"<image x="{}" y="{}" width="{}" height="{}" href="resources/amsat.png"/><text x="{}" y="{}" class="{}">{}</text>
"#,
            logo_x, logo_y, logo_size, logo_size,
            logo_x + logo_size + 10.0,
            *current_y + ROW_HEIGHT / 2.0,
            status_class,
            status_text
        ));
        
        // Last update time
        let last_update_str = transponder.last_updated.format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let hours_ago = (now_utc.signed_duration_since(transponder.last_updated)).num_hours();
        
        block.push_str(&format!(
            r#"<text x="{}" y="{}" class="table-text">Last update: {} ({}h ago)</text>"#,
            X_CALLSIGN,
            *current_y + ROW_HEIGHT / 2.0,
            last_update_str,
            hours_ago
        ));
        block.push('\n');
        *current_y += ROW_HEIGHT;
        
        // Check if we have reports
        let total_reports = transponder.total_reports();
        if total_reports == 0 {
            block.push_str(&format!(
                r#"<text x="410" y="{}" text-anchor="middle" class="table-text">No reports available.</text>"#,
                *current_y + 40.0
            ));
            block.push('\n');
            *current_y += 80.0;
            *current_y += BLOCK_SPACING;
            return Ok(block);
        }
        
        // Render reports
        if let Some(ref reports) = transponder.amsat_report {
            block.push_str(&self.generate_reports_section(
                &transponder.amsat_api_name,
                reports,
                current_y,
                now_utc,
            )?);
        }
        
        *current_y += BLOCK_SPACING;
        Ok(block)
    }
    
    /// Generate transponder information
    fn generate_transponder_info(&self, transponders: &[Transponder], current_y: &mut f32) -> String {
        let mut info = String::new();

        if transponders.is_empty() {
            info.push_str(&format!(
                r#"<text x="{}" y="{}" class="table-text" style="font-size:14px;fill:#666;">No transponder information available.</text>"#,
                X_CALLSIGN,
                *current_y + TRANSPONDER_INFO_HEIGHT / 2.0
            ));
            info.push('\n');
            *current_y += TRANSPONDER_INFO_HEIGHT;
            return info;
        }
        
        let primary = transponders.iter().find(|t| t.is_primary)
            .or_else(|| transponders.first());
        
        if let Some(trans) = primary {
            let uplink = trans.uplink.to_display();
            let downlink = trans.downlink.to_display();
            
            info.push_str(&format!(
                r#"<text x="{}" y="{}" class="table-text" style="font-size:14px;fill:#666;">↑{} ↓{} | {}</text>"#,
                X_CALLSIGN,
                *current_y + TRANSPONDER_INFO_HEIGHT / 2.0,
                Self::escape_xml(&uplink),
                Self::escape_xml(&downlink),
                Self::escape_xml(&trans.mode)
            ));
            info.push('\n');
            *current_y += TRANSPONDER_INFO_HEIGHT;
        }
        
        if transponders.len() > 1 {
            info.push_str(&format!(
                r#"<text x="{}" y="{}" class="table-text" style="font-size:12px;fill:#999;">({} transponders total)</text>"#,
                X_CALLSIGN,
                *current_y + 12.0,
                transponders.len()
            ));
            info.push('\n');
            *current_y += 18.0;
        }
        
        info
    }
    
    /// Generate reports section
    fn generate_reports_section(
        &self,
        api_name: &str,
        data_blocks: &[SatelliteDataBlock],
        current_y: &mut f32,
        now_utc: &DateTime<Utc>,
    ) -> Result<String> {
        let mut section = String::new();
        
        // Section header (transponder name)
        if api_name != "" {
            section.push_str(&format!(
                r#"<text x="{}" y="{}" class="table-text" style="font-weight:bold;font-size:12px;">{}</text>"#,
                X_CALLSIGN,
                *current_y + 18.0,
                Self::escape_xml(api_name)
            ));
            section.push('\n');
            *current_y += 28.0;
        }
        
        // Table header
        section.push_str(&format!(
            r##"<g class="header">
<rect x="0" y="{}" width="100%" height="{}" fill="#f0f2f5" />
<text x="{}" y="{}" class="table-text header-text">Callsign</text>
<text x="{}" y="{}" class="table-text header-text">Grids</text>
<text x="{}" y="{}" class="table-text header-text">Report</text>
<text x="{}" y="{}" class="table-text header-text">Time</text>
</g>
"##,
            *current_y, HEADER_HEIGHT,
            X_CALLSIGN, *current_y + HEADER_HEIGHT / 2.0,
            X_GRIDS, *current_y + HEADER_HEIGHT / 2.0,
            X_REPORT, *current_y + HEADER_HEIGHT / 2.0,
            X_TIME, *current_y + HEADER_HEIGHT / 2.0,
        ));
        *current_y += HEADER_HEIGHT;
        
        // Data rows
        let mut row_count = 0;
        'outer: for data_block in data_blocks {
            for report in &data_block.reports {
                if row_count >= MAX_REPORTS_PER_SATELLITE {
                    break 'outer;
                }
                
                let y_pos = *current_y + ROW_HEIGHT / 2.0;
                let report_color = ReportStatus::string_to_color_hex(&report.report);
                let report_text = ReportStatus::from_string(&report.report).to_string();
                
                let report_time = DateTime::parse_from_rfc3339(&report.reported_time)
                    .unwrap_or_else(|_| Utc::now().into());
                let hours_ago = (now_utc.signed_duration_since(report_time)).num_hours();
                
                let time_color = map_time_to_color(&report.reported_time, now_utc, 0.0, 12.0)
                    .unwrap_or_else(|_| "#808080".to_string());
                
                section.push_str(&format!(
                    r##"<g class="data-row">
   <text x="{}" y="{}" class="table-text">{}</text>
   <text x="{}" y="{}" class="table-text">{}</text>
   <rect x="{}" y="{}" width="{}" height="{}" fill="{}" rx="1" />
   <text x="{}" y="{}" class="table-text">{}</text>
   <rect x="{}" y="{}" width="{}" height="{}" fill="{}" rx="1" />
   <text x="{}" y="{}" class="table-text">{} ({}h ago)</text>
</g>
"##,
                    X_CALLSIGN, y_pos, Self::escape_xml(&report.callsign),
                    X_GRIDS, y_pos, Self::escape_xml(&report.grid_square),
                    X_REPORT, y_pos - COLOR_BLOCK_HEIGHT / 2.0, COLOR_BLOCK_WIDTH, COLOR_BLOCK_HEIGHT, report_color,
                    X_REPORT + COLOR_BLOCK_WIDTH + COLOR_BLOCK_TEXT_SPACING, y_pos, report_text,
                    X_TIME, y_pos - COLOR_BLOCK_HEIGHT / 2.0, COLOR_BLOCK_WIDTH, COLOR_BLOCK_HEIGHT, time_color,
                    X_TIME + COLOR_BLOCK_WIDTH + COLOR_BLOCK_TEXT_SPACING, y_pos, report.reported_time, hours_ago
                ));
                
                *current_y += ROW_HEIGHT;
                row_count += 1;
            }
        }
        
        Ok(section)
    }
    
    /// Generate footer
    fn generate_footer(&self, footer_y: f32) -> String {
        let render_time = chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        
        format!(
            r##"    <g id="footer">
<rect x="0" y="{}" width="100%" height="{}" fill="#f0f2f5" />
<text x="50%" y="{}" class="table-text footer-text" text-anchor="middle">
    Powered by Rinko, rendered at {} BJT
</text>
</g>
"##,
            footer_y,
            FOOTER_HEIGHT,
            footer_y + FOOTER_HEIGHT / 2.0,
            render_time
        )
    }
    
    /// Render SVG to PNG
    async fn render_svg_to_png(&self, svg_content: &str, output_path: &Path) -> Result<()> {
        use resvg::usvg::{fontdb, Options, Tree};
        use resvg::tiny_skia;
        use fontdb::Database;
        
        // Load fonts
        let mut fontdb = Database::new();
        fontdb.load_fonts_dir("fonts");
        tracing::debug!("Loaded {} font faces from fonts directory", fontdb.len());

        // Parse SVG with font database
        let mut options = Options::default();
        options.font_family = "Consolas".to_string();
        options.fontdb = std::sync::Arc::new(fontdb);

        let tree = Tree::from_str(svg_content, &options)
            .context("Failed to parse SVG")?;
        
        let size = tree.size();
        let width = size.width() as u32;
        let height = size.height() as u32;
        
        // Render to PNG
        let mut pixmap = tiny_skia::Pixmap::new(width, height)
            .context("Failed to create pixmap")?;
        
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
        
        pixmap.save_png(output_path)
            .context("Failed to save PNG")?;
        
        Ok(())
    }
    
    fn escape_xml(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
}
