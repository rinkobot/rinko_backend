mod config;
mod service;

use anyhow::Result;
use tonic::transport::Server;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use rinko_common::proto::bot_backend_server::BotBackendServer;
use config::BackendConfig;
use service::BotBackendService;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = BackendConfig::from_file("config.toml")
        .unwrap_or_else(|_| {
            tracing::warn!("Failed to load config.toml, using defaults");
            BackendConfig::default()
        });

    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config.log_level.clone().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Rinko Backend starting...");
    tracing::info!("Server will listen on {}", config.server_address());

    // Create gRPC service
    let bot_service = BotBackendService::new();
    let server_addr = config.server_address().parse()?;

    tracing::info!("✓ gRPC server starting on {}", server_addr);

    // Start gRPC server
    Server::builder()
        .add_service(BotBackendServer::new(bot_service))
        .serve(server_addr)
        .await?;

    Ok(())
}
