///! Satellite status management module
///! 
///! This module provides comprehensive satellite status tracking from AMSAT API.
///! 
///! ## Features
///! - Automatic satellite data updates
///! - Satellite search and querying
///! - File-based caching
///! - Hot-reloadable configuration
///! - SVG/PNG rendering
///! 
///! ## Main Components
///! - `SatelliteManager`: Core manager for satellite data (V2 - NORAD ID based)
///! - `SatelliteUpdater`: Scheduled update task runner
///! - `SatelliteRenderer`: Image generation from satellite data

// ============ Core Data Structures (V2) ============
mod types;
pub use types::*;

// ============ Data Source Management ============
mod frequency_db;
pub use frequency_db::{FrequencyDatabase, DatabaseStats, download_csv_from_github};

mod name_mapper;
pub use name_mapper::{NameMapper, NameMappingConfig, MappingReport, MappingStats};

// ============ Core Manager ============
mod manager;
pub use manager::{SatelliteManager, UpdateReport};

// ============ Search Engine ============
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
