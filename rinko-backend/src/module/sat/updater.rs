///! Satellite updater - scheduled update tasks
use super::manager::SatelliteManager;
use chrono::{DateTime, Timelike, Utc};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_SECONDS: u64 = 60;
const UPDATE_TIMEOUT_SECONDS: u64 = 300; // 5 minutes

/// Satellite updater handles scheduled updates
pub struct SatelliteUpdater {
    manager: Arc<SatelliteManager>,
    update_interval_minutes: u64,
}

impl SatelliteUpdater {
    /// Create a new updater
    pub fn new(manager: Arc<SatelliteManager>, update_interval_minutes: u64) -> Self {
        Self {
            manager,
            update_interval_minutes,
        }
    }

    /// Start updater with immediate initial update
    /// 
    /// Performs one update immediately, then starts the scheduled loop.
    /// Returns a JoinHandle for the background task.
    pub async fn start_with_initial_update(self) -> anyhow::Result<JoinHandle<()>> {
        tracing::info!("Starting satellite updater (initial update + schedule)");

        // Perform initial update
        self.run_update_cycle().await;

        // Start scheduled updates
        let handle = tokio::spawn(async move {
            self.run_scheduled_loop().await;
        });

        Ok(handle)
    }

    /// Start updater without initial update
    /// 
    /// Calculates next update time and starts the scheduled loop.
    /// Returns a JoinHandle for the background task.
    pub async fn start(self) -> anyhow::Result<JoinHandle<()>> {
        tracing::info!("Starting satellite updater (scheduled only)");

        let handle = tokio::spawn(async move {
            self.run_scheduled_loop().await;
        });

        Ok(handle)
    }

    /// Run the scheduled update loop
    async fn run_scheduled_loop(&self) {
        loop {
            let now = Utc::now();
            let next_trigger = self.calculate_next_trigger(now);
            let sleep_duration = (next_trigger - now)
                .to_std()
                .unwrap_or(Duration::from_secs(60));

            tracing::info!(
                "Next satellite update scheduled at: {} (in {:.1} minutes)",
                next_trigger.format("%Y-%m-%d %H:%M:%S UTC"),
                sleep_duration.as_secs_f64() / 60.0
            );

            tokio::time::sleep(sleep_duration).await;

            self.run_update_cycle().await;
        }
    }

    /// Run a single update cycle with retries
    async fn run_update_cycle(&self) {
        for attempt in 1..=MAX_RETRIES {
            tracing::info!(
                "Starting satellite update (attempt {}/{})",
                attempt,
                MAX_RETRIES
            );

            let result = tokio::time::timeout(
                Duration::from_secs(UPDATE_TIMEOUT_SECONDS),
                self.manager.update_all_satellites(),
            )
            .await;

            match result {
                Ok(Ok(report)) => {
                    tracing::info!(
                        "✓ Satellite update completed: {} successful, {} failed, {} new, {:.2}s",
                        report.successful_updates,
                        report.failed_updates,
                        report.new_satellites.len(),
                        report.duration_seconds
                    );

                    if !report.new_satellites.is_empty() {
                        tracing::info!("New satellites: {:?}", report.new_satellites);
                    }

                    if !report.inactive_satellites.is_empty() {
                        tracing::warn!("Inactive satellites: {:?}", report.inactive_satellites);
                    }

                    // Success, break retry loop
                    break;
                }
                Ok(Err(e)) => {
                    tracing::error!(
                        "✗ Satellite update failed (attempt {}/{}): {}",
                        attempt,
                        MAX_RETRIES,
                        e
                    );
                }
                Err(_) => {
                    tracing::error!(
                        "✗ Satellite update timed out after {}s (attempt {}/{})",
                        UPDATE_TIMEOUT_SECONDS,
                        attempt,
                        MAX_RETRIES
                    );
                }
            }

            // Retry if not last attempt
            if attempt < MAX_RETRIES {
                let delay = Duration::from_secs(RETRY_DELAY_SECONDS * attempt as u64);
                tracing::info!("Retrying in {:?}...", delay);
                tokio::time::sleep(delay).await;
            } else {
                tracing::error!(
                    "Satellite update failed after {} attempts, giving up",
                    MAX_RETRIES
                );
            }
        }
    }

    /// Calculate next trigger time based on update interval
    /// 
    /// If interval is <= 15 minutes: triggers at 02, 17, 32, 47 past the hour
    /// Otherwise: triggers every N minutes from now
    fn calculate_next_trigger(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        if self.update_interval_minutes <= 15 {
            // Use fixed-minute schedule (02, 17, 32, 47)
            self.calculate_fixed_minute_trigger(now)
        } else {
            // Use simple interval
            now + chrono::Duration::minutes(self.update_interval_minutes as i64)
        }
    }

    /// Calculate next trigger for fixed-minute schedule
    /// Updates at: xx:02, xx:17, xx:32, xx:47
    fn calculate_fixed_minute_trigger(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        let current_minute = now.minute();

        let next_minute = match current_minute {
            0..=1 => 2,
            2..=16 => 17,
            17..=31 => 32,
            32..=46 => 47,
            _ => {
                // 47..=59, wrap to next hour at 02
                return now
                    .with_hour((now.hour() + 1) % 24)
                    .unwrap()
                    .with_minute(2)
                    .unwrap()
                    .with_second(0)
                    .unwrap()
                    .with_nanosecond(0)
                    .unwrap();
            }
        };

        now.with_minute(next_minute)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap()
    }
}

/// Helper function to create and start updater
pub async fn start_satellite_updater(
    manager: Arc<SatelliteManager>,
    update_interval_minutes: u64,
    initial_update: bool,
) -> anyhow::Result<JoinHandle<()>> {
    let updater = SatelliteUpdater::new(manager, update_interval_minutes);

    if initial_update {
        updater.start_with_initial_update().await
    } else {
        updater.start().await
    }
}
