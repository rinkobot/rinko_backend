use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tracing::{info, warn};

/// DXpedition 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxPedition {
    /// 电台呼号 (如 "KP5/NP3VI", "J38TT")
    pub callsign: String,
    /// 地点名称 (如 "DESECHEO", "GRENADA")
    pub location: String,
    /// 详情链接
    pub url: Option<String>,
    /// 开始日期 (1-28，表示月份中的某天)
    pub start_day: Option<u8>,
    /// 持续天数
    pub duration_days: Option<u8>,
}

/// DX World 时间线数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DxWorldTimeline {
    /// 月份名称 (如 "FEBRUARY")
    pub month: String,
    /// 最后更新时间
    pub last_update: Option<String>,
    /// DXpedition 列表
    pub expeditions: Vec<DxPedition>,
}

/// DX World HTML 解析器
pub struct DxWorldParser;

impl DxWorldParser {
    /// 从 HTML 文件解析 DXpedition 数据
    pub async fn parse_file<P: AsRef<Path>>(path: P) -> Result<DxWorldTimeline> {
        let html_content = fs::read_to_string(path.as_ref())
            .await
            .with_context(|| format!("Failed to read HTML file: {:?}", path.as_ref()))?;
        
        Self::parse_html(&html_content)
    }

    /// 从 HTML 字符串解析 DXpedition 数据
    pub fn parse_html(html: &str) -> Result<DxWorldTimeline> {
        // 提取月份名称
        let month = Self::extract_month(html)?;
        
        // 提取最后更新时间
        let last_update = Self::extract_last_update(html);
        
        // 提取电台呼号列表
        let callsigns = Self::extract_callsigns(html)?;
        
        // 提取地点和链接信息
        let locations_with_urls = Self::extract_tooltips(html)?;
        
        // 提取时间线数据 (开始日期和持续时间)
        let timeline_data = Self::extract_timeline_data(html)?;
        
        // 合并数据
        let mut expeditions = Vec::new();
        let max_len = callsigns.len().max(locations_with_urls.len());
        
        for i in 0..max_len {
            let callsign = callsigns.get(i).cloned().unwrap_or_default();
            
            // 跳过空的呼号
            if callsign.is_empty() {
                continue;
            }
            
            let (location, url) = locations_with_urls
                .get(i)
                .cloned()
                .unwrap_or((String::new(), None));
            
            let (start_day, duration_days) = timeline_data
                .get(i)
                .cloned()
                .unwrap_or((None, None));
            
            expeditions.push(DxPedition {
                callsign,
                location,
                url,
                start_day,
                duration_days,
            });
        }
        
        info!("Parsed {} DXpeditions from HTML", expeditions.len());
        
        Ok(DxWorldTimeline {
            month,
            last_update,
            expeditions,
        })
    }

    /// 提取月份名称
    fn extract_month(html: &str) -> Result<String> {
        // 查找类似 context.fillText('FEBRUARY', 275, 340); 的内容
        let re = Regex::new(r#"context\.fillText\(['"]([A-Z]+)['"],\s*\d+,\s*340\)"#)
            .context("Failed to compile month regex")?;
        
        if let Some(caps) = re.captures(html) {
            if let Some(month) = caps.get(1) {
                return Ok(month.as_str().to_string());
            }
        }
        
        // 备用方案：从 meta description 提取
        let re_meta = Regex::new(r#"content="DX-World\.net - ([A-Z]+) Featured"#)
            .context("Failed to compile meta regex")?;
        
        if let Some(caps) = re_meta.captures(html) {
            if let Some(month) = caps.get(1) {
                return Ok(month.as_str().to_string());
            }
        }
        
        warn!("Could not extract month from HTML, using default");
        Ok("UNKNOWN".to_string())
    }

    /// 提取最后更新时间
    fn extract_last_update(html: &str) -> Option<String> {
        // 查找类似 context.fillText('Last update: February 15, 2026', 15, 65);
        let re = Regex::new(r#"Last update:\s*([^'"]+)['"]"#).ok()?;
        
        re.captures(html)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// 提取电台呼号列表
    fn extract_callsigns(html: &str) -> Result<Vec<String>> {
        // 查找类似 var labels = ['KP5/NP3VI','','J38TT', ...]; 的内容
        let re = Regex::new(r#"var labels = \[(.*?)\];"#)
            .context("Failed to compile labels regex")?;
        
        if let Some(caps) = re.captures(html) {
            if let Some(labels_str) = caps.get(1) {
                let labels_content = labels_str.as_str();
                
                // 提取所有引号内的内容
                let item_re = Regex::new(r#"['"]([^'"]*)['"]\s*"#)
                    .context("Failed to compile item regex")?;
                
                let callsigns: Vec<String> = item_re
                    .captures_iter(labels_content)
                    .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
                    .collect();
                
                info!("Extracted {} callsigns", callsigns.len());
                return Ok(callsigns);
            }
        }
        
        warn!("Could not extract callsigns from HTML");
        Ok(Vec::new())
    }

    /// 提取地点和链接信息
    fn extract_tooltips(html: &str) -> Result<Vec<(String, Option<String>)>> {
        // 查找 tooltips 数组中的内容
        // 格式: "<b>DESESCHEO</b><br /><a href=\"https://...\" target=\"_blank\">Read more</a>"
        let re = Regex::new(r#"<b>([^<]*)</b><br /><a href=\\"([^\\"]*)\\""#)
            .context("Failed to compile tooltip regex")?;
        
        let mut results = Vec::new();
        
        for caps in re.captures_iter(html) {
            let location = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let url = caps.get(2).map(|m| m.as_str().to_string());
            results.push((location, url));
        }
        
        info!("Extracted {} location entries", results.len());
        Ok(results)
    }

    /// 提取时间线数据（开始日期和持续时间）
    fn extract_timeline_data(html: &str) -> Result<Vec<(Option<u8>, Option<u8>)>> {
        // 查找 data 数组
        // 格式: data = [[[0, 28,,,'#8BFF61'],[0, ,,,'#8FF4FF']], ...]
        // 使用 (?s) 让 . 匹配换行符
        let re = Regex::new(r#"(?s)data = \[(.*?)\];"#)
            .context("Failed to compile data regex")?;
        
        let mut results = Vec::new();
        
        if let Some(caps) = re.captures(html) {
            if let Some(data_str) = caps.get(1) {
                let data_content = data_str.as_str();
                
                // 提取每一行的数组 [[...],[...]]
                let line_re = Regex::new(r#"\[\[([^\]]+)\]\s*,\s*\[([^\]]+)\]\]"#)
                    .context("Failed to compile line regex")?;
                
                for caps in line_re.captures_iter(data_content) {
                    // 解析第一个数组（通常包含主要的开始日期和持续时间）
                    if let Some(first_array) = caps.get(1) {
                        let parts: Vec<&str> = first_array.as_str()
                            .split(',')
                            .map(|s| s.trim())
                            .collect();
                        
                        let start_day = if !parts.is_empty() && !parts[0].is_empty() {
                            parts[0].parse::<u8>().ok()
                        } else {
                            None
                        };
                        
                        let duration_days = if parts.len() > 1 && !parts[1].is_empty() {
                            parts[1].parse::<u8>().ok()
                        } else {
                            None
                        };
                        
                        results.push((start_day, duration_days));
                    }
                }
            }
        }
        
        info!("Extracted {} timeline data entries", results.len());
        Ok(results)
    }

    /// 将解析结果保存为 JSON
    pub async fn save_as_json<P: AsRef<Path>>(
        timeline: &DxWorldTimeline,
        path: P,
    ) -> Result<()> {
        let json = serde_json::to_string_pretty(timeline)
            .context("Failed to serialize timeline to JSON")?;
        
        fs::write(path.as_ref(), json)
            .await
            .with_context(|| format!("Failed to write JSON to: {:?}", path.as_ref()))?;
        
        info!("Saved timeline as JSON to: {:?}", path.as_ref());
        Ok(())
    }

    /// 从 JSON 文件加载数据
    pub async fn load_from_json<P: AsRef<Path>>(path: P) -> Result<DxWorldTimeline> {
        let json = fs::read_to_string(path.as_ref())
            .await
            .with_context(|| format!("Failed to read JSON file: {:?}", path.as_ref()))?;
        
        let timeline: DxWorldTimeline = serde_json::from_str(&json)
            .context("Failed to deserialize JSON")?;
        
        Ok(timeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_month() {
        let html = r#"context.fillText('FEBRUARY', 275, 340);"#;
        let month = DxWorldParser::extract_month(html).unwrap();
        assert_eq!(month, "FEBRUARY");
    }

    #[test]
    fn test_extract_callsigns() {
        let html = r#"var labels = ['KP5/NP3VI','','J38TT','8R1WA'];"#;
        let callsigns = DxWorldParser::extract_callsigns(html).unwrap();
        assert_eq!(callsigns, vec!["KP5/NP3VI", "", "J38TT", "8R1WA"]);
    }

    #[test]
    fn test_extract_last_update() {
        let html = r#"context.fillText('Last update: February 15, 2026', 15, 65);"#;
        let update = DxWorldParser::extract_last_update(html);
        assert_eq!(update, Some("February 15, 2026".to_string()));
    }
}
