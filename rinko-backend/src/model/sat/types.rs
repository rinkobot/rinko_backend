///! Core data structures for satellite status management
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Satellite report from AMSAT API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmsatReport {
    pub name: String,
    pub reported_time: String,  // RFC3339 format
    pub callsign: String,
    pub report: String,
    pub grid_square: String,
}

impl Default for AmsatReport {
    fn default() -> Self {
        Self {
            name: String::new(),
            reported_time: String::new(),
            callsign: String::new(),
            report: ReportStatus::Grey.to_string(),
            grid_square: String::new(),
        }
    }
}

/// Report status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReportStatus {
    Blue,    // Transponder/Repeater active
    Yellow,  // Beacon/Telemetry only
    Orange,  // Conflicting reports
    Red,     // No signal
    Purple,  // ISS Crew voice active
    Grey,    // Unknown status
}

impl ReportStatus {
    /// Convert to user-friendly string
    pub fn to_string(&self) -> String {
        match self {
            ReportStatus::Blue => "Transponder/Repeater active".to_string(),
            ReportStatus::Yellow => "Telemetry/Beacon only".to_string(),
            ReportStatus::Orange => "Conflicting reports".to_string(),
            ReportStatus::Red => "No signal".to_string(),
            ReportStatus::Purple => "ISS Crew (Voice) Active".to_string(),
            ReportStatus::Grey => "Unknown status".to_string(),
        }
    }

    /// Convert to report format string
    pub fn to_report_format(&self) -> String {
        match self {
            ReportStatus::Blue => "Heard".to_string(),
            ReportStatus::Yellow => "Telemetry Only".to_string(),
            ReportStatus::Red => "Not Heard".to_string(),
            ReportStatus::Purple => "Crew Active".to_string(),
            _ => "Unknown status".to_string(),
        }
    }

    /// Parse from string
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "heard" => ReportStatus::Blue,
            "telemetry only" => ReportStatus::Yellow,
            "conflicting reports" => ReportStatus::Orange,
            "not heard" => ReportStatus::Red,
            "crew active" => ReportStatus::Purple,
            _ => ReportStatus::Grey,
        }
    }

    /// Convert to hex color for rendering
    pub fn to_color_hex(&self) -> &'static str {
        match self {
            ReportStatus::Blue => "#4297f3ff",
            ReportStatus::Yellow => "#f3cd36ff",
            ReportStatus::Orange => "#f97316",
            ReportStatus::Red => "#ed3f3fff",
            ReportStatus::Purple => "#946af5ff",
            ReportStatus::Grey => "#6b7280",
        }
    }

    /// Get color from string status
    pub fn string_to_color_hex(status: &str) -> &'static str {
        Self::from_string(status).to_color_hex()
    }
}

/// Satellite data block (one hour block)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteDataBlock {
    pub time: String,                   // Time block (e.g., "2026-02-16T08:00:00Z")
    pub reports: Vec<AmsatReport>,      // Reports for this time block
}

/// Satellite complete information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteInfo {
    pub name: String,
    pub aliases: Vec<String>,
    pub catalog_number: Option<String>,        // International satellite catalog number
    pub data_blocks: Vec<SatelliteDataBlock>,  // Reports grouped by hour
    pub last_updated: DateTime<Utc>,
    pub last_fetch_success: Option<DateTime<Utc>>,
    pub is_active: bool,                       // Active flag instead of deletion
    pub amsat_update_status: bool,             // Whether last AMSAT update succeeded
    pub metadata: HashMap<String, String>,     // Extension fields
}

impl Default for SatelliteInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            aliases: Vec::new(),
            catalog_number: None,
            data_blocks: Vec::new(),
            last_updated: Utc::now(),
            last_fetch_success: None,
            is_active: true,
            amsat_update_status: false,
            metadata: HashMap::new(),
        }
    }
}

impl SatelliteInfo {
    /// Create new satellite info with name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Check if satellite has recent data (within 48 hours)
    pub fn has_recent_data(&self) -> bool {
        if let Some(last_success) = self.last_fetch_success {
            (Utc::now() - last_success).num_hours() <= 48
        } else {
            false
        }
    }

    /// Get total number of reports
    pub fn total_reports(&self) -> usize {
        self.data_blocks.iter()
            .map(|block| block.reports.len())
            .sum()
    }
}

/// Satellite list configuration (stored in TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteList {
    pub satellites: Vec<SatelliteEntry>,
}

impl Default for SatelliteList {
    fn default() -> Self {
        Self {
            satellites: Vec::new(),
        }
    }
}

/// Satellite entry in configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteEntry {
    pub official_name: String,
    pub aliases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_number: Option<String>,
}

impl SatelliteEntry {
    pub fn new(official_name: impl Into<String>) -> Self {
        Self {
            official_name: official_name.into(),
            aliases: Vec::new(),
            catalog_number: None,
        }
    }
}

/// Update report summary
#[derive(Debug, Clone)]
pub struct UpdateReport {
    pub total_satellites: usize,
    pub successful_updates: usize,
    pub failed_updates: usize,
    pub new_satellites: Vec<String>,
    pub inactive_satellites: Vec<String>,
    pub duration_seconds: f64,
}

impl UpdateReport {
    pub fn new() -> Self {
        Self {
            total_satellites: 0,
            successful_updates: 0,
            failed_updates: 0,
            new_satellites: Vec::new(),
            inactive_satellites: Vec::new(),
            duration_seconds: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_status_conversion() {
        assert_eq!(ReportStatus::from_string("heard"), ReportStatus::Blue);
        assert_eq!(ReportStatus::from_string("Heard"), ReportStatus::Blue);
        assert_eq!(ReportStatus::from_string("not heard"), ReportStatus::Red);
        assert_eq!(ReportStatus::from_string("unknown"), ReportStatus::Grey);
    }

    #[test]
    fn test_report_status_color() {
        assert_eq!(ReportStatus::Blue.to_color_hex(), "#4297f3ff");
        assert_eq!(ReportStatus::Red.to_color_hex(), "#ed3f3fff");
    }

    #[test]
    fn test_satellite_info_creation() {
        let sat = SatelliteInfo::new("AO-91");
        assert_eq!(sat.name, "AO-91");
        assert!(sat.is_active);
        assert_eq!(sat.total_reports(), 0);
    }
}
