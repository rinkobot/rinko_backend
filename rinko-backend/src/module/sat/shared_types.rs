///! Shared data types for satellite status management
///! 
///! These types are used by both V2 data structures and legacy components.
///! They represent the core AMSAT API data format.

use serde::{Deserialize, Serialize};

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
/// 
/// Used to group AMSAT reports by time blocks for efficient storage and querying.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteDataBlock {
    pub time: String,                   // Time block (e.g., "2026-02-16T08:00:00Z")
    pub reports: Vec<AmsatReport>,      // Reports for this time block
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
