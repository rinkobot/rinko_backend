use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::api_client::AmsatApiClient;
use super::cache::CacheManager;
use super::types::{SatelliteEntry, SatelliteInfo, KNOWN_SATELLITES};

/// 卫星状态管理器
pub struct SatelliteManager {
    /// API 客户端
    api_client: AmsatApiClient,
    
    /// 缓存管理器
    cache_manager: CacheManager,
    
    /// 卫星数据（内存缓存）
    satellites: Arc<RwLock<HashMap<String, SatelliteInfo>>>,
    
    /// 更新间隔（分钟）
    update_interval_minutes: i64,
}

impl SatelliteManager {
    /// 创建新的卫星管理器
    /// 
    /// # Arguments
    /// * `cache_dir` - 缓存目录路径
    /// * `update_interval_minutes` - 更新间隔（分钟）
    pub fn new<P: AsRef<Path>>(
        cache_dir: P,
        update_interval_minutes: i64,
    ) -> Result<Self> {
        let api_client = AmsatApiClient::new()?;
        let cache_manager = CacheManager::new(cache_dir)?;
        
        Ok(Self {
            api_client,
            cache_manager,
            satellites: Arc::new(RwLock::new(HashMap::new())),
            update_interval_minutes,
        })
    }
    
    /// 初始化管理器（加载缓存数据）
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing satellite manager...");
        
        // 确保缓存目录存在
        self.cache_manager.ensure_cache_dir().await?;
        
        // 加载缓存数据
        match self.cache_manager.load_cache().await {
            Ok(cached_satellites) => {
                if !cached_satellites.is_empty() {
                    let mut satellites = self.satellites.write().await;
                    *satellites = cached_satellites;
                    info!("Loaded {} satellites from cache", satellites.len());
                } else {
                    info!("No cached data found, initializing with known satellites");
                    self.initialize_known_satellites().await?;
                }
            }
            Err(e) => {
                warn!("Failed to load cache: {}, initializing fresh", e);
                self.initialize_known_satellites().await?;
            }
        }
        
        // 导出卫星列表
        self.export_satellite_list().await?;
        
        info!("Satellite manager initialized successfully");
        Ok(())
    }
    
    /// 初始化已知卫星列表
    async fn initialize_known_satellites(&self) -> Result<()> {
        let mut satellites = self.satellites.write().await;
        
        for &name in KNOWN_SATELLITES.iter() {
            if !satellites.contains_key(name) {
                satellites.insert(name.to_string(), SatelliteInfo::new(name.to_string()));
            }
        }
        
        info!("Initialized {} known satellites", satellites.len());
        Ok(())
    }
    
    /// 更新所有卫星状态
    pub async fn update_all_satellites(&self) -> Result<()> {
        info!("Starting full satellite update...");
        
        let satellite_names: Vec<String> = {
            let satellites = self.satellites.read().await;
            satellites.keys().cloned().collect()
        };
        
        info!("Updating {} satellites", satellite_names.len());
        
        let results = self
            .api_client
            .fetch_multiple_satellites(&satellite_names, Some(96))
            .await;
        
        let mut update_count = 0;
        let mut error_count = 0;
        
        {
            let mut satellites = self.satellites.write().await;
            
            for (name, result) in results {
                match result {
                    Ok(reports) => {
                        if let Some(sat) = satellites.get_mut(&name) {
                            sat.update_reports(reports);
                            update_count += 1;
                        }
                    }
                    Err(e) => {
                        error!("Failed to update satellite '{}': {}", name, e);
                        if let Some(sat) = satellites.get_mut(&name) {
                            sat.mark_fetch_failed();
                        }
                        error_count += 1;
                    }
                }
            }
        }
        
        info!(
            "Update completed: {} updated, {} errors",
            update_count, error_count
        );
        
        // 保存到缓存
        self.save_cache().await?;
        
        // 导出卫星列表
        self.export_satellite_list().await?;
        
        Ok(())
    }
    
    /// 更新单个卫星
    pub async fn update_satellite(&self, name: &str) -> Result<()> {
        debug!("Updating satellite '{}'", name);
        
        let reports = self.api_client.fetch_satellite_status(name, Some(96)).await?;
        
        {
            let mut satellites = self.satellites.write().await;
            
            if let Some(sat) = satellites.get_mut(name) {
                sat.update_reports(reports);
            } else {
                // 自动添加新卫星
                let mut new_sat = SatelliteInfo::new(name.to_string());
                new_sat.update_reports(reports);
                satellites.insert(name.to_string(), new_sat);
                info!("Auto-added new satellite: {}", name);
            }
        }
        
        Ok(())
    }
    
    /// 保存缓存
    async fn save_cache(&self) -> Result<()> {
        let satellites = self.satellites.read().await;
        self.cache_manager.save_cache(&satellites).await
    }
    
    /// 导出卫星列表
    async fn export_satellite_list(&self) -> Result<()> {
        let satellites = self.satellites.read().await;
        self.cache_manager.export_satellite_list(&satellites).await
    }
    
    /// 查询卫星
    pub async fn query_satellite(&self, query: &str) -> Result<Option<SatelliteInfo>> {
        // 首先从本地列表搜索
        let search_results = self.cache_manager.search_satellites(query).await?;
        
        if search_results.is_empty() {
            debug!("No satellites found for query '{}'", query);
            return Ok(None);
        }
        
        // 取第一个匹配结果
        let entry = &search_results[0];
        
        // 从缓存中获取完整信息
        let satellites = self.satellites.read().await;
        Ok(satellites.get(&entry.name).cloned())
    }
    
    /// 搜索卫星（返回多个匹配）
    pub async fn search_satellites(&self, query: &str) -> Result<Vec<SatelliteEntry>> {
        self.cache_manager.search_satellites(query).await
    }
    
    /// 获取所有活跃卫星
    pub async fn get_active_satellites(&self) -> Vec<SatelliteInfo> {
        let satellites = self.satellites.read().await;
        satellites
            .values()
            .filter(|sat| sat.is_active)
            .cloned()
            .collect()
    }
    
    /// 获取卫星总数
    pub async fn get_satellite_count(&self) -> (usize, usize) {
        let satellites = self.satellites.read().await;
        let total = satellites.len();
        let active = satellites.values().filter(|sat| sat.is_active).count();
        (total, active)
    }
    
    /// 剔除非活跃卫星（可选功能）
    pub async fn prune_inactive_satellites(&self, days_threshold: i64) -> Result<usize> {
        let mut satellites = self.satellites.write().await;
        let mut removed = 0;
        
        let threshold = chrono::Utc::now() - chrono::Duration::days(days_threshold);
        
        satellites.retain(|name, sat| {
            if let Some(last_success) = sat.last_fetch_success {
                if last_success < threshold && !sat.is_active {
                    info!("Pruning inactive satellite: {}", name);
                    removed += 1;
                    return false;
                }
            }
            true
        });
        
        if removed > 0 {
            info!("Pruned {} inactive satellites", removed);
            self.save_cache().await?;
            self.export_satellite_list().await?;
        }
        
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_satellite_manager_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SatelliteManager::new(temp_dir.path(), 10).unwrap();
        
        manager.initialize().await.unwrap();
        
        let (total, active) = manager.get_satellite_count().await;
        assert!(total > 0);
    }
}
