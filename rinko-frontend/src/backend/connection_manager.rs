use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::backend::client::BackendClient;
use crate::config::BackendConfig;

/// Connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Connecting,
}

/// Backend connection manager with auto-reconnect
pub struct BackendConnectionManager {
    config: BackendConfig,
    client: Arc<RwLock<Option<BackendClient>>>,
    state: Arc<RwLock<ConnectionState>>,
}

impl BackendConnectionManager {
    /// Create a new connection manager
    pub fn new(config: BackendConfig) -> Self {
        Self {
            config,
            client: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
        }
    }

    /// Get the current client (if connected)
    pub fn client(&self) -> Arc<RwLock<Option<BackendClient>>> {
        self.client.clone()
    }

    /// Get the current connection state
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// Try to connect to backend
    async fn try_connect(&self) -> Result<BackendClient> {
        tracing::info!("Attempting to connect to backend at {}...", self.config.url);
        
        let client = BackendClient::new(
            &self.config.url,
            self.config.frontend_id.clone()
        ).await?;
        
        tracing::info!("Successfully connected to backend");
        Ok(client)
    }

    /// Initial connection attempt (non-blocking)
    pub async fn initialize(&self) {
        *self.state.write().await = ConnectionState::Connecting;
        
        match self.try_connect().await {
            Ok(client) => {
                *self.client.write().await = Some(client);
                *self.state.write().await = ConnectionState::Connected;
                tracing::info!("Backend connection established");
            }
            Err(e) => {
                *self.state.write().await = ConnectionState::Disconnected;
                tracing::warn!("Failed to connect to backend: {}. Will retry in background.", e);
            }
        }
    }

    /// Start auto-reconnect task
    pub fn start_reconnect_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let reconnect_interval = Duration::from_secs(10); // Retry every 10 seconds
            
            loop {
                sleep(reconnect_interval).await;
                
                // C-2 fix: check-and-set state atomically under a single write lock to
                // prevent TOCTOU races between reconnect, heartbeat and subscription tasks.
                {
                    let mut state = self.state.write().await;
                    if *state != ConnectionState::Disconnected {
                        continue;
                    }
                    *state = ConnectionState::Connecting;
                } // write lock released before the blocking connect call

                tracing::debug!("Backend disconnected, attempting reconnection...");

                match self.try_connect().await {
                    Ok(client) => {
                        *self.client.write().await = Some(client);
                        *self.state.write().await = ConnectionState::Connected;
                        tracing::info!("Backend reconnected successfully");
                    }
                    Err(e) => {
                        *self.state.write().await = ConnectionState::Disconnected;
                        tracing::debug!("Reconnection failed: {}. Will retry in {}s",
                            e, reconnect_interval.as_secs());
                    }
                }
            }
        });
    }

    /// Mark connection as disconnected (called when operations fail).
    ///
    /// C-2 fix: the state transition is guarded by a single write-lock acquire so that
    /// concurrent callers (heartbeat + subscription) cannot both observe `Connected` and
    /// then both redundantly clear the client.
    pub async fn mark_disconnected(&self) {
        {
            let mut state = self.state.write().await;
            if *state != ConnectionState::Connected {
                return;
            }
            *state = ConnectionState::Disconnected;
        } // release state lock before acquiring client lock to avoid potential deadlock

        tracing::warn!("Backend connection lost, entering offline mode");
        *self.client.write().await = None;
    }

    /// Start heartbeat task with auto-reconnect
    pub fn start_heartbeat_task(self: Arc<Self>) {
        let heartbeat_interval = self.config.heartbeat_interval;
        
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(heartbeat_interval)).await;
                
                let state = self.state.read().await.clone();
                
                if state == ConnectionState::Connected {
                    // C-1 fix: clone the client so we do NOT hold the write lock across the
                    // network round-trip.  BackendClient::clone() reuses the same underlying
                    // H2 channel, so this is cheap and creates no extra TCP connection.
                    let client_clone = self.client.read().await.clone();

                    if let Some(mut client) = client_clone {
                        let mut status = std::collections::HashMap::new();
                        status.insert("status".to_string(), "healthy".to_string());

                        match client.heartbeat(status).await {
                            Ok(response) => {
                                if response.healthy {
                                    tracing::debug!("Heartbeat sent successfully");
                                } else {
                                    tracing::warn!("Backend reported unhealthy: {}", response.message);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Heartbeat failed: {}", e);
                                self.mark_disconnected().await;
                            }
                        }
                    }
                }
            }
        });
    }

    /// Start command subscription task with auto-reconnect
    pub fn start_command_subscription_task(
        self: Arc<Self>,
        platforms: Vec<crate::utils::Platform>,
    ) {
        tokio::spawn(async move {
            loop {
                let state = self.state.read().await.clone();
                
                if state == ConnectionState::Connected {
                    tracing::info!("Subscribing to backend commands...");

                    // C-3 fix: clone the shared client instead of opening a second TCP
                    // connection.  The cloned handle multiplexes over the same H2 stream
                    // managed by BackendConnectionManager, so state remains consistent.
                    let mut subscription_client = match self.client.read().await.clone() {
                        Some(c) => c,
                        None => {
                            tracing::warn!("Client unavailable for subscription, waiting...");
                            sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    };

                    match subscription_client.subscribe_commands(platforms.clone()).await {
                        Ok(mut stream) => {
                            tracing::info!("Subscribed to backend commands");

                            // Process commands until stream ends or error
                            loop {
                                match stream.message().await {
                                    Ok(Some(command)) => {
                                        tracing::info!(
                                            "Received command from backend: {:?} (ID: {})",
                                            command.command_type,
                                            command.command_id
                                        );

                                        // P-3: dispatch on typed CommandType enum instead of a
                                        // raw string, giving compile-time exhaustiveness checking.
                                        let cmd_type = rinko_common::proto::CommandType::try_from(
                                            command.command_type,
                                        )
                                        .unwrap_or(rinko_common::proto::CommandType::Unspecified);

                                        match cmd_type {
                                            rinko_common::proto::CommandType::SendMessage => {
                                                // P-4: access structured payload instead of stringly-typed map.
                                                if let Some(
                                                    rinko_common::proto::bot_command::Payload::SendMessage(
                                                        ref payload,
                                                    ),
                                                ) = command.payload
                                                {
                                                    tracing::info!(
                                                        "SendMessage command: target='{}', content='{}', type={:?}",
                                                        payload.target_openid,
                                                        payload.content,
                                                        payload.content_type
                                                    );
                                                    // TODO: dispatch to platform send handler
                                                } else {
                                                    tracing::warn!(
                                                        "SendMessage command missing payload (ID: {})",
                                                        command.command_id
                                                    );
                                                }
                                            }
                                            rinko_common::proto::CommandType::Shutdown => {
                                                tracing::warn!("Received shutdown command from backend");
                                                // TODO: Graceful shutdown
                                            }
                                            rinko_common::proto::CommandType::GetStatus => {
                                                tracing::info!("Get status command received");
                                                // TODO: Collect and report status to backend
                                            }
                                            rinko_common::proto::CommandType::Unspecified => {
                                                tracing::debug!(
                                                    "Received command with unspecified type (ID: {})",
                                                    command.command_id
                                                );
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        tracing::warn!("Backend command stream ended");
                                        break;
                                    }
                                    Err(e) => {
                                        tracing::error!("Error receiving command: {}", e);
                                        break;
                                    }
                                }
                            }

                            // Stream ended, mark as disconnected
                            self.mark_disconnected().await;
                        }
                        Err(e) => {
                            tracing::error!("Failed to subscribe to commands: {}", e);
                            self.mark_disconnected().await;
                        }
                    }
                }
                
                // Wait before retrying subscription (will be connected by reconnect task)
                sleep(Duration::from_secs(5)).await;
            }
        });
    }
}
