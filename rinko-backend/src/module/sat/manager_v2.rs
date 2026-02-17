use crate::module::sat::Transponder;

///! Satellite Manager V2 - NORAD ID based management
///!
///! Core business logic for managing satellites with NORAD ID as primary key

use super::{
    api_client, scraper,
    types_v2::{NoradId, Satellite},
    frequency_db::FrequencyDatabase,
    name_mapper::NameMapper,
};
use anyhow::Result;
use chrono::{DateTime, Duration, Timelike, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

const DATA_RETENTION_HOURS: i64 = 48;
const INACTIVE_THRESHOLD_HOURS: i64 = 168;
const API_REQUEST_DELAY_MS: u64 = 200;

/// Update report V2
#[derive(Debug, Clone)]
pub struct UpdateReportV2 {
    pub total_satellites: usize,
    pub successful_updates: usize,
    pub failed_updates: usize,
    pub new_satellites: Vec<NoradId>,
    pub inactive_satellites: Vec<NoradId>,
    pub duration_seconds: f64,
}

impl UpdateReportV2 {
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

impl Default for UpdateReportV2 {
    fn default() -> Self {
        Self::new()
    }
}

/// Satellite Manager V2
pub struct SatelliteManagerV2 {
    /// Satellites indexed by NORAD ID
    satellites: Arc<RwLock<HashMap<NoradId, Satellite>>>,
    
    /// Frequency database (CSV)
    frequency_db: Arc<RwLock<FrequencyDatabase>>,
    
    /// Name mapper (AMSAT API <-> CSV)
    name_mapper: Arc<RwLock<NameMapper>>,
    
    /// Cache directory
    cache_dir: PathBuf,
    
    /// Update interval (minutes)
    update_interval_minutes: i64,
}

impl SatelliteManagerV2 {
    /// Create a new manager with default CSV path
    /// CSV will be downloaded if not present
    pub async fn new(
        cache_dir: impl AsRef<Path>,
        update_interval_minutes: i64,
    ) -> Result<Arc<Self>> {
        let cache_dir_path = cache_dir.as_ref();
        let csv_path = cache_dir_path.join("amsat-active-frequencies.csv");
        
        // Download CSV if it doesn't exist
        if !csv_path.exists() {
            tracing::info!("Downloading CSV database...");
            super::frequency_db::download_csv_from_github(&csv_path).await?;
        }
        
        Self::new_with_csv(cache_dir, csv_path, update_interval_minutes).await
    }
    
    /// Create a new manager with custom CSV path
    pub async fn new_with_csv(
        cache_dir: impl AsRef<Path>,
        csv_path: impl AsRef<Path>,
        update_interval_minutes: i64,
    ) -> Result<Arc<Self>> {
        let cache_dir = cache_dir.as_ref().to_path_buf();
        
        // Load frequency database
        let frequency_db = FrequencyDatabase::load_from_csv(csv_path).await?;
        
        // Create name mapper
        let name_mapper = NameMapper::with_defaults();
        
        Ok(Arc::new(Self {
            satellites: Arc::new(RwLock::new(HashMap::new())),
            frequency_db: Arc::new(RwLock::new(frequency_db)),
            name_mapper: Arc::new(RwLock::new(name_mapper)),
            cache_dir,
            update_interval_minutes,
        }))
    }
    
    /// Initialize manager - load cache and build mappings
    pub async fn initialize(&self) -> Result<()> {
        tracing::info!("Initializing satellite manager V2...");
        
        // Load V2 cache if exists
        let cache_path = self.cache_dir.join("satellite_cache_v2.json");
        
        if cache_path.exists() {
            tracing::info!("Loading V2 cache from: {}", cache_path.display());
            let content = tokio::fs::read_to_string(&cache_path).await?;
            let cached: HashMap<NoradId, Satellite> = serde_json::from_str(&content)?;
            
            *self.satellites.write().await = cached;
            tracing::info!("Loaded {} satellites from V2 cache", self.satellites.read().await.len());
        } else {
            // Initialize from frequency database
            tracing::info!("No V2 cache found, initializing from frequency database");
            let db = self.frequency_db.read().await;
            let satellites = db.get_all_satellites()
                .iter()
                .map(|sat| (sat.norad_id, (*sat).clone()))
                .collect();
            
            *self.satellites.write().await = satellites;
            tracing::info!("Initialized {} satellites from CSV", self.satellites.read().await.len());
        }
        
        // Fetch AMSAT satellite list and build mappings
        self.update_name_mappings().await?;
        
        Ok(())
    }
    
    /// Update name mappings from AMSAT API
    async fn update_name_mappings(&self) -> Result<()> {
        tracing::info!("Updating AMSAT name mappings...");
        
        let scraper = scraper::SatelliteScraper::new();
        let amsat_list = scraper.scrape_satellite_list().await?;
        
        let amsat_names: Vec<String> = amsat_list.satellites.iter()
            .map(|s| s.official_name.clone())
            .collect();
        
        let mut mapper = self.name_mapper.write().await;
        mapper.set_amsat_names(amsat_names.clone());
        
        tracing::info!("Updated name mapper with {} AMSAT satellites", amsat_names.len());
        
        Ok(())
    }
    
    /// Update all satellites
    pub async fn update_all_satellites(&self) -> Result<UpdateReportV2> {
        let start_time = std::time::Instant::now();
        let mut report = UpdateReportV2::new();
        
        tracing::info!("Starting satellite data update (V2)...");
        
        // Get all AMSAT API satellite names
        let amsat_names = {
            let mapper = self.name_mapper.read().await;
            mapper.stats().amsat_names_count
        };
        
        if amsat_names == 0 {
            tracing::warn!("No AMSAT names available, updating mappings first");
            self.update_name_mappings().await?;
        }
        
        // Get AMSAT API names from name mapper
        let amsat_api_names: Vec<String> = {
            let satellites = self.satellites.read().await;
            let mut names = Vec::new();
            for sat in satellites.values() {
                names.extend(sat.get_amsat_api_names());
            }
            
            // Also add unmapped names from mapper
            let mapper = self.name_mapper.read().await;
            for name in mapper.get_amsat_names().iter() {
                if !names.contains(name) {
                    names.push(name.clone());
                }
            }
            
            names
        };
        
        tracing::info!("Fetching data for {} AMSAT API names", amsat_api_names.len());
        
        // Fetch data from AMSAT API
        let fetch_results = api_client::batch_fetch_satellites(
            &amsat_api_names,
            1, // Last 1 hour
            API_REQUEST_DELAY_MS,
        ).await;
        
        // Process results and map to NORAD IDs
        let mut satellites = self.satellites.write().await;
        report.total_satellites = satellites.len();
        
        for (amsat_name, fetch_result) in fetch_results {
            // Map AMSAT name to NORAD ID
            let norad_id = {
                let mapper = self.name_mapper.read().await;
                self.find_norad_id_for_amsat_name(&amsat_name, &mapper).await
            };
            
            if let Some(norad_id) = norad_id {
                match self.update_single_satellite_v2(
                    norad_id,
                    &amsat_name,
                    satellites.get(&norad_id).cloned(),
                    Some(&fetch_result),
                ).await {
                    Ok(updated_sat) => {
                        // Check if satellite became inactive
                        let was_active = satellites.get(&norad_id).map_or(true, |s| s.is_active);
                        if !updated_sat.is_active && was_active {
                            report.inactive_satellites.push(norad_id);
                        }
                        
                        satellites.insert(norad_id, updated_sat);
                        report.successful_updates += 1;
                    }
                    Err(e) => {
                        tracing::error!("Failed to update NORAD {}: {}", norad_id, e);
                        report.failed_updates += 1;
                    }
                }
            } else {
                tracing::debug!("Could not map '{}' to NORAD ID", amsat_name);
            }
        }
        
        // Save cache
        self.save_cache().await?;
        
        report.duration_seconds = start_time.elapsed().as_secs_f64();
        
        tracing::info!(
            "Update complete: {} successful, {} failed in {:.2}s",
            report.successful_updates,
            report.failed_updates,
            report.duration_seconds
        );
        
        Ok(report)
    }
    
    /// Find NORAD ID for AMSAT API name
    async fn find_norad_id_for_amsat_name(
        &self,
        amsat_name: &str,
        _mapper: &NameMapper,
    ) -> Option<NoradId> {
        // Try existing satellites first
        let satellites = self.satellites.read().await;
        for sat in satellites.values() {
            if sat.common_name == amsat_name || sat.aliases.contains(&amsat_name.to_string()) {
                return Some(sat.norad_id);
            }
            
            // Check transponder AMSAT API names
            for trans in &sat.transponders {
                if trans.amsat_api_name.as_ref() == Some(&amsat_name.to_string()) {
                    return Some(sat.norad_id);
                }
            }
        }
        drop(satellites);
        
        // Try frequency database
        let db = self.frequency_db.read().await;
        let csv_sats = db.find_satellites_by_name(amsat_name);
        if let Some(sat) = csv_sats.first() {
            return Some(sat.norad_id);
        }
        
        None
    }
    
    /// Update a single satellite
    async fn update_single_satellite_v2(
        &self,
        norad_id: NoradId,
        amsat_api_name: &str,
        existing: Option<Satellite>,
        fetch_result: Option<&Result<Vec<super::types::AmsatReport>>>,
    ) -> Result<Satellite> {
        let mut sat = existing.unwrap_or_else(|| {
            // Try to get from frequency database - use try_read since we're in async
            // For now, just create a new satellite if not exists
            Satellite::new(norad_id, amsat_api_name)
        });
        
        // Process fetch result
        match fetch_result {
            Some(Ok(reports)) if !reports.is_empty() => {
                // Merge reports into satellite
                self.merge_reports_v2(&mut sat, amsat_api_name, reports).await?;
                sat.last_fetch_success = Some(Utc::now());
                sat.amsat_update_status = true;
            }
            Some(Err(e)) => {
                tracing::warn!("Fetch failed for {} (NORAD {}): {}", amsat_api_name, norad_id, e);
                sat.amsat_update_status = false;
            }
            _ => {
                sat.amsat_update_status = false;
            }
        }
        
        // Update activity status
        if let Some(last_success) = sat.last_fetch_success {
            let hours_since = (Utc::now() - last_success).num_hours();
            sat.is_active = hours_since <= INACTIVE_THRESHOLD_HOURS;
        }
        
        sat.last_updated = Utc::now();
        
        Ok(sat)
    }
    
    /// Merge reports into satellite
    async fn merge_reports_v2(
        &self,
        transponder: &mut Transponder,
        amsat_api_name: &str,
        new_reports: &[super::types::AmsatReport],
    ) -> Result<()> {
        use super::shared_types::SatelliteDataBlock;
        
        // Get or create report list for this AMSAT API name
        let data_blocks = match transponder.amsat_report {
            Some(ref mut blocks) => blocks,
            None => {
                transponder.amsat_report = Some(Vec::new());
                transponder.amsat_report.as_mut().unwrap()
            }
        };
        
        // Group reports by time block (hourly)
        let mut blocks_map: HashMap<String, Vec<super::types::AmsatReport>> = HashMap::new();
        
        for report in new_reports {
            let time_block = self.get_time_block(&report.reported_time)?;
            blocks_map.entry(time_block).or_insert_with(Vec::new).push(report.clone());
        }
        
        // Merge with existing blocks
        for (time, reports) in blocks_map {
            if let Some(existing_block) = data_blocks.iter_mut().find(|b| b.time == time) {
                // Merge reports (avoid duplicates)
                for report in reports {
                    if !existing_block.reports.iter().any(|r| {
                        r.callsign == report.callsign && r.reported_time == report.reported_time
                    }) {
                        existing_block.reports.push(report);
                    }
                }
            } else {
                // Create new block
                data_blocks.push(SatelliteDataBlock {
                    time: time.clone(),
                    reports,
                });
            }
        }
        
        // Sort blocks by time
        data_blocks.sort_by(|a, b| b.time.cmp(&a.time));
        
        // Clean old data
        self.clean_old_reports(data_blocks);
        
        Ok(())
    }
    
    /// Get time block (hourly) from timestamp
    fn get_time_block(&self, timestamp: &str) -> Result<String> {
        let dt = DateTime::parse_from_rfc3339(timestamp)?;
        let dt_utc = dt.with_timezone(&Utc);
        let time_block = dt_utc.with_minute(0).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap();
        Ok(time_block.to_rfc3339())
    }
    
    /// Clean old reports (keep last 48 hours)
    fn clean_old_reports(&self, data_blocks: &mut Vec<super::types::SatelliteDataBlock>) {
        let cutoff = Utc::now() - Duration::hours(DATA_RETENTION_HOURS);
        
        data_blocks.retain(|block| {
            if let Ok(block_time) = DateTime::parse_from_rfc3339(&block.time) {
                block_time.with_timezone(&Utc) >= cutoff
            } else {
                false
            }
        });
    }
    
    /// Save cache
    async fn save_cache(&self) -> Result<()> {
        let cache_path = self.cache_dir.join("satellite_cache_v2.json");
        let satellites = self.satellites.read().await;
        
        let json = serde_json::to_string_pretty(&*satellites)?;
        tokio::fs::write(&cache_path, json).await?;
        
        tracing::debug!("Saved {} satellites to cache", satellites.len());
        
        Ok(())
    }
    
    /// Get satellite by NORAD ID
    pub async fn get_satellite(&self, norad_id: NoradId) -> Option<Satellite> {
        self.satellites.read().await.get(&norad_id).cloned()
    }
    
    /// Get all satellites
    pub async fn get_all_satellites(&self) -> Vec<Satellite> {
        self.satellites.read().await.values().cloned().collect()
    }
    
    /// Get satellites by IDs
    pub async fn get_satellites_by_ids(&self, norad_ids: &[NoradId]) -> Vec<Satellite> {
        let satellites = self.satellites.read().await;
        norad_ids.iter()
            .filter_map(|id| satellites.get(id).cloned())
            .collect()
    }
    
    /// Search satellites by query (NORAD ID, name, alias)
    /// This is a convenience wrapper around search_satellites_v2
    pub async fn search_satellites(&self, query: &str) -> Vec<super::search_v2::SearchResult> {
        let satellites = self.get_all_satellites().await;
        super::search_v2::search_satellites_v2(query, &satellites, super::search_v2::DEFAULT_THRESHOLD)
    }
    
    /// Get active satellites (have data in last 7 days)
    pub async fn get_active_satellites(&self) -> Vec<Satellite> {
        let satellites = self.get_all_satellites().await;
        super::search_v2::get_active_satellites(&satellites)
    }
    
    /// Get cache directory
    pub fn cache_dir(&self) -> &std::path::Path {
        &self.cache_dir
    }
}
