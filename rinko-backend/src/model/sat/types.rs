use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// 卫星状态枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SatelliteStatus {
    /// 在轨运行
    Operational,
    /// 部分功能
    PartiallyOperational,
    /// 非运行状态
    NonOperational,
    /// 已失联
    Lost,
    /// 已再入
    Deorbited,
    /// 未知状态
    Unknown,
}

impl Default for SatelliteStatus {
    fn default() -> Self {
        SatelliteStatus::Unknown
    }
}

/// AMSAT API 返回的单条报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmsatReport {
    /// 卫星名称
    pub name: String,
    /// 报告时间
    pub reported_time: String,
    /// 呼号
    pub callsign: String,
    /// 报告内容
    pub report: String,
    /// 网格方块
    pub grid_square: String,
}

/// 卫星完整信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteInfo {
    /// 卫星名称（主名称）
    pub name: String,
    
    /// 卫星编号（NORAD ID等，占位符）
    #[serde(default)]
    pub catalog_number: Option<String>,
    
    /// 卫星状态
    #[serde(default)]
    pub status: SatelliteStatus,
    
    /// 别名列表（用于搜索）
    #[serde(default)]
    pub aliases: Vec<String>,
    
    /// 最新报告列表
    #[serde(default)]
    pub recent_reports: Vec<AmsatReport>,
    
    /// 最后更新时间
    #[serde(default = "default_datetime")]
    pub last_updated: DateTime<Utc>,
    
    /// 最后成功拉取时间
    #[serde(default)]
    pub last_fetch_success: Option<DateTime<Utc>>,
    
    /// 是否活跃（最近有报告）
    #[serde(default)]
    pub is_active: bool,
    
    /// 扩展字段（为未来功能预留）
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_datetime() -> DateTime<Utc> {
    Utc::now()
}

impl SatelliteInfo {
    /// 创建新的卫星信息
    pub fn new(name: String) -> Self {
        Self {
            name: name.clone(),
            catalog_number: None,
            status: SatelliteStatus::Unknown,
            aliases: vec![name.clone()],
            recent_reports: Vec::new(),
            last_updated: Utc::now(),
            last_fetch_success: None,
            is_active: false,
            metadata: HashMap::new(),
        }
    }
    
    /// 更新卫星报告
    pub fn update_reports(&mut self, reports: Vec<AmsatReport>) {
        self.recent_reports = reports;
        self.last_updated = Utc::now();
        self.last_fetch_success = Some(Utc::now());
        self.is_active = !self.recent_reports.is_empty();
    }
    
    /// 标记获取失败
    pub fn mark_fetch_failed(&mut self) {
        self.last_updated = Utc::now();
        // 不更新 last_fetch_success
    }
    
    /// 检查是否需要更新
    pub fn needs_update(&self, interval_minutes: i64) -> bool {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.last_updated);
        duration.num_minutes() >= interval_minutes
    }
}

/// 卫星列表导出格式（用于本地文件）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteListExport {
    /// 导出时间
    pub exported_at: DateTime<Utc>,
    
    /// 活跃卫星数量
    pub active_count: usize,
    
    /// 总卫星数量
    pub total_count: usize,
    
    /// 卫星列表（简化版）
    pub satellites: Vec<SatelliteEntry>,
}

/// 卫星条目（简化版）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteEntry {
    /// 主名称
    pub name: String,
    
    /// 别名
    pub aliases: Vec<String>,
    
    /// 是否活跃
    pub is_active: bool,
    
    /// 状态
    pub status: SatelliteStatus,
}

/// AMSAT 已知卫星名称列表（从API文档获取）
pub const KNOWN_SATELLITES: &[&str] = &[
    "AISAT-1", "AO-123", "AO-16", "AO-27", "AO-73", "AO-7[A]", "AO-7[B]", 
    "AO-85", "AO-91", "CAS-2T", "CAS-4A", "CAS-4B", "CatSat", "CUTE-1", 
    "DSTAR1", "DUCHIFAT1", "DUCHIFAT3", "EO-79", "EO-80", "ESEO", 
    "FloripaSat-1", "FO-118[H/u]", "FO-118[V/u+FM]", "FO-118[V/u]", "FO-29", 
    "FO-99", "GO-32", "HA-1", "HO-107", "HO-113", "IO-117", "IO-26", "IO-86", 
    "ISS-DATA", "ISS-DATV", "ISS-FM", "ISS-SSTV", "JO-97", "K2SAT", 
    "LEDSAT", "LilacSat-2", "LO-19", "LO-87", "LO-90", "LO-93", "MO-122", 
    "NO-44", "NO-45", "OUFTI-1",
];
