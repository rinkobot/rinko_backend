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
///! - `SatelliteManager`: Core manager for satellite data
///! - `SatelliteUpdater`: Scheduled update task runner
///! - `SatelliteRenderer`: Image generation from satellite data

// Core types
mod types;
pub use types::{
    AmsatReport, ReportStatus, SatelliteDataBlock, SatelliteEntry, SatelliteInfo,
    SatelliteList, UpdateReport,
};

// API client and scraper
mod api_client;
mod scraper;

// Cache management
mod cache;

// Search engine
mod search;

// Core manager
mod manager;
pub use manager::SatelliteManager;

// Updater
mod updater;
pub use updater::{SatelliteUpdater, start_satellite_updater};

// Renderer
mod renderer;
pub use renderer::SatelliteRenderer;
