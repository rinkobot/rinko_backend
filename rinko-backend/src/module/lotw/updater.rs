///! LoTW queue status updater
///!
///! Fetches the ARRL LoTW queue status HTML, parses it,
///! renders it to a PNG, and caches the latest snapshot in memory.

use anyhow::{Context, Result};
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::parser::parse_lotw_html;
use super::types::LotwQueueSnapshot;
use crate::module::renderer::LotwRenderer;

const LOTW_STATUS_URL: &str = "https://www.arrl.org/logbook-queue-status";
const DEFAULT_IMAGE_DIR: &str = "data/image_cache";

/// Shared LoTW updater – owns the HTTP client, renderer, and in-memory cache.
pub struct LotwUpdater {
    client:   Client,
    renderer: LotwRenderer,
    /// Most-recently-fetched snapshot (None until first successful fetch)
    snapshot: Arc<RwLock<Option<LotwQueueSnapshot>>>,
}

impl LotwUpdater {
    pub fn new(image_dir: Option<PathBuf>) -> Self {
        let image_dir = image_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_IMAGE_DIR));
        Self {
            client:   Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("Mozilla/5.0 Rinko-bot/1.0")
                .build()
                .expect("Failed to build reqwest client"),
            renderer: LotwRenderer::new(image_dir),
            snapshot: Arc::new(RwLock::new(None)),
        }
    }

    /// Return a handle to the shared snapshot (for read-only access from handlers).
    pub fn snapshot_handle(&self) -> Arc<RwLock<Option<LotwQueueSnapshot>>> {
        self.snapshot.clone()
    }

    /// Fetch → parse → render one cycle.  Returns the path to the PNG.
    pub async fn update(&self) -> Result<PathBuf> {
        tracing::info!("Fetching LoTW queue status from {}", LOTW_STATUS_URL);

        let html = self
            .client
            .get(LOTW_STATUS_URL)
            .send()
            .await
            .context("Failed to GET LoTW queue status page")?
            .text()
            .await
            .context("Failed to read LoTW response body")?;

        let snapshot = parse_lotw_html(&html)
            .context("Failed to parse LoTW HTML")?;

        tracing::info!(
            "LoTW snapshot: {} rows, fetched at {}",
            snapshot.rows.len(),
            snapshot.fetched_at
        );

        let png_path = self
            .renderer
            .render(&snapshot)
            .await
            .context("Failed to render LoTW PNG")?;

        // Update in-memory cache
        *self.snapshot.write().await = Some(snapshot);

        Ok(png_path)
    }
}