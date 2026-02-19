///! LoTW queue status data types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One row from the LoTW queue status table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotwQueueRow {
    /// Status epoch (UTC), e.g. "2026-02-18 04:59:02"
    pub epoch: String,
    /// Number of log files in the queue
    pub logs: u64,
    /// Number of QSOs in the queue
    pub qsos: u64,
    /// Total bytes in the queue
    pub bytes: u64,
    /// Timestamp of the log currently being processed (UTC)
    pub currently_processing: String,
    /// Latency string, e.g. "0d 00h 00m 59s ago"
    pub latency_text: String,
    /// Whether latency is considered "bad" (> threshold)
    pub latency_bad: bool,
}

/// A full snapshot of the LoTW queue status page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotwQueueSnapshot {
    /// When this snapshot was fetched
    pub fetched_at: DateTime<Utc>,
    /// Parsed table rows (newest first)
    pub rows: Vec<LotwQueueRow>,
}

impl LotwQueueSnapshot {
    /// Latency threshold in seconds above which a row is flagged "bad"
    pub const BAD_LATENCY_SECS: u64 = 10 * 60; // 10 minutes
}
