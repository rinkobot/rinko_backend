///! File cache management for satellite data
use super::types::{SatelliteInfo, SatelliteList};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

const SATELLITE_CACHE_PATH: &str = "satellite_cache";
const SATELLITE_CACHE_FILE: &str = "satellite_cache.json";
const SATELLITE_LIST_FILE: &str = "satellite_list.toml";

/// Load satellite cache from JSON file
/// 
/// # Arguments
/// * `cache_dir` - Directory containing cache files
/// 
/// # Returns
/// Vec of SatelliteInfo on success, empty vec if file doesn't exist
pub async fn load_satellite_cache(cache_dir: &Path) -> Result<Vec<SatelliteInfo>> {
    let cache_path = cache_dir.join(format!("{}/{}", SATELLITE_CACHE_PATH, SATELLITE_CACHE_FILE));
    
    if !cache_path.exists() {
        tracing::info!("Cache file not found at {:?}, starting fresh", cache_path);
        return Ok(Vec::new());
    }
    
    let content = fs::read_to_string(&cache_path)
        .await
        .context(format!("Failed to read cache file: {:?}", cache_path))?;
    
    let satellites: Vec<SatelliteInfo> = serde_json::from_str(&content)
        .context("Failed to parse satellite cache JSON")?;
    
    tracing::info!(
        "Loaded {} satellites from cache: {:?}",
        satellites.len(),
        cache_path
    );
    
    Ok(satellites)
}

/// Save satellite cache to JSON file
/// 
/// # Arguments
/// * `cache_dir` - Directory to save cache files
/// * `satellites` - Vec of SatelliteInfo to save
pub async fn save_satellite_cache(
    cache_dir: &Path,
    satellites: &[SatelliteInfo],
) -> Result<()> {
    // Ensure directory exists
    fs::create_dir_all(cache_dir)
        .await
        .context(format!("Failed to create cache directory: {:?}", cache_dir))?;
    
    let cache_path = cache_dir.join(format!("{}/{}", SATELLITE_CACHE_PATH, SATELLITE_CACHE_FILE));
    
    let json = serde_json::to_string_pretty(satellites)
        .context("Failed to serialize satellite cache")?;
    
    fs::write(&cache_path, json)
        .await
        .context(format!("Failed to write cache file: {:?}", cache_path))?;
    
    tracing::debug!(
        "Saved {} satellites to cache: {:?}",
        satellites.len(),
        cache_path
    );
    
    Ok(())
}

/// Load satellite list from TOML file
/// 
/// # Arguments
/// * `cache_dir` - Directory containing configuration files
/// 
/// # Returns
/// SatelliteList on success, default if file doesn't exist
pub async fn load_satellite_list(cache_dir: &Path) -> Result<SatelliteList> {
    let list_path = cache_dir.join(SATELLITE_LIST_FILE);
    
    if !list_path.exists() {
        tracing::info!("Satellite list file not found at {:?}, creating default", list_path);
        let default_list = SatelliteList::default();
        save_satellite_list(cache_dir, &default_list).await?;
        return Ok(default_list);
    }
    
    let content = fs::read_to_string(&list_path)
        .await
        .context(format!("Failed to read satellite list file: {:?}", list_path))?;
    
    let list: SatelliteList = toml::from_str(&content)
        .context("Failed to parse satellite list TOML")?;
    
    tracing::info!(
        "Loaded {} satellites from list: {:?}",
        list.satellites.len(),
        list_path
    );
    
    Ok(list)
}

/// Save satellite list to TOML file
/// 
/// # Arguments
/// * `cache_dir` - Directory to save configuration files
/// * `list` - SatelliteList to save
pub async fn save_satellite_list(cache_dir: &Path, list: &SatelliteList) -> Result<()> {
    // Ensure directory exists
    fs::create_dir_all(cache_dir)
        .await
        .context(format!("Failed to create cache directory: {:?}", cache_dir))?;
    
    let list_path = cache_dir.join(SATELLITE_LIST_FILE);
    
    let toml = toml::to_string_pretty(list)
        .context("Failed to serialize satellite list")?;
    
    fs::write(&list_path, toml)
        .await
        .context(format!("Failed to write satellite list file: {:?}", list_path))?;
    
    tracing::debug!(
        "Saved {} satellites to list: {:?}",
        list.satellites.len(),
        list_path
    );
    
    Ok(())
}

/// Get the path to the rendered images directory
pub fn get_images_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("image_cache")
}

/// Ensure the images directory exists
pub async fn ensure_images_dir(cache_dir: &Path) -> Result<PathBuf> {
    let images_dir = get_images_dir(cache_dir);
    fs::create_dir_all(&images_dir)
        .await
        .context(format!("Failed to create images directory: {:?}", images_dir))?;
    Ok(images_dir)
}

/// Clean up old cached images (older than specified days)
pub async fn cleanup_old_images(cache_dir: &Path, days_to_keep: i64) -> Result<usize> {
    let images_dir = get_images_dir(cache_dir);
    
    if !images_dir.exists() {
        return Ok(0);
    }
    
    let mut deleted_count = 0;
    let cutoff_time = chrono::Utc::now() - chrono::Duration::days(days_to_keep);
    
    let mut entries = fs::read_dir(&images_dir).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        
        if path.is_file() && path.extension().map_or(false, |ext| ext == "png") {
            if let Ok(metadata) = entry.metadata().await {
                if let Ok(modified) = metadata.modified() {
                    let modified_time: chrono::DateTime<chrono::Utc> = modified.into();
                    if modified_time < cutoff_time {
                        if let Err(e) = fs::remove_file(&path).await {
                            tracing::warn!("Failed to delete old image {:?}: {}", path, e);
                        } else {
                            deleted_count += 1;
                            tracing::debug!("Deleted old image: {:?}", path);
                        }
                    }
                }
            }
        }
    }
    
    if deleted_count > 0 {
        tracing::info!("Cleaned up {} old images from {:?}", deleted_count, images_dir);
    }
    
    Ok(deleted_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::SatelliteEntry;

    #[tokio::test]
    async fn test_save_and_load_cache() {
        let temp_dir = std::env::temp_dir().join("rinko_test_cache");
        let _ = fs::remove_dir_all(&temp_dir).await; // Clean up first
        
        let satellites = vec![
            SatelliteInfo::new("AO-91"),
            SatelliteInfo::new("ISS-FM"),
        ];
        
        save_satellite_cache(&temp_dir, &satellites).await.unwrap();
        let loaded = load_satellite_cache(&temp_dir).await.unwrap();
        
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "AO-91");
        assert_eq!(loaded[1].name, "ISS-FM");
        
        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_save_and_load_list() {
        let temp_dir = std::env::temp_dir().join("rinko_test_list");
        let _ = fs::remove_dir_all(&temp_dir).await;
        
        let mut list = SatelliteList::default();
        list.satellites.push(SatelliteEntry::new("AO-91"));
        list.satellites.push(SatelliteEntry::new("ISS-FM"));
        
        save_satellite_list(&temp_dir, &list).await.unwrap();
        let loaded = load_satellite_list(&temp_dir).await.unwrap();
        
        assert_eq!(loaded.satellites.len(), 2);
        assert_eq!(loaded.satellites[0].official_name, "AO-91");
        
        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir).await;
    }
}
