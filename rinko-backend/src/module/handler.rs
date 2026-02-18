///! Handles requests related to the model.
use rinko_common::proto::{UnifiedMessage, MessageResponse, ContentType};
use regex::Regex;
use anyhow::Result;
use std::sync::Arc;

use super::sat::{SatelliteManager, SatelliteRenderer};

/// Message handler with satellite manager
pub struct MessageHandler {
    satellite_manager: Arc<SatelliteManager>,
}

impl MessageHandler {
    /// Create a new message handler
    pub fn new(satellite_manager: Arc<SatelliteManager>) -> Self {
        Self { satellite_manager }
    }
    
    /// Handle incoming message
    pub async fn handle_message(
        &self,
        msg: &UnifiedMessage,
    ) -> Result<MessageResponse> {
        let content = &msg.content;
        let message_id = uuid::Uuid::now_v7().to_string();

        match parse_command(&content) {
            Some((command, args)) => {
                if command.is_empty() {
                    // No command found
                    Ok(MessageResponse {
                        success: false,
                        message: "No command found, please provide a valid command prefix.".to_string(),
                        message_id,
                        content_type: rinko_common::proto::ContentType::Text as i32,
                    })
                } else {
                    // Route to appropriate handler
                    self.router(msg, &command, &args).await
                }
            },
            None => {
                // Invalid command format, return error response
                Ok(MessageResponse {
                    success: false,
                    message: "Command Parse failed, please check your input.".to_string(),
                    message_id,
                    content_type: rinko_common::proto::ContentType::Text as i32,
                })
            }
        }
    }
    
    /// Route commands to appropriate handlers
    async fn router(
        &self,
        _msg: &UnifiedMessage,
        command: &str,
        args: &str,
    ) -> Result<MessageResponse> {
        match command {
            "q" | "query" => self.amsat_query(args).await,
            "dxw" => {
                Ok(MessageResponse {
                    success: true,
                    message: "data/image_cache/dxw_latest.png".to_string(),
                    message_id: uuid::Uuid::now_v7().to_string(),
                    content_type: rinko_common::proto::ContentType::Image as i32,
                })
            }
            _ => {
                Ok(MessageResponse {
                    success: false,
                    message: format!("Unknown command: /{}", command),
                    message_id: uuid::Uuid::now_v7().to_string(),
                    content_type: rinko_common::proto::ContentType::Text as i32,
                })
            }
        }
    }
    
    /// Query satellite information
    async fn amsat_query(&self, query: &str) -> Result<MessageResponse> {
        let query = query.trim();
        
        if query.is_empty() {
            return Ok(MessageResponse {
                success: false,
                message: "Please provide a satellite name to query. Example: /q ISS".to_string(),
                message_id: uuid::Uuid::now_v7().to_string(),
                content_type: ContentType::Text as i32,
            });
        }
        
        // Search for AMSAT entries matching the query
        let search_results = self.satellite_manager.search_amsat_entries(query).await;
        
        if search_results.is_empty() {
            return Ok(MessageResponse {
                success: false,
                message: format!("^ ^)/"),
                message_id: uuid::Uuid::now_v7().to_string(),
                content_type: ContentType::Text as i32,
            });
        }
        
        // Try to render as image using renderer
        let cache_dir = self.satellite_manager.cache_dir();
        let images_dir = cache_dir.join("image_cache");

        // Make sure image cache directory exists
        tokio::fs::create_dir_all(&images_dir).await
            .expect("Failed to create image cache directory");

        let renderer = SatelliteRenderer::new(&images_dir);
        
        match renderer.render_amsat_results(&search_results, &self.satellite_manager).await {
            Ok(image_path) => {
                // Return image path
                let path_str = image_path.to_string_lossy().to_string();
                Ok(MessageResponse {
                    success: true,
                    message: format!("file:///{}", path_str.replace("\\", "/")),
                    message_id: uuid::Uuid::now_v7().to_string(),
                    content_type: ContentType::Image as i32,
                })
            }
            Err(e) => {
                // Fallback to text format if rendering fails
                tracing::warn!("Image rendering failed, falling back to text: {}", e);
                
                Ok(MessageResponse {
                    success: true,
                    message: "Rinko internal error >_".to_string(),
                    message_id: uuid::Uuid::now_v7().to_string(),
                    content_type: ContentType::Text as i32,
                })
            }
        }
    }
}

/// Parse command from message content
fn parse_command(content: &str) -> Option<(String, String)> {
    let re = Regex::new(r"^\s*/(\S+)\s*(.*)$").unwrap();
    if let Some(caps) = re.captures(content) {
        let command = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let args = caps.get(2).map_or("", |m| m.as_str()).to_string();
        Some((command, args))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_command() {
        assert_eq!(
            parse_command("/q AO-91"),
            Some(("q".to_string(), "AO-91".to_string()))
        );
        
        assert_eq!(
            parse_command("/query ISS"),
            Some(("query".to_string(), "ISS".to_string()))
        );
        
        assert_eq!(
            parse_command("  /sat   FO-29  "),
            Some(("sat".to_string(), "FO-29  ".to_string()))
        );
        
        assert_eq!(parse_command("no command here"), None);
    }
}
