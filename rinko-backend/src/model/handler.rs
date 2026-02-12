///! Handles requests related to the model.
use rinko_common::proto::{UnifiedMessage, MessageResponse};
use regex::Regex;
use anyhow::Result;
use std::sync::Arc;

use super::sat::{SatelliteManager, SatelliteInfo};

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
            "sat" | "satellite" => self.amsat_query(args).await,
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
                content_type: rinko_common::proto::ContentType::Text as i32,
            });
        }
        
        // Search for satellites
        match self.satellite_manager.query_satellite(query).await {
            Ok(Some(sat_info)) => {
                let response_text = format_satellite_info(&sat_info);
                Ok(MessageResponse {
                    success: true,
                    message: response_text,
                    message_id: uuid::Uuid::now_v7().to_string(),
                    content_type: rinko_common::proto::ContentType::Text as i32,
                })
            }
            Ok(None) => {
                // Try searching for similar satellites
                match self.satellite_manager.search_satellites(query).await {
                    Ok(results) if !results.is_empty() => {
                        let suggestions: Vec<String> = results
                            .iter()
                            .take(5)
                            .map(|s| s.name.clone())
                            .collect();
                        
                        Ok(MessageResponse {
                            success: false,
                            message: format!(
                                "Satellite '{}' not found. Did you mean: {}",
                                query,
                                suggestions.join(", ")
                            ),
                            message_id: uuid::Uuid::now_v7().to_string(),
                            content_type: rinko_common::proto::ContentType::Text as i32,
                        })
                    }
                    _ => {
                        Ok(MessageResponse {
                            success: false,
                            message: format!("Satellite '{}' not found in database.", query),
                            message_id: uuid::Uuid::now_v7().to_string(),
                            content_type: rinko_common::proto::ContentType::Text as i32,
                        })
                    }
                }
            }
            Err(e) => {
                Ok(MessageResponse {
                    success: false,
                    message: format!("Query failed: {}", e),
                    message_id: uuid::Uuid::now_v7().to_string(),
                    content_type: rinko_common::proto::ContentType::Text as i32,
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
    output.push_str(&format!("Status: {:?}\n", sat.status));
    output.push_str(&format!("Active: {}\n", if sat.is_active { "Yes" } else { "No" }));
    
    if let Some(last_fetch) = sat.last_fetch_success {
        output.push_str(&format!("Last Updated: {}\n", last_fetch.format("%Y-%m-%d %H:%M UTC")));
    }
    
    if !sat.aliases.is_empty() && sat.aliases.len() > 1 {
        output.push_str(&format!("Aliases: {}\n", sat.aliases.join(", ")));
    }
    
    if !sat.recent_reports.is_empty() {
        output.push_str(&format!("\nRecent Reports ({}):\n", sat.recent_reports.len()));
        for (i, report) in sat.recent_reports.iter().take(5).enumerate() {
            output.push_str(&format!(
                "  {}. {} - {} ({})\n",
                i + 1,
                report.reported_time,
                report.callsign,
                report.grid_square
            ));
            if !report.report.is_empty() {
                let report_preview = if report.report.len() > 80 {
                    format!("{}...", &report.report[..80])
                } else {
                    report.report.clone()
                };
                output.push_str(&format!("     {}\n", report_preview));
            }
        }
    } else {
        output.push_str("\nNo recent reports available.\n");
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
