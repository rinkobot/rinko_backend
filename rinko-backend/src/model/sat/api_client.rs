use anyhow::{Context, Result};
use reqwest::Client;
use tracing::{debug, error, warn};
use super::types::AmsatReport;

const AMSAT_API_BASE: &str = "https://amsat.org/status/api/v1/sat_info.php";
const DEFAULT_HOURS: u32 = 96;
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// AMSAT API 客户端
#[derive(Debug, Clone)]
pub struct AmsatApiClient {
    client: Client,
    base_url: String,
}

impl AmsatApiClient {
    /// 创建新的API客户端
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("Failed to create HTTP client")?;
        
        Ok(Self {
            client,
            base_url: AMSAT_API_BASE.to_string(),
        })
    }
    
    /// 获取指定卫星的状态报告
    /// 
    /// # Arguments
    /// * `satellite_name` - 卫星名称（必须与AMSAT API中的名称完全匹配）
    /// * `hours` - 获取最近多少小时的数据（可选，默认96小时）
    pub async fn fetch_satellite_status(
        &self,
        satellite_name: &str,
        hours: Option<u32>,
    ) -> Result<Vec<AmsatReport>> {
        let hours = hours.unwrap_or(DEFAULT_HOURS);
        
        debug!(
            "Fetching satellite status for '{}' (last {} hours)",
            satellite_name, hours
        );
        
        let url = format!(
            "{}?name={}&hours={}",
            self.base_url,
            urlencoding::encode(satellite_name),
            hours
        );
        
        match self.client.get(&url).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    warn!(
                        "API request failed for '{}': status {}",
                        satellite_name,
                        response.status()
                    );
                    return Ok(Vec::new());
                }
                
                match response.json::<Vec<AmsatReport>>().await {
                    Ok(reports) => {
                        debug!(
                            "Fetched {} reports for '{}'",
                            reports.len(),
                            satellite_name
                        );
                        Ok(reports)
                    }
                    Err(e) => {
                        warn!(
                            "Failed to parse JSON response for '{}': {}",
                            satellite_name, e
                        );
                        Ok(Vec::new())
                    }
                }
            }
            Err(e) => {
                error!(
                    "Network error fetching status for '{}': {}",
                    satellite_name, e
                );
                Ok(Vec::new())
            }
        }
    }
    
    /// 批量获取多个卫星的状态
    /// 
    /// # Arguments
    /// * `satellite_names` - 卫星名称列表
    /// * `hours` - 获取最近多少小时的数据
    pub async fn fetch_multiple_satellites(
        &self,
        satellite_names: &[String],
        hours: Option<u32>,
    ) -> Vec<(String, Result<Vec<AmsatReport>>)> {
        let mut results = Vec::new();
        
        for name in satellite_names {
            let reports = self.fetch_satellite_status(name, hours).await;
            results.push((name.clone(), reports));
            
            // 添加小延迟避免过于频繁的请求
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
        
        results
    }
}

impl Default for AmsatApiClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default AmsatApiClient")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // 需要网络连接，正常情况下忽略
    async fn test_fetch_satellite_status() {
        let client = AmsatApiClient::new().unwrap();
        let reports = client.fetch_satellite_status("AO-91", Some(24)).await;
        
        assert!(reports.is_ok());
        println!("Fetched {} reports", reports.unwrap().len());
    }
}
