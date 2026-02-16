use axum::{
    routing::get,
    Router,
    http::StatusCode,
    response::IntoResponse,
};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tower_http::cors::{CorsLayer, Any};
use std::path::PathBuf;
use std::net::SocketAddr;
use tracing::{info, error};

mod config;
use config::Config;

/// Health check endpoint
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// Stats endpoint - returns basic server information
async fn stats() -> impl IntoResponse {
    let stats = serde_json::json!({
        "status": "running",
        "service": "rinko-media-server",
        "version": env!("CARGO_PKG_VERSION"),
    });
    (StatusCode::OK, serde_json::to_string(&stats).unwrap())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();

    // Load configuration
    let config = Config::load("config.toml")?;
    info!("Loaded configuration: {:?}", config);

    // Validate media directory exists
    let media_path = PathBuf::from(&config.media_directory);
    if !media_path.exists() {
        error!("Media directory does not exist: {}", config.media_directory);
        anyhow::bail!("Media directory not found: {}", config.media_directory);
    }

    info!("Serving media from: {}", media_path.display());

    // Setup CORS if enabled
    let cors = if config.enable_cors {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::permissive()
    };

    // Build the application routes
    let app = Router::new()
        // Health check endpoint
        .route("/health", get(health_check))
        // Stats endpoint
        .route("/stats", get(stats))
        // Static files service - serves files under /media/*
        .nest_service(
            &format!("/{}", config.url_prefix),
            ServeDir::new(&config.media_directory)
                .precompressed_br()
                .precompressed_gzip()
        )
        // Add middleware
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Bind to address
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Starting media server on http://{}:{}", 
          if config.bind_all { "0.0.0.0" } else { "127.0.0.1" }, 
          config.port);
    info!("Media URL pattern: http://{}:{}/{}/[filename]", 
          config.domain.as_deref().unwrap_or("localhost"),
          config.port,
          config.url_prefix);

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
