use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Port to bind the server to
    #[serde(default = "default_port")]
    pub port: u16,

    /// Whether to bind to all interfaces (0.0.0.0) or just localhost
    #[serde(default = "default_bind_all")]
    pub bind_all: bool,

    /// Directory containing media files to serve
    pub media_directory: String,

    /// URL prefix for media files (e.g., "media" -> /media/filename.png)
    #[serde(default = "default_url_prefix")]
    pub url_prefix: String,

    /// Domain name for constructing full URLs (optional)
    #[serde(default)]
    pub domain: Option<String>,

    /// Enable CORS for cross-origin requests
    #[serde(default = "default_enable_cors")]
    pub enable_cors: bool,
}

fn default_port() -> u16 {
    3030
}

fn default_bind_all() -> bool {
    true
}

fn default_url_prefix() -> String {
    "media".to_string()
}

fn default_enable_cors() -> bool {
    true
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file '{}': {}", path, e))?;
        
        let config: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file: {}", e))?;

        Ok(config)
    }
}
