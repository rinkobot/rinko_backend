use anyhow::{Context, Result};
use chrono::Utc;
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

use super::types::{SatelliteEntry, SatelliteInfo, SatelliteListExport};

/// 本地缓存文件名
const CACHE_FILE: &str = "satellite_cache.json";
const SATELLITE_LIST_FILE: &str = "satellite_list.json";

/// 缓存管理器
pub struct CacheManager {
    cache_dir: PathBuf,
}

impl CacheManager {
    /// 创建新的缓存管理器
    /// 
    /// # Arguments
    /// * `cache_dir` - 缓存目录路径
    pub fn new<P: AsRef<Path>>(cache_dir: P) -> Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        
        Ok(Self { cache_dir })
    }
    
    /// 确保缓存目录存在
    pub async fn ensure_cache_dir(&self) -> Result<()> {
        if !self.cache_dir.exists() {
            fs::create_dir_all(&self.cache_dir)
                .await
                .context("Failed to create cache directory")?;
            info!("Created cache directory: {:?}", self.cache_dir);
        }
        Ok(())
    }
    
    /// 获取缓存文件路径
    fn get_cache_path(&self) -> PathBuf {
        self.cache_dir.join(CACHE_FILE)
    }
    
    /// 获取卫星列表文件路径
    fn get_satellite_list_path(&self) -> PathBuf {
        self.cache_dir.join(SATELLITE_LIST_FILE)
    }
    
    /// 加载缓存的卫星数据
    pub async fn load_cache(&self) -> Result<HashMap<String, SatelliteInfo>> {
        let cache_path = self.get_cache_path();
        
        if !cache_path.exists() {
            debug!("Cache file does not exist: {:?}", cache_path);
            return Ok(HashMap::new());
        }
        
        let content = fs::read_to_string(&cache_path)
            .await
            .context("Failed to read cache file")?;
        
        let cache: HashMap<String, SatelliteInfo> = serde_json::from_str(&content)
            .context("Failed to parse cache file")?;
        
        info!("Loaded {} satellites from cache", cache.len());
        Ok(cache)
    }
    
    /// 保存卫星数据到缓存
    pub async fn save_cache(&self, satellites: &HashMap<String, SatelliteInfo>) -> Result<()> {
        self.ensure_cache_dir().await?;
        
        let cache_path = self.get_cache_path();
        let content = serde_json::to_string_pretty(satellites)
            .context("Failed to serialize cache")?;
        
        fs::write(&cache_path, content)
            .await
            .context("Failed to write cache file")?;
        
        debug!("Saved {} satellites to cache", satellites.len());
        Ok(())
    }
    
    /// 导出卫星列表到文件
    pub async fn export_satellite_list(
        &self,
        satellites: &HashMap<String, SatelliteInfo>,
    ) -> Result<()> {
        self.ensure_cache_dir().await?;
        
        let active_count = satellites.values().filter(|s| s.is_active).count();
        
        let satellite_entries: Vec<SatelliteEntry> = satellites
            .values()
            .map(|sat| SatelliteEntry {
                name: sat.name.clone(),
                aliases: sat.aliases.clone(),
                is_active: sat.is_active,
                status: sat.status.clone(),
            })
            .collect();
        
        let export = SatelliteListExport {
            exported_at: Utc::now(),
            active_count,
            total_count: satellites.len(),
            satellites: satellite_entries,
        };
        
        let list_path = self.get_satellite_list_path();
        let content = serde_json::to_string_pretty(&export)
            .context("Failed to serialize satellite list")?;
        
        fs::write(&list_path, content)
            .await
            .context("Failed to write satellite list file")?;
        
        info!(
            "Exported satellite list: {} total, {} active",
            export.total_count, export.active_count
        );
        Ok(())
    }
    
    /// 加载卫星列表
    pub async fn load_satellite_list(&self) -> Result<SatelliteListExport> {
        let list_path = self.get_satellite_list_path();
        
        if !list_path.exists() {
            warn!("Satellite list file does not exist: {:?}", list_path);
            return Ok(SatelliteListExport {
                exported_at: Utc::now(),
                active_count: 0,
                total_count: 0,
                satellites: Vec::new(),
            });
        }
        
        let content = fs::read_to_string(&list_path)
            .await
            .context("Failed to read satellite list file")?;
        
        let list: SatelliteListExport = serde_json::from_str(&content)
            .context("Failed to parse satellite list file")?;
        
        debug!("Loaded satellite list with {} entries", list.total_count);
        Ok(list)
    }
    
    /// 查询卫星（从列表文件中搜索）
    pub async fn search_satellites(&self, query: &str) -> Result<Vec<SatelliteEntry>> {
        let list = self.load_satellite_list().await?;
        
        let query_lower = query.to_lowercase();
        let results: Vec<SatelliteEntry> = list
            .satellites
            .into_iter()
            .filter(|sat| {
                // 匹配主名称或别名
                sat.name.to_lowercase().contains(&query_lower)
                    || sat.aliases.iter().any(|alias| {
                        alias.to_lowercase().contains(&query_lower)
                    })
            })
            .collect();
        
        debug!("Search '{}' found {} results", query, results.len());
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_cache_operations() {
        let temp_dir = TempDir::new().unwrap();
        let cache_manager = CacheManager::new(temp_dir.path()).unwrap();
        
        // 测试保存和加载
        let mut satellites = HashMap::new();
        satellites.insert(
            "AO-91".to_string(),
            SatelliteInfo::new("AO-91".to_string()),
        );
        
        cache_manager.save_cache(&satellites).await.unwrap();
        let loaded = cache_manager.load_cache().await.unwrap();
        
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("AO-91"));
    }
}
