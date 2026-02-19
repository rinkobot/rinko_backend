use rinko_backend::config;
use rinko_backend::service;
use rinko_backend::module::sat::SatelliteManager;
use rinko_backend::module::scheduled::{ScheduledTaskManager, ScheduledTaskConfig};

use anyhow::Result;
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

    // Initialize satellite manager V2
    tracing::info!("Initializing satellite manager (V2)...");
    let cache_dir = "data";
    let update_interval_minutes = 10; // Update every 10 minutes
    
    let satellite_manager = SatelliteManager::new(cache_dir, update_interval_minutes as i64).await?;
    
    // Initialize satellite manager (load cache and configuration)
    satellite_manager.initialize().await?;
    tracing::info!("Satellite manager (V2) initialized successfully");
    
    // Configure and start scheduled tasks
    let task_config = ScheduledTaskConfig {
        satellite_update_interval_minutes: update_interval_minutes,
        lotw_update_interval_minutes: 60, // Update LoTW status every hour
        qo100_update_interval_minutes: 10, // Update QO-100 cluster every 10 minutes
        image_cleanup_interval_hours: 24, // Clean images daily
        image_retention_days: 1, // Keep images for 1 day
        cache_dir: cache_dir.to_string(),
        perform_initial_update: true, // Perform initial update immediately
    };
    
    let mut task_manager = ScheduledTaskManager::new(task_config, satellite_manager.clone());
    task_manager.start_all().await?;
    tracing::info!("All scheduled tasks started successfully");

    // Create gRPC service with satellite manager and LoTW updater
    let lotw_updater = task_manager.lotw_updater();
    let qo100_updater = task_manager.qo100_updater();
    let bot_service = BotBackendService::new(satellite_manager, lotw_updater, qo100_updater);
    let server_addr = config.server_address().parse()?;

    tracing::info!("gRPC server starting on {}", server_addr);

    // Start gRPC server
    Server::builder()
        .add_service(BotBackendServer::new(bot_service))
        .serve(server_addr)
        .await?;

    Ok(())
}
