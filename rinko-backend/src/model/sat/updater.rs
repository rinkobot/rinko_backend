use anyhow::Result;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};

use super::manager::SatelliteManager;

/// 定时更新任务
pub struct SatelliteUpdater {
    manager: Arc<SatelliteManager>,
    update_interval_minutes: u64,
}

impl SatelliteUpdater {
    /// 创建新的更新任务
    pub fn new(manager: Arc<SatelliteManager>, update_interval_minutes: u64) -> Self {
        Self {
            manager,
            update_interval_minutes,
        }
    }
    
    /// 启动定时更新任务
    /// 
    /// 这个函数会启动一个后台任务，每隔指定的时间间隔自动更新所有卫星状态
    pub fn start(self) -> tokio::task::JoinHandle<()> {
        info!(
            "Starting satellite updater (interval: {} minutes)",
            self.update_interval_minutes
        );
        
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(self.update_interval_minutes * 60));
            
            loop {
                ticker.tick().await;
                
                info!("Triggering scheduled satellite update");
                
                match self.manager.update_all_satellites().await {
                    Ok(_) => {
                        let (total, active) = self.manager.get_satellite_count().await;
                        info!(
                            "Scheduled update completed successfully: {} total, {} active",
                            total, active
                        );
                    }
                    Err(e) => {
                        error!("Scheduled update failed: {}", e);
                    }
                }
            }
        })
    }
    
    /// 启动带初始更新的定时任务
    /// 
    /// 立即执行一次更新，然后启动定时任务
    pub async fn start_with_initial_update(self) -> Result<tokio::task::JoinHandle<()>> {
        info!("Running initial satellite update");
        
        // 先执行一次更新
        match self.manager.update_all_satellites().await {
            Ok(_) => {
                let (total, active) = self.manager.get_satellite_count().await;
                info!(
                    "Initial update completed: {} total, {} active",
                    total, active
                );
            }
            Err(e) => {
                error!("Initial update failed: {}", e);
            }
        }
        
        // 启动定时任务
        Ok(self.start())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_updater_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = Arc::new(
            SatelliteManager::new(temp_dir.path(), 10).unwrap()
        );
        
        manager.initialize().await.unwrap();
        
        let updater = SatelliteUpdater::new(manager, 10);
        
        // 只测试创建，不实际运行定时任务
        assert_eq!(updater.update_interval_minutes, 10);
    }
}
