///! Satellite status management module
///!
///! This module provides comprehensive satellite status tracking from AMSAT API.
///!
///! ## Architecture (Dual-Store)
///! - Primary: AMSAT entries (keyed by API name, e.g. "ISS-FM", "AO-91")
///! - Secondary: FrequencyDatabase (GitHub CSV metadata, read-only reference)
///!
///! ## Main Components
///! - `SatelliteManager`: Core manager with dual-store architecture
///! - `SatelliteRenderer`: Image generation from satellite data
///! - `AmsatEntry`: Primary data unit for user queries

// ============ Core Data Structures ============
mod types;
pub use types::*;

// ============ AMSAT Entry Types ============
mod amsat_types;
pub use amsat_types::{
    AmsatEntry, ParsedAmsatName, parse_amsat_name, normalize_for_search,
    find_matching_transponder_index,
};

// ============ Data Source Management ============
mod frequency_db;
pub use frequency_db::{FrequencyDatabase, DatabaseStats, download_csv_from_github};

mod name_mapper;
pub use name_mapper::{NameMapper, NameMappingConfig, MappingReport, MappingStats};

// ============ Core Manager ============
mod manager;
pub use manager::{
    SatelliteManager, UpdateReport,
    AmsatSearchResult, AmsatMatchType, TransponderMetadata,
};

// ============ Search Engine (legacy, kept for compatibility) ============
mod search;
pub use search::{
    search_transponders, search_with_keywords, search_multiple,
    SearchResult, MatchType, filter_by_transponder, get_active_satellites,
};

// ============ Rendering System ============
mod renderer;
pub use renderer::SatelliteRenderer;

// ============ API Client and Scraper ============
mod api_client;
mod scraper;
pub use scraper::SatelliteScraper;

// ============ Cache Management ============
mod cache;
pub use cache::cleanup_old_images;

// ============ Updater ============
mod updater;
pub use updater::{SatelliteUpdater, start_satellite_updater};
