///! Frequency database - CSV parser and manager
///!
///! Loads and manages satellite frequency data from CSV file
///! Source: https://github.com/palewire/amateur-satellite-database

use super::types::{
    FrequencyCsvRow, NoradId, Satellite, SatelliteConfig, SatelliteRegistry, Transponder,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Frequency database manager
pub struct FrequencyDatabase {
    /// Satellites indexed by NORAD ID
    satellites: HashMap<NoradId, Satellite>,
    
    /// CSV file path
    csv_path: String,
}

impl FrequencyDatabase {
    /// Create a new frequency database
    pub fn new() -> Self {
        Self {
            satellites: HashMap::new(),
            csv_path: String::new(),
        }
    }
    
    /// Load from CSV file
    pub async fn load_from_csv(csv_path: impl AsRef<Path>) -> Result<Self> {
        let csv_path_str = csv_path.as_ref().to_string_lossy().to_string();
        tracing::info!("Loading frequency database from: {}", csv_path_str);
        
        let content = tokio::fs::read_to_string(&csv_path).await
            .context(format!("Failed to read CSV file: {}", csv_path_str))?;
        
        let mut db = Self {
            satellites: HashMap::new(),
            csv_path: csv_path_str,
        };
        
        db.parse_csv(&content)?;
        
        tracing::info!(
            "Loaded {} satellites from frequency database",
            db.satellites.len()
        );
        
        Ok(db)
    }
    
    /// Parse CSV content
    fn parse_csv(&mut self, content: &str) -> Result<()> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true) // Allow variable number of fields
            .from_reader(content.as_bytes());
        
        let mut row_count = 0;
        let mut error_count = 0;
        
        for result in reader.deserialize() {
            row_count += 1;
            
            match result {
                Ok(row) => {
                    if let Err(e) = self.process_csv_row(row) {
                        error_count += 1;
                        tracing::warn!("Error processing CSV row {}: {}", row_count, e);
                    }
                }
                Err(e) => {
                    error_count += 1;
                    tracing::warn!("Error parsing CSV row {}: {}", row_count, e);
                }
            }
        }
        
        tracing::debug!(
            "Processed {} CSV rows, {} errors",
            row_count,
            error_count
        );
        
        // Set primary transponders
        self.set_primary_transponders();
        
        Ok(())
    }
    
    /// Process a single CSV row
    fn process_csv_row(&mut self, row: FrequencyCsvRow) -> Result<()> {
        let norad_id = row.parse_norad_id()
            .context(format!("Invalid NORAD ID: {}", row.norad_id))?;
        
        let transponder = row.to_transponder();
        
        // Get or create satellite
        let satellite = self.satellites.entry(norad_id).or_insert_with(|| {
            Satellite::new(norad_id, row.name.clone())
        });
        
        // Add alias if different from common name
        if row.name != satellite.common_name && !satellite.aliases.contains(&row.name) {
            satellite.aliases.push(row.name.clone());
        }
        
        // Update SatNOGS ID if available
        if let Some(ref satnogs_id) = transponder.satnogs_id {
            if satellite.satnogs_id.is_none() {
                satellite.satnogs_id = Some(satnogs_id.clone());
            }
        }
        
        // Add transponder (avoid duplicates)
        if !Self::has_duplicate_transponder(&satellite.transponders, &transponder) {
            satellite.transponders.push(transponder);
        }
        
        Ok(())
    }
    
    /// Check if transponder is duplicate
    fn has_duplicate_transponder(existing: &[Transponder], new: &Transponder) -> bool {
        existing.iter().any(|t| {
            t.uplink == new.uplink
                && t.downlink == new.downlink
                && t.mode == new.mode
        })
    }
    
    /// Set primary transponders (first one with downlink)
    fn set_primary_transponders(&mut self) {
        for satellite in self.satellites.values_mut() {
            if let Some(primary) = satellite.transponders.iter_mut()
                .find(|t| t.downlink.is_some())
            {
                primary.is_primary = true;
            }
        }
    }
    
    /// Get satellite by NORAD ID
    pub fn get_satellite(&self, norad_id: NoradId) -> Option<&Satellite> {
        self.satellites.get(&norad_id)
    }
    
    /// Get all satellites
    pub fn get_all_satellites(&self) -> Vec<&Satellite> {
        self.satellites.values().collect()
    }
    
    /// Get satellites by name (fuzzy match)
    pub fn find_satellites_by_name(&self, name: &str) -> Vec<&Satellite> {
        let name_lower = name.to_lowercase();
        
        self.satellites.values()
            .filter(|sat| {
                sat.common_name.to_lowercase().contains(&name_lower)
                    || sat.aliases.iter().any(|a| a.to_lowercase().contains(&name_lower))
            })
            .collect()
    }
    
    /// Generate satellite registry from database
    pub fn to_registry(&self) -> SatelliteRegistry {
        let mut satellites: Vec<SatelliteConfig> = self.satellites.values()
            .map(|sat| SatelliteConfig {
                norad_id: sat.norad_id,
                common_name: sat.common_name.clone(),
                aliases: sat.aliases.clone(),
                custom_transponders: None,
                enabled: true,
                notes: None,
            })
            .collect();
        
        // Sort by NORAD ID
        satellites.sort_by_key(|s| s.norad_id);
        
        SatelliteRegistry { satellites }
    }
    
    /// Get statistics
    pub fn stats(&self) -> DatabaseStats {
        let total_satellites = self.satellites.len();
        let total_transponders: usize = self.satellites.values()
            .map(|s| s.transponders.len())
            .sum();
        
        let satellites_with_multiple_transponders = self.satellites.values()
            .filter(|s| s.transponders.len() > 1)
            .count();
        
        DatabaseStats {
            total_satellites,
            total_transponders,
            satellites_with_multiple_transponders,
        }
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_satellites: usize,
    pub total_transponders: usize,
    pub satellites_with_multiple_transponders: usize,
}

impl std::fmt::Display for DatabaseStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Satellites: {}, Transponders: {}, Multi-transponder satellites: {}",
            self.total_satellites,
            self.total_transponders,
            self.satellites_with_multiple_transponders
        )
    }
}

/// Download CSV from GitHub
pub async fn download_csv_from_github(output_path: impl AsRef<Path>) -> Result<()> {
    const CSV_URL: &str = "https://raw.githubusercontent.com/palewire/amateur-satellite-database/main/data/amsat-active-frequencies.csv";
    
    tracing::info!("Downloading frequency database from GitHub...");
    
    let response = reqwest::get(CSV_URL).await
        .context("Failed to download CSV from GitHub")?;
    
    if !response.status().is_success() {
        anyhow::bail!("HTTP error: {}", response.status());
    }
    
    let content = response.text().await
        .context("Failed to read response body")?;
    
    tokio::fs::write(&output_path, content).await
        .context("Failed to write CSV file")?;
    
    tracing::info!(
        "Downloaded frequency database to: {}",
        output_path.as_ref().display()
    );
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CSV: &str = r#"name,norad_id,uplink,downlink,beacon,mode,callsign,satnogs_id
ISS,25544,,145.800,,SSTV,,XSKZ-5603-1870-9019-3066
ISS,25544,,437.550,,1200bps AFSK SSTV,RS0ISS NA1SS,XSKZ-5603-1870-9019-3066
ISS,25544,145.825,145.825,,1200bps AFSK Digipeater,RS0ISS ARISS,XSKZ-5603-1870-9019-3066
AO-91,43017,435.250,145.960,145.960,FM* CTCSS 67.0Hz/200bps DUV,,PMAW-9203-2442-8666-3249
AO-7,7530,145.850-145.950,29.400-29.500,29.502,A,,HHSS-6325-1344-4603-7774
AO-7,7530,432.125-432.175,145.975-145.925,145.970,B C,,HHSS-6325-1344-4603-7774"#;

    #[test]
    fn test_parse_csv() {
        let mut db = FrequencyDatabase::new();
        db.parse_csv(SAMPLE_CSV).unwrap();
        
        let stats = db.stats();
        assert_eq!(stats.total_satellites, 3); // ISS, AO-91, AO-7
        assert!(stats.total_transponders >= 5);
    }

    #[test]
    fn test_iss_multiple_transponders() {
        let mut db = FrequencyDatabase::new();
        db.parse_csv(SAMPLE_CSV).unwrap();
        
        let iss = db.get_satellite(25544).unwrap();
        assert_eq!(iss.common_name, "ISS");
        assert!(iss.transponders.len() >= 3); // SSTV, AFSK SSTV, Digipeater
    }

    #[test]
    fn test_frequency_range_parsing() {
        let mut db = FrequencyDatabase::new();
        db.parse_csv(SAMPLE_CSV).unwrap();
        
        let ao7 = db.get_satellite(7530).unwrap();
        assert!(ao7.transponders.iter().any(|t| {
            matches!(t.uplink, super::super::types::Frequency::Range { .. })
        }));
    }

    #[test]
    fn test_find_by_name() {
        let mut db = FrequencyDatabase::new();
        db.parse_csv(SAMPLE_CSV).unwrap();
        
        let results = db.find_satellites_by_name("ISS");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].norad_id, 25544);
    }
}
