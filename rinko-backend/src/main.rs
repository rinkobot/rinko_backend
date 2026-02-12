use rinko_backend::config;
use rinko_backend::service;
use rinko_backend::model::sat::{SatelliteManager, SatelliteUpdater};

use anyhow::Result;
use std::sync::Arc;
use tonic::transport::Server;

use rinko_common::proto::bot_backend_server::BotBackendServer;
use service::BotBackendService;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    config::read_config()?;
    let config = config::CONFIG.get().unwrap();

    // Initialize logging
    let _logging_guard = rinko_backend::logging::init_logging(
        "logs",
        "rinko-backend",
        &config.log_level,
    );

    tracing::info!("Rinko Backend starting...");
    tracing::info!("Server will listen on {}", config.server_address());

    // Initialize satellite manager
    tracing::info!("Initializing satellite manager...");
    let cache_dir = "data/satellite_cache";
    let update_interval_minutes = 10; // 10 minutes
    
    let satellite_manager = Arc::new(
        SatelliteManager::new(cache_dir, update_interval_minutes as i64)?
    );
    
    // Initialize satellite manager (load cache)
    satellite_manager.initialize().await?;
    tracing::info!("Satellite manager initialized successfully");
    
    // Start satellite updater with initial update
    let updater = SatelliteUpdater::new(
        satellite_manager.clone(),
        update_interval_minutes,
    );
    
    let _updater_handle = updater.start_with_initial_update().await?;
    tracing::info!("Satellite updater started (interval: {} minutes)", update_interval_minutes);

    // Create gRPC service with satellite manager
    let bot_service = BotBackendService::new(satellite_manager);
    let server_addr = config.server_address().parse()?;

    tracing::info!("gRPC server starting on {}", server_addr);

    // Start gRPC server
    Server::builder()
        .add_service(BotBackendServer::new(bot_service))
        .serve(server_addr)
        .await?;

    Ok(())
}
