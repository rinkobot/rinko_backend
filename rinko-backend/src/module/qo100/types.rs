///! QO-100 DX Cluster data types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One row (spot) from the QO-100 DX Cluster table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qo100Spot {
    /// Date/Time string, e.g. "2026-02-18 14:21"
    pub datetime: String,
    /// DX callsign, e.g. "PY5ZUE/P"
    pub dx: String,
    /// Frequency string, e.g. ".740" or "--"
    pub freq: String,
    /// Comment / info, e.g. "QO-100 HI24"
    pub comments: String,
    /// Spotter callsign, e.g. "OM0AAO"
    pub spotter: String,
    /// Source, e.g. "DXCluster"
    pub source: String,
}

/// A full snapshot of the QO-100 DX Cluster page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qo100Snapshot {
    /// When this snapshot was fetched
    pub fetched_at: DateTime<Utc>,
    /// Parsed table rows (newest first, as on page)
    pub spots: Vec<Qo100Spot>,
}
