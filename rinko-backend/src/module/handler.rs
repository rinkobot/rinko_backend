///! Handles requests related to the model.
use rinko_common::proto::{UnifiedMessage, MessageResponse, ContentType};
use regex::Regex;
use anyhow::Result;
use std::sync::Arc;

use super::sat::{SatelliteManager, SatelliteInfo, SatelliteRenderer};

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
                message: "Please provide a satellite name to query. Example: /q AO-91".to_string(),
                message_id: uuid::Uuid::now_v7().to_string(),
                content_type: ContentType::Text as i32,
            });
        }
        
        // Search for satellites
        let satellites = self.satellite_manager.search_satellites(query).await?;
        
        if satellites.is_empty() {
            return Ok(MessageResponse {
                success: false,
                message: format!("Satellite '{}' not found. Try searching by name or alias.", query),
                message_id: uuid::Uuid::now_v7().to_string(),
                content_type: ContentType::Text as i32,
            });
        }
        
        // Limit to 5 satellites per query
        let limited_satellites: Vec<_> = satellites.into_iter().take(5).collect();
        
        // Try to render as image
        let cache_dir = self.satellite_manager.cache_dir();
        // image_path is like: cache_dir/../iamge_cache/sat_name.png
        let images_dir = cache_dir.join("image_cache");
        let renderer = SatelliteRenderer::new(&images_dir);
        
        match renderer.render_satellites(&limited_satellites).await {
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
                
                let response_text = if limited_satellites.len() == 1 {
                    format_satellite_info(&limited_satellites[0])
                } else {
                    format_multiple_satellites(&limited_satellites)
                };
                
                Ok(MessageResponse {
                    success: true,
                    message: response_text,
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

/// Format satellite information for display
fn format_satellite_info(sat: &SatelliteInfo) -> String {
    let mut output = String::new();
    
    output.push_str(&format!("🛰️ Satellite: {}\n", sat.name));
    output.push_str(&format!("Active: {}\n", if sat.is_active { "✓ Yes" } else { "✗ No" }));
    output.push_str(&format!(
        "Update Status: {}\n",
        if sat.amsat_update_status { "✓ OK" } else { "✗ Failed" }
    ));
    
    if let Some(catalog_num) = &sat.catalog_number {
        output.push_str(&format!("Catalog: {}\n", catalog_num));
    }
    
    if let Some(last_fetch) = sat.last_fetch_success {
        output.push_str(&format!("Last Updated: {}\n", last_fetch.format("%Y-%m-%d %H:%M UTC")));
    }
    
    if !sat.aliases.is_empty() {
        output.push_str(&format!("Aliases: {}\n", sat.aliases.join(", ")));
    }
    
    let total_reports = sat.total_reports();
    output.push_str(&format!("\nTotal Reports: {}\n", total_reports));
    
    if !sat.data_blocks.is_empty() {
        output.push_str("\nRecent Time Blocks:\n");
        for (i, block) in sat.data_blocks.iter().take(3).enumerate() {
            output.push_str(&format!(
                "  {}. {} ({} reports)\n",
                i + 1,
                block.time,
                block.reports.len()
            ));
        }
    }
    
    output
}

/// Format multiple satellites for display
fn format_multiple_satellites(satellites: &[SatelliteInfo]) -> String {
    let mut output = String::new();
    
    output.push_str(&format!("🛰️ Found {} satellites:\n\n", satellites.len()));
    
    for (i, sat) in satellites.iter().enumerate() {
        output.push_str(&format!(
            "{}. {} - {} (Reports: {})\n",
            i + 1,
            sat.name,
            if sat.is_active { "Active" } else { "Inactive" },
            sat.total_reports()
        ));
    }
    
    output
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
