///! Cache management for satellite images
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

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
