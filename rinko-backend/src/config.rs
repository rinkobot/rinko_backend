use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(default = "default_host")]
    pub host: String,
    
    #[serde(default = "default_port")]
    pub port: u16,
    
    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_media_server_url")]
    pub media_server_url: Option<String>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    50051
}

fn default_media_server_url() -> Option<String> {
    None
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            log_level: default_log_level(),
            media_server_url: default_media_server_url(),
        }
    }
}

pub static CONFIG: OnceLock<BackendConfig> = OnceLock::new();

pub fn read_config() -> anyhow::Result<()> {
    let path = "../config.toml";
    let config_str = std::fs::read_to_string(path)?;
    let config: BackendConfig = match toml::from_str(&config_str) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to parse config file {}: {}", path, e);
            panic!()
        }
    };

    CONFIG.set(config.clone()).unwrap();

    Ok(())
}

impl BackendConfig {
    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
