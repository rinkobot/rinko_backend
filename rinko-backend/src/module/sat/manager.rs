///! Satellite status manager - Core business logic
use super::{
    api_client, cache, scraper, search,
    types::{
        AmsatReport, SatelliteDataBlock, SatelliteEntry, SatelliteInfo, SatelliteList,
        UpdateReport,
    },
};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Timelike, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

const DATA_RETENTION_HOURS: i64 = 48; // Keep 48 hours of data
const INACTIVE_THRESHOLD_HOURS: i64 = 168; // 7 days without data = inactive
const API_REQUEST_DELAY_MS: u64 = 200; // Delay between API requests

/// Satellite manager - main coordinator
pub struct SatelliteManager {
    satellites: Arc<RwLock<HashMap<String, SatelliteInfo>>>,
    satellite_list: Arc<RwLock<SatelliteList>>,
    cache_dir: PathBuf,
    update_interval_minutes: i64,
}

impl SatelliteManager {
    /// Create a new satellite manager
    pub fn new(cache_dir: impl AsRef<Path>, update_interval_minutes: i64) -> Result<Arc<Self>> {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        Ok(Arc::new(Self {
            satellites: Arc::new(RwLock::new(HashMap::new())),
            satellite_list: Arc::new(RwLock::new(SatelliteList::default())),
            cache_dir,
            update_interval_minutes,
        }))
    }

    /// Initialize manager - load cache and satellite list
    pub async fn initialize(&self) -> Result<()> {
        tracing::info!("Initializing satellite manager...");

        // Load satellite list
        let list = cache::load_satellite_list(&self.cache_dir)
            .await
            .context("Failed to load satellite list")?;

        // If list is empty, populate it with known satellites
        if list.satellites.is_empty() {
            tracing::info!("Satellite list is empty, fetching from AMSAT...");
            self.initialize_satellite_list().await?;
        } else {
            *self.satellite_list.write().await = list;
            tracing::info!(
                "Loaded {} satellites from configuration",
                self.satellite_list.read().await.satellites.len()
            );
        }

        // Load satellite cache
        let cached_satellites = cache::load_satellite_cache(&self.cache_dir)
            .await
            .context("Failed to load satellite cache")?;

        let mut satellites = self.satellites.write().await;
        for sat in cached_satellites {
            satellites.insert(sat.name.clone(), sat);
        }

        tracing::info!(
            "Loaded {} satellites from cache",
            satellites.len()
        );

        Ok(())
    }

    /// Initialize satellite list from AMSAT
    async fn initialize_satellite_list(&self) -> Result<()> {
        let sat_names = scraper::fetch_satellite_names_with_fallback().await;

        let mut list = SatelliteList::default();
        for name in sat_names {
            list.satellites.push(SatelliteEntry::new(name));
        }

        cache::save_satellite_list(&self.cache_dir, &list).await?;
        *self.satellite_list.write().await = list;

        tracing::info!(
            "Initialized satellite list with {} satellites",
            self.satellite_list.read().await.satellites.len()
        );

        Ok(())
    }

    /// Update all satellites (called by updater)
    pub async fn update_all_satellites(&self) -> Result<UpdateReport> {
        let start_time = std::time::Instant::now();
        let mut report = UpdateReport::new();

        tracing::info!("Starting satellite data update...");

        // Fetch latest satellite names from AMSAT
        let current_sat_names = scraper::fetch_satellite_names_with_fallback().await;

        // Update satellite list
        let mut list = self.satellite_list.write().await;
        for sat_name in &current_sat_names {
            if !list.satellites.iter().any(|s| &s.official_name == sat_name) {
                tracing::info!("New satellite discovered: {}", sat_name);
                list.satellites.push(SatelliteEntry::new(sat_name));
                report.new_satellites.push(sat_name.clone());
            }
        }
        report.total_satellites = list.satellites.len();

        // Save updated list
        cache::save_satellite_list(&self.cache_dir, &list).await?;

        // Collect satellite names to update
        let sat_names_to_update: Vec<String> = list
            .satellites
            .iter()
            .map(|s| s.official_name.clone())
            .collect();

        // Release the lock
        drop(list);

        // Fetch data for all satellites
        let fetch_results = api_client::batch_fetch_satellites(
            &sat_names_to_update,
            1, // Fetch last 1 hour
            API_REQUEST_DELAY_MS,
        )
        .await;

        // Update each satellite
        let mut satellites = self.satellites.write().await;
        for sat_name in sat_names_to_update {
            let fetch_result = fetch_results.get(&sat_name);

            let existing = satellites.get(&sat_name).cloned();
            let was_active = existing.as_ref().map_or(true, |s| s.is_active);
            match self.update_single_satellite(&sat_name, existing, fetch_result).await {
                Ok(updated_sat) => {
                    // Check if satellite became inactive
                    if !updated_sat.is_active && was_active {
                        report.inactive_satellites.push(sat_name.clone());
                    }

                    satellites.insert(sat_name, updated_sat);
                    report.successful_updates += 1;
                }
                Err(e) => {
                    tracing::error!("Failed to update {}: {}", sat_name, e);
                    report.failed_updates += 1;
                }
            }
        }

        // Save cache
        let sat_vec: Vec<SatelliteInfo> = satellites.values().cloned().collect();
        cache::save_satellite_cache(&self.cache_dir, &sat_vec).await?;

        report.duration_seconds = start_time.elapsed().as_secs_f64();

        tracing::info!(
            "Update complete: {} successful, {} failed in {:.2}s",
            report.successful_updates,
            report.failed_updates,
            report.duration_seconds
        );

        if !report.new_satellites.is_empty() {
            tracing::info!("New satellites: {:?}", report.new_satellites);
        }
        if !report.inactive_satellites.is_empty() {
            tracing::warn!("Inactive satellites: {:?}", report.inactive_satellites);
        }

        Ok(report)
    }

    /// Update a single satellite
    async fn update_single_satellite(
        &self,
        sat_name: &str,
        existing: Option<SatelliteInfo>,
        fetch_result: Option<&Result<Vec<AmsatReport>>>,
    ) -> Result<SatelliteInfo> {
        let mut info = existing.unwrap_or_else(|| SatelliteInfo::new(sat_name));

        // Copy aliases from satellite list
        let list = self.satellite_list.read().await;
        if let Some(entry) = list.satellites.iter().find(|s| s.official_name == sat_name) {
            info.aliases = entry.aliases.clone();
            info.catalog_number = entry.catalog_number.clone();
        }
        drop(list);

        // Process fetch result
        match fetch_result {
            Some(Ok(new_reports)) if !new_reports.is_empty() => {
                // Successful fetch
                info.data_blocks = Self::merge_reports(info.data_blocks, new_reports.clone());
                info.last_fetch_success = Some(Utc::now());
                info.amsat_update_status = true;
            }
            Some(Ok(_)) => {
                // Empty result
                info.amsat_update_status = true;
            }
            Some(Err(_)) | None => {
                // Fetch failed
                info.amsat_update_status = false;
            }
        }

        // Clean up old data (keep only last 48 hours)
        Self::clean_old_data(&mut info.data_blocks, DATA_RETENTION_HOURS);

        // Update metadata
        info.last_updated = Utc::now();

        // Determine if satellite is active
        info.is_active = if let Some(last_success) = info.last_fetch_success {
            (Utc::now() - last_success).num_hours() <= INACTIVE_THRESHOLD_HOURS
        } else {
            false
        };

        Ok(info)
    }

    /// Merge new reports into existing data blocks
    fn merge_reports(
        existing: Vec<SatelliteDataBlock>,
        new_reports: Vec<AmsatReport>,
    ) -> Vec<SatelliteDataBlock> {
        let mut grouped: BTreeMap<String, Vec<AmsatReport>> = BTreeMap::new();

        // Group existing reports
        for block in existing {
            grouped
                .entry(block.time.clone())
                .or_default()
                .extend(block.reports);
        }

        // Add new reports
        for report in new_reports {
            // Parse and normalize time to hour block
            if let Ok(datetime) = DateTime::parse_from_rfc3339(&report.reported_time) {
                let utc_time = datetime.with_timezone(&Utc);

                // Skip future reports (with 5 min tolerance)
                if utc_time > Utc::now() + Duration::minutes(5) {
                    continue;
                }

                // Round down to hour
                let hour_block = utc_time
                    .with_minute(0)
                    .unwrap()
                    .with_second(0)
                    .unwrap()
                    .with_nanosecond(0)
                    .unwrap()
                    .to_rfc3339();

                grouped.entry(hour_block).or_default().push(report);
            }
        }

        // Remove duplicates within each block (by callsign)
        for reports in grouped.values_mut() {
            let mut seen: HashSet<String> = HashSet::new();
            reports.retain(|report| {
                if seen.contains(&report.callsign) {
                    false
                } else {
                    seen.insert(report.callsign.clone());
                    true
                }
            });
        }

        // Convert back to Vec and sort by time (descending)
        let mut blocks: Vec<SatelliteDataBlock> = grouped
            .into_iter()
            .map(|(time, reports)| SatelliteDataBlock { time, reports })
            .collect();

        blocks.sort_by(|a, b| b.time.cmp(&a.time));

        blocks
    }

    /// Clean old data blocks (older than retention period)
    fn clean_old_data(blocks: &mut Vec<SatelliteDataBlock>, retention_hours: i64) {
        let cutoff = Utc::now() - Duration::hours(retention_hours);

        blocks.retain(|block| {
            if let Ok(block_time) = DateTime::parse_from_rfc3339(&block.time) {
                block_time.with_timezone(&Utc) >= cutoff
            } else {
                false
            }
        });
    }

    /// Query a single satellite by name
    pub async fn query_satellite(&self, name: &str) -> Result<Option<SatelliteInfo>> {
        // First try exact match
        let satellites = self.satellites.read().await;
        if let Some(sat) = satellites.get(name) {
            return Ok(Some(sat.clone()));
        }

        // Try searching
        let list = self.satellite_list.read().await;
        let matches = search::search_satellites(name, &list, search::DEFAULT_THRESHOLD);

        if let Some(first_match) = matches.first() {
            Ok(satellites.get(first_match).cloned())
        } else {
            Ok(None)
        }
    }

    /// Search for satellites (returns multiple matches)
    pub async fn search_satellites(&self, query: &str) -> Result<Vec<SatelliteInfo>> {
        let list = self.satellite_list.read().await;
        let matches = search::search_multiple(query, &list, search::DEFAULT_THRESHOLD);

        let satellites = self.satellites.read().await;
        let mut results = Vec::new();

        for sat_name in matches {
            if let Some(sat) = satellites.get(&sat_name) {
                results.push(sat.clone());
            }
        }

        Ok(results)
    }

    /// Get all active satellites
    pub async fn get_active_satellites(&self) -> Vec<SatelliteInfo> {
        let satellites = self.satellites.read().await;
        satellites
            .values()
            .filter(|s| s.is_active)
            .cloned()
            .collect()
    }

    /// Get all satellites (including inactive)
    pub async fn get_all_satellites(&self) -> Vec<SatelliteInfo> {
        let satellites = self.satellites.read().await;
        satellites.values().cloned().collect()
    }

    /// Reload satellite list from file (for hot reload)
    pub async fn reload_satellite_list(&self) -> Result<()> {
        tracing::info!("Reloading satellite list from file...");

        let list = cache::load_satellite_list(&self.cache_dir).await?;
        *self.satellite_list.write().await = list;

        tracing::info!(
            "Reloaded {} satellites from configuration",
            self.satellite_list.read().await.satellites.len()
        );

        Ok(())
    }

    /// Get update interval in minutes
    pub fn update_interval_minutes(&self) -> i64 {
        self.update_interval_minutes
    }

    /// Get cache directory path
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let temp_dir = std::env::temp_dir().join("rinko_test_manager");
        let manager = SatelliteManager::new(&temp_dir, 10).unwrap();
        assert_eq!(manager.update_interval_minutes(), 10);
    }

    #[tokio::test]
    async fn test_merge_reports() {
        let existing = vec![];
        let new_reports = vec![AmsatReport {
            name: "AO-91".to_string(),
            reported_time: "2026-02-16T08:15:00Z".to_string(),
            callsign: "BG2DNN".to_string(),
            report: "Heard".to_string(),
            grid_square: "OM89".to_string(),
        }];

        let merged = SatelliteManager::merge_reports(existing, new_reports);
        assert!(!merged.is_empty());
        assert_eq!(merged[0].reports. len(), 1);
    }
}
