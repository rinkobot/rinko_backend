use crate::module::sat::search::DEFAULT_THRESHOLD;

///! Name mapper - Maps CSV satellite names to AMSAT API names
///!
///! Handles the mapping between:
///! - CSV database names (e.g., "ISS")
///! - AMSAT API names (e.g., "ISS FM", "ISS SSTV", "ISS DATA", "ISS DATV")

use super::types::NoradId;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strsim::jaro_winkler;

/// Name mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameMappingConfig {
    /// Manual mappings (override fuzzy matching)
    /// Key: CSV name, Value: list of AMSAT API names
    #[serde(default)]
    pub manual_mappings: HashMap<String, Vec<String>>,
    
    /// Fuzzy matching threshold (0.0 - 1.0)
    #[serde(default = "default_threshold")]
    pub fuzzy_threshold: f64,
    
    /// NORAD ID to AMSAT API names
    /// Used to group multiple AMSAT names under one satellite
    #[serde(default)]
    pub norad_to_amsat_names: HashMap<NoradId, Vec<String>>,
}

fn default_threshold() -> f64 {
    DEFAULT_THRESHOLD
}

impl Default for NameMappingConfig {
    fn default() -> Self {
        Self {
            manual_mappings: Self::default_manual_mappings(),
            fuzzy_threshold: DEFAULT_THRESHOLD,
            norad_to_amsat_names: HashMap::new(),
        }
    }
}

impl NameMappingConfig {
    /// Default manual mappings for known problematic cases
    fn default_manual_mappings() -> HashMap<String, Vec<String>> {
        let mappings = HashMap::new();
        // TODO: Load default mappings from file
        mappings
    }
    
    /// Load from TOML file
    pub async fn load_from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let content = tokio::fs::read_to_string(&path).await?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// Save to TOML file
    pub async fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }
}

/// Name mapper
pub struct NameMapper {
    config: NameMappingConfig,
    
    /// AMSAT API satellite names (from scraper)
    amsat_names: Vec<String>,
    
    /// Cached fuzzy match results
    fuzzy_cache: HashMap<String, Vec<String>>,
}

impl NameMapper {
    /// Create a new name mapper
    pub fn new(config: NameMappingConfig) -> Self {
        Self {
            config,
            amsat_names: Vec::new(),
            fuzzy_cache: HashMap::new(),
        }
    }
    
    /// Create with default config
    pub fn with_defaults() -> Self {
        Self::new(NameMappingConfig::default())
    }
    
    /// Set AMSAT API names (from scraper)
    pub fn set_amsat_names(&mut self, names: Vec<String>) {
        self.amsat_names = names;
        self.fuzzy_cache.clear(); // Clear cache when names change
    }
    
    /// Map CSV name to AMSAT API names
    pub fn map_to_amsat(&mut self, csv_name: &str) -> Vec<String> {
        // Check manual mappings first
        if let Some(mapped) = self.config.manual_mappings.get(csv_name) {
            return mapped.clone();
        }
        
        // Check cache
        if let Some(cached) = self.fuzzy_cache.get(csv_name) {
            return cached.clone();
        }
        
        // Perform fuzzy matching
        let matches = self.fuzzy_match(csv_name);
        self.fuzzy_cache.insert(csv_name.to_string(), matches.clone());
        
        matches
    }
    
    /// Fuzzy match CSV name against AMSAT API names
    fn fuzzy_match(&self, csv_name: &str) -> Vec<String> {
        let mut matches = Vec::new();
        
        for amsat_name in &self.amsat_names {
            let similarity = jaro_winkler(
                &csv_name.to_lowercase(),
                &amsat_name.to_lowercase(),
            );
            
            if similarity >= self.config.fuzzy_threshold {
                matches.push(amsat_name.clone());
            }
        }
        
        // If no fuzzy match found, try exact prefix match
        if matches.is_empty() {
            let csv_lower = csv_name.to_lowercase();
            for amsat_name in &self.amsat_names {
                let amsat_lower = amsat_name.to_lowercase();
                if amsat_lower.starts_with(&csv_lower) || csv_lower.starts_with(&amsat_lower) {
                    matches.push(amsat_name.clone());
                }
            }
        }
        
        matches
    }
    
    /// Map NORAD ID to AMSAT API names
    pub fn map_norad_to_amsat(&self, norad_id: NoradId) -> Option<Vec<String>> {
        self.config.norad_to_amsat_names.get(&norad_id).cloned()
    }
    
    /// Add manual mapping
    pub fn add_manual_mapping(&mut self, csv_name: String, amsat_names: Vec<String>) {
        self.config.manual_mappings.insert(csv_name.clone(), amsat_names);
        self.fuzzy_cache.remove(&csv_name); // Invalidate cache
    }
    
    /// Associate NORAD ID with AMSAT API names
    pub fn associate_norad_with_amsat(&mut self, norad_id: NoradId, amsat_names: Vec<String>) {
        self.config.norad_to_amsat_names.insert(norad_id, amsat_names);
    }
    
    /// Get mapping statistics
    pub fn stats(&self) -> MappingStats {
        MappingStats {
            manual_mappings: self.config.manual_mappings.len(),
            cached_fuzzy_matches: self.fuzzy_cache.len(),
            amsat_names_count: self.amsat_names.len(),
            threshold: self.config.fuzzy_threshold,
        }
    }
    
    /// Get all unmapped CSV names (for diagnostics)
    pub fn find_unmapped(&mut self, csv_names: &[String]) -> Vec<String> {
        csv_names
            .iter()
            .filter(|name| self.map_to_amsat(name).is_empty())
            .cloned()
            .collect()
    }
    
    /// Generate mapping report
    pub fn generate_report(&mut self, csv_names: &[String]) -> MappingReport {
        let mut report = MappingReport::default();
        
        for csv_name in csv_names {
            let amsat_names = self.map_to_amsat(csv_name);
            
            if amsat_names.is_empty() {
                report.unmapped.push(csv_name.clone());
            } else if amsat_names.len() == 1 {
                report.one_to_one.push((csv_name.clone(), amsat_names[0].clone()));
            } else {
                report.one_to_many.push((csv_name.clone(), amsat_names));
            }
        }
        
        report
    }
    
    /// Save configuration to file
    pub async fn save_config(&self, path: impl AsRef<std::path::Path>) -> anyhow::Result<()> {
        self.config.save_to_file(path).await
    }
    
    /// Get AMSAT names list
    pub fn get_amsat_names(&self) -> &[String] {
        &self.amsat_names
    }
}

/// Mapping statistics
#[derive(Debug, Clone)]
pub struct MappingStats {
    pub manual_mappings: usize,
    pub cached_fuzzy_matches: usize,
    pub amsat_names_count: usize,
    pub threshold: f64,
}

impl std::fmt::Display for MappingStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Manual: {}, Cached: {}, AMSAT names: {}, Threshold: {:.2}",
            self.manual_mappings,
            self.cached_fuzzy_matches,
            self.amsat_names_count,
            self.threshold
        )
    }
}

/// Mapping report
#[derive(Debug, Clone, Default)]
pub struct MappingReport {
    pub one_to_one: Vec<(String, String)>,
    pub one_to_many: Vec<(String, Vec<String>)>,
    pub unmapped: Vec<String>,
}

impl MappingReport {
    /// Get total mapped count
    pub fn total_mapped(&self) -> usize {
        self.one_to_one.len() + self.one_to_many.len()
    }
    
    /// Get mapping coverage percentage
    pub fn coverage(&self) -> f64 {
        let total = self.total_mapped() + self.unmapped.len();
        if total == 0 {
            return 0.0;
        }
        (self.total_mapped() as f64 / total as f64) * 100.0
    }
    
    /// Print report
    pub fn print(&self) {
        println!("\n=== Name Mapping Report ===");
        println!("Total mapped: {}", self.total_mapped());
        println!("Unmapped: {}", self.unmapped.len());
        println!("Coverage: {:.1}%\n", self.coverage());
        
        if !self.one_to_many.is_empty() {
            println!("Multi-transponder satellites ({}):", self.one_to_many.len());
            for (csv_name, amsat_names) in &self.one_to_many {
                println!("  {} -> {:?}", csv_name, amsat_names);
            }
            println!();
        }
        
        if !self.unmapped.is_empty() {
            println!("Unmapped satellites ({}):", self.unmapped.len());
            for name in self.unmapped.iter().take(10) {
                println!("  {}", name);
            }
            if self.unmapped.len() > 10 {
                println!("  ... and {} more", self.unmapped.len() - 10);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manual_mapping() {
        let mut mapper = NameMapper::with_defaults();
        
        let mapped = mapper.map_to_amsat("ISS");
        assert!(mapped.len() >= 1);
        assert!(mapped.contains(&"ISS FM".to_string()));
    }

    #[test]
    fn test_fuzzy_matching() {
        let mut mapper = NameMapper::with_defaults();
        mapper.set_amsat_names(vec![
            "AO-91".to_string(),
            "Fox-1B".to_string(),
            "AO-7".to_string(),
        ]);
        
        let mapped = mapper.map_to_amsat("AO-91");
        assert!(mapped.contains(&"AO-91".to_string()));
    }

    #[test]
    fn test_add_manual_mapping() {
        let mut mapper = NameMapper::with_defaults();
        
        mapper.add_manual_mapping(
            "TEST-1".to_string(),
            vec!["TEST-1A".to_string(), "TEST-1B".to_string()],
        );
        
        let mapped = mapper.map_to_amsat("TEST-1");
        assert_eq!(mapped.len(), 2);
    }

    #[test]
    fn test_mapping_report() {
        let mut mapper = NameMapper::with_defaults();
        mapper.set_amsat_names(vec!["AO-91".to_string(), "ISS FM".to_string()]);
        
        let report = mapper.generate_report(&[
            "ISS".to_string(),
            "AO-91".to_string(),
            "UNKNOWN-SAT".to_string(),
        ]);
        
        assert!(report.total_mapped() >= 1);
        assert!(!report.unmapped.is_empty());
    }
}
