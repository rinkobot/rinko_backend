///! V2 Data structures - NORAD ID based satellite system
///!
///! This module defines the new data structures based on NORAD ID as primary key.
///! Designed to integrate frequency database from GitHub and AMSAT status reports.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// NORAD ID type (satellite unique identifier)
pub type NoradId = u32;

///! Shared data types for satellite status management
///! 
///! These types are used by both V2 data structures and legacy components.
///! They represent the core AMSAT API data format.

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

// Report status tests are in the main tests block below


/// Frequency representation supporting various formats
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Frequency {
    /// Single frequency (e.g., "145.800")
    Single(f64),
    
    /// Frequency range (e.g., "145.850-145.950")
    Range { start: f64, end: f64 },
    
    /// Multiple frequencies (e.g., "435.400/436.210/10460.000")
    Multiple(Vec<f64>),
    
    /// No frequency specified
    None,
}

impl Frequency {
    /// Parse frequency from CSV string
    /// 
    /// Examples:
    /// - "145.800" → Single(145.800)
    /// - "145.850-145.950" → Range(145.850, 145.950)
    /// - "435.400/436.210" → Multiple([435.400, 436.210])
    /// - "" → None
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        
        if s.is_empty() {
            return Frequency::None;
        }
        
        // Check for range (contains '-')
        if s.contains('-') {
            let parts: Vec<&str> = s.split('-').collect();
            if parts.len() == 2 {
                if let (Ok(start), Ok(end)) = (parts[0].trim().parse(), parts[1].trim().parse()) {
                    return Frequency::Range { start, end };
                }
            }
        }
        
        // Check for multiple (contains '/')
        if s.contains('/') {
            let parts: Vec<f64> = s.split('/')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            if !parts.is_empty() {
                return Frequency::Multiple(parts);
            }
        }
        
        // Try single frequency
        if let Ok(freq) = s.parse() {
            return Frequency::Single(freq);
        }
        
        Frequency::None
    }
    
    /// Convert to display string
    pub fn to_display(&self) -> String {
        match self {
            Frequency::Single(f) => format!("{:.3} MHz", f),
            Frequency::Range { start, end } => format!("{:.3}-{:.3} MHz", start, end),
            Frequency::Multiple(freqs) => {
                freqs.iter()
                    .map(|f| format!("{:.3}", f))
                    .collect::<Vec<_>>()
                    .join("/") + " MHz"
            }
            Frequency::None => "N/A".to_string(),
        }
    }
    
    /// Check if frequency is specified
    pub fn is_some(&self) -> bool {
        !matches!(self, Frequency::None)
    }
}

impl Default for Frequency {
    fn default() -> Self {
        Frequency::None
    }
}

/// Transponder configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transponder {
    /// Transponder label (e.g., "FM", "SSTV", "Mode A")
    pub label: String,
    
    /// Uplink frequency (MHz)
    pub uplink: Frequency,
    
    /// Downlink frequency (MHz)
    pub downlink: Frequency,
    
    /// Beacon frequency (MHz)
    pub beacon: Frequency,
    
    /// Mode description (e.g., "FM tone 67.0Hz", "SSB", "1200bps AFSK")
    pub mode: String,
    
    /// Callsign(s)
    pub callsign: Option<String>,
    
    /// Whether this is the primary transponder
    #[serde(default)]
    pub is_primary: bool,
    
    /// AMSAT API name for this transponder (used to link status reports)
    /// Example: For ISS SSTV transponder -> "ISS SSTV"
    pub amsat_api_name: String,

    pub aliases: Vec<String>,
    
    /// SatNOGS ID
    pub satnogs_id: Option<String>,

    /// NORAD ID of the parent satellite (for reference)
    pub norad_id: Option<NoradId>,

    /// AMSAT Report
    pub amsat_report: Option<Vec<SatelliteDataBlock>>,

    /// Last update time
    pub last_updated: DateTime<Utc>,
    
    /// Last successful fetch time
    pub last_fetch_success_time: Option<DateTime<Utc>>,

    pub amsat_update_success: bool,
}

impl Transponder {
    /// Create a new transponder with label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            uplink: Frequency::None,
            downlink: Frequency::None,
            beacon: Frequency::None,
            mode: String::new(),
            callsign: None,
            is_primary: false,
            amsat_api_name: String::new(),
            aliases: Vec::new(),
            satnogs_id: None,
            norad_id: None,
            amsat_report: None,
            last_updated: Utc::now(),
            last_fetch_success_time: None,
            amsat_update_success: false
        }
    }
    
    /// Create a basic transponder with frequencies
    pub fn new_basic(label: impl Into<String>, uplink: Frequency, downlink: Frequency, norad_id: NoradId) -> Self {
        Self {
            label: label.into(),
            uplink,
            downlink,
            beacon: Frequency::None,
            mode: String::new(),
            callsign: None,
            is_primary: true,
            amsat_api_name: String::new(),
            aliases: Vec::new(),
            satnogs_id: None,
            norad_id: Some(norad_id),
            amsat_report: None,
            last_updated: Utc::now(),
            last_fetch_success_time: None,
            amsat_update_success: false
        }
    }
    
    /// Check if transponder has any frequency defined
    pub fn has_frequency(&self) -> bool {
        self.uplink.is_some() || self.downlink.is_some() || self.beacon.is_some()
    }

    pub fn total_reports(&self) -> usize {
        self.amsat_report.as_ref().map_or(0, |reports| reports.len())
    }
    
    /// Get latest status from reports
    pub fn latest_status(&self) -> ReportStatus {
        if let Some(ref reports) = self.amsat_report {
            if let Some(last_block) = reports.last() {
                if let Some(last_report) = last_block.reports.last() {
                    return ReportStatus::from_string(&last_report.report);
                }
            }
        }
        ReportStatus::Grey
    }
    
    /// Format frequencies for display
    pub fn format_frequencies(&self) -> String {
        let mut parts = Vec::new();
        
        if self.uplink.is_some() {
            parts.push(format!("↑{}", self.uplink.to_display()));
        }
        if self.downlink.is_some() {
            parts.push(format!("↓{}", self.downlink.to_display()));
        }
        if self.beacon.is_some() {
            parts.push(format!("📡{}", self.beacon.to_display()));
        }
        
        if parts.is_empty() {
            "N/A".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

/// Complete satellite information (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Satellite {
    /// NORAD ID (primary key)
    pub norad_id: NoradId,
    
    /// Common name (e.g., "ISS", "AO-91")
    pub common_name: String,
    
    /// All aliases
    pub aliases: Vec<String>,
    
    /// Transponder list
    pub transponders: Vec<Transponder>,
    
    /// Whether satellite is active
    pub is_active: bool,
    
    /// SatNOGS ID (optional)
    pub satnogs_id: Option<String>,
    
    /// Extended metadata
    pub metadata: HashMap<String, String>,
    
    /// Last successful AMSAT API fetch timestamp
    pub last_fetch_success: Option<DateTime<Utc>>,
    
    /// Whether AMSAT update was successful
    pub amsat_update_status: bool,
    
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

impl Satellite {
    /// Create a new satellite with NORAD ID
    pub fn new(norad_id: NoradId, common_name: impl Into<String>) -> Self {
        Self {
            norad_id,
            common_name: common_name.into(),
            aliases: Vec::new(),
            transponders: Vec::new(),
            is_active: true,
            satnogs_id: None,
            metadata: HashMap::new(),
            last_fetch_success: None,
            amsat_update_status: false,
            last_updated: Utc::now(),
        }
    }
    
    /// Get total number of reports across all transponders
    pub fn total_reports(&self) -> usize {
        self.transponders.iter()
            .filter_map(|t| t.amsat_report.as_ref())
            .map(|reports| reports.len())
            .sum()
    }
    
    /// Get primary transponder status (for quick status check)
    pub fn primary_status(&self) -> ReportStatus {
        if let Some(trans) = self.primary_transponder() {
            return trans.latest_status();
        }
        ReportStatus::Grey
    }
    
    /// Get all recent reports across all transponders
    pub fn get_all_recent_reports(&self, hours: i64) -> Vec<(String, Vec<SatelliteDataBlock>)> {
        let cutoff = Utc::now() - chrono::Duration::hours(hours);
        let mut results = Vec::new();
        
        for trans in &self.transponders {
            if let Some(ref reports) = trans.amsat_report {
                let filtered: Vec<_> = reports.iter()
                    .filter(|block| {
                        if let Ok(time) = DateTime::parse_from_rfc3339(&block.time) {
                            time.with_timezone(&Utc) >= cutoff
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect();
                
                if !filtered.is_empty() {
                    results.push((trans.label.clone(), filtered));
                }
            }
        }
        
        results
    }
    
    /// Check if satellite has recent data (within 48 hours)
    pub fn has_recent_data(&self) -> bool {
        for transponder in &self.transponders {
            if let Some(ref reports) = transponder.amsat_report {
                if let Some(last_report) = reports.last() {
                    let last_timestamp = &last_report.time;
                    // try to transform datelike string to utc
                    // Time block (e.g., "2026-02-16T08:00:00Z")
                    if let Ok(parsed_time) = DateTime::parse_from_rfc3339(last_timestamp) {
                        let last_utc = parsed_time.with_timezone(&Utc);
                        if (Utc::now() - last_utc).num_hours() <= 48 {
                            return true;
                        }
                    } else {
                        tracing::warn!("Failed to parse timestamp: {}", last_timestamp);
                    }
                }
            }
        }
        false
    }
    
    /// Get primary transponder
    pub fn primary_transponder(&self) -> Option<&Transponder> {
        self.transponders.iter().find(|t| t.is_primary)
            .or_else(|| self.transponders.first())
    }
    
    /// Get all AMSAT API names for this satellite
    pub fn get_amsat_api_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        
        for transponder in &self.transponders {
            if !transponder.amsat_api_name.is_empty() {
                names.push(transponder.amsat_api_name.clone());
            }
        }
        
        // If no transponder has API name, use common name
        if names.is_empty() {
            names.push(self.common_name.clone());
        }
        
        names
    }
}

/// Satellite registry (V2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteRegistry {
    /// Satellite configurations
    pub satellites: Vec<SatelliteConfig>,
}

impl Default for SatelliteRegistry {
    fn default() -> Self {
        Self {
            satellites: Vec::new(),
        }
    }
}

/// Satellite configuration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteConfig {
    /// NORAD ID
    pub norad_id: NoradId,
    
    /// Common name
    pub common_name: String,
    
    /// Aliases
    #[serde(default)]
    pub aliases: Vec<String>,
    
    /// Custom transponders (overrides CSV data if present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_transponders: Option<Vec<Transponder>>,
    
    /// Whether this satellite is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    /// Custom notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

fn default_enabled() -> bool {
    true
}

impl SatelliteConfig {
    /// Create new config
    pub fn new(norad_id: NoradId, common_name: impl Into<String>) -> Self {
        Self {
            norad_id,
            common_name: common_name.into(),
            aliases: Vec::new(),
            custom_transponders: None,
            enabled: true,
            notes: None,
        }
    }
}

/// CSV row from frequency database
#[derive(Debug, Clone, Deserialize)]
pub struct FrequencyCsvRow {
    pub name: String,
    pub norad_id: String,  // Will be parsed to u32
    pub uplink: String,
    pub downlink: String,
    pub beacon: String,
    pub mode: String,
    pub callsign: String,
    pub satnogs_id: String,
}

impl FrequencyCsvRow {
    /// Parse NORAD ID
    pub fn parse_norad_id(&self) -> Option<NoradId> {
        self.norad_id.trim().parse().ok()
    }
    
    /// Convert to Transponder
    /// Multiple rows with same NORAD ID but different modes become separate transponders
    pub fn to_transponder(&self) -> Transponder {
        let label = if !self.mode.is_empty() {
            // Try to extract mode label (e.g., "FM", "SSB", "SSTV")
            self.extract_mode_label()
        } else {
            "Default".to_string()
        };
        
        Transponder {
            label,
            uplink: Frequency::parse(&self.uplink),
            downlink: Frequency::parse(&self.downlink),
            beacon: Frequency::parse(&self.beacon),
            mode: self.mode.clone(),
            callsign: if self.callsign.is_empty() { None } else { Some(self.callsign.clone()) },
            is_primary: false, // Will be determined later
            amsat_api_name: String::new(), // Will be mapped later
            aliases: Vec::new(), // Will be mapped later
            satnogs_id: if self.satnogs_id.is_empty() { None } else { Some(self.satnogs_id.clone()) },
            norad_id: self.parse_norad_id(),
            amsat_report: None,
            last_updated: Utc::now(),
            last_fetch_success_time: None,
            amsat_update_success: false,
        }
    }
    
    /// Extract mode label from mode string
    fn extract_mode_label(&self) -> String {
        let mode = self.mode.trim().to_uppercase();
        
        // Common patterns
        if mode.contains("SSTV") {
            return "SSTV".to_string();
        }
        if mode.starts_with("FM") {
            return "FM".to_string();
        }
        if mode.contains("SSB") {
            return "SSB".to_string();
        }
        if mode.starts_with("CW") {
            return "CW".to_string();
        }
        if mode.contains("DIGIPEATER") || mode.contains("APRS") || mode.contains("PACKET") || mode.contains("DIGI") {
            return "Digipeater".to_string();
        }
        if mode.contains("DVB-S2") || mode.contains("DATV") {
            return "DATV".to_string();
        }
        
        // Mode A, B, C, etc.
        if mode.len() == 1 && mode.chars().next().unwrap().is_alphabetic() {
            return format!("Mode {}", mode);
        }
        
        // Fall back to first word or full mode
        mode.split_whitespace()
            .next()
            .unwrap_or("Default")
            .to_string()
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
    fn test_satellite_creation() {
        let sat = Satellite::new(25544, "ISS");
        assert_eq!(sat.norad_id, 25544);
        assert_eq!(sat.common_name, "ISS");
        assert!(sat.is_active);
        assert_eq!(sat.total_reports(), 0);
    }

    #[test]
    fn test_frequency_parse_single() {
        let freq = Frequency::parse("145.800");
        assert_eq!(freq, Frequency::Single(145.800));
        assert_eq!(freq.to_display(), "145.800 MHz");
    }

    #[test]
    fn test_frequency_parse_range() {
        let freq = Frequency::parse("145.850-145.950");
        assert_eq!(freq, Frequency::Range { start: 145.850, end: 145.950 });
        assert_eq!(freq.to_display(), "145.850-145.950 MHz");
    }

    #[test]
    fn test_frequency_parse_multiple() {
        let freq = Frequency::parse("435.400/436.210/10460.000");
        assert_eq!(freq, Frequency::Multiple(vec![435.400, 436.210, 10460.000]));
        assert!(freq.to_display().contains("435.400"));
    }

    #[test]
    fn test_frequency_parse_empty() {
        let freq = Frequency::parse("");
        assert_eq!(freq, Frequency::None);
        assert_eq!(freq.to_display(), "N/A");
    }

    #[test]
    fn test_transponder_has_frequency() {
        let mut trans = Transponder::new("FM");
        assert!(!trans.has_frequency());
        
        trans.downlink = Frequency::Single(145.800);
        assert!(trans.has_frequency());
    }

    #[test]
    fn test_csv_row_mode_label_extraction() {
        let row = FrequencyCsvRow {
            name: "ISS".to_string(),
            norad_id: "25544".to_string(),
            uplink: "".to_string(),
            downlink: "145.800".to_string(),
            beacon: "".to_string(),
            mode: "SSTV".to_string(),
            callsign: "".to_string(),
            satnogs_id: "".to_string(),
        };
        
        assert_eq!(row.extract_mode_label(), "SSTV");
    }
}
