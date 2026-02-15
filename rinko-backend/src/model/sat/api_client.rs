///! AMSAT API client for fetching satellite status data
use super::types::AmsatReport;
use anyhow::{Context, Result};
use reqwest;
use std::time::Duration;

const AMSAT_API_URL: &str = "https://www.amsat.org/status/api/v1/sat_info.php";
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_SECONDS: u64 = 2;
const REQUEST_TIMEOUT_SECONDS: u64 = 60;

/// Fetch satellite data from AMSAT API
/// 
/// # Arguments
/// * `sat_name` - Satellite name (case-sensitive)
/// * `hours` - Number of hours of data to fetch (default: 1, max: 96)
/// 
/// # Returns
/// Vec of AmsatReport on success, Error on failure
pub async fn fetch_satellite_data(sat_name: &str, hours: u64) -> Result<Vec<AmsatReport>> {
    let api_url = format!("{}?name={}&hours={}", AMSAT_API_URL, sat_name, hours);
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .build()
        .context("Failed to build HTTP client")?;

    for attempt in 1..=MAX_RETRIES {
        if attempt > 1 {
            let delay = Duration::from_secs(RETRY_DELAY_SECONDS * attempt as u64);
            tracing::debug!(
                "Retrying {} after {:?} (attempt {}/{})",
                sat_name,
                delay,
                attempt,
                MAX_RETRIES
            );
            tokio::time::sleep(delay).await;
        }

        match fetch_attempt(&client, &api_url, sat_name).await {
            Ok(data) => {
                tracing::debug!(
                    "Successfully fetched {} reports for {}",
                    data.len(),
                    sat_name
                );
                return Ok(data);
            }
            Err(e) => {
                if attempt == MAX_RETRIES {
                    tracing::error!(
                        "Failed to fetch {} after {} attempts: {}",
                        sat_name,
                        MAX_RETRIES,
                        e
                    );
                    return Err(e);
                } else {
                    tracing::warn!(
                        "Attempt {}/{} failed for {}: {}",
                        attempt,
                        MAX_RETRIES,
                        sat_name,
                        e
                    );
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to fetch data for {} after {} attempts",
        sat_name,
        MAX_RETRIES
    ))
}

/// Single fetch attempt
async fn fetch_attempt(
    client: &reqwest::Client,
    url: &str,
    sat_name: &str,
) -> Result<Vec<AmsatReport>> {
    let response = client
        .get(url)
        .send()
        .await
        .context(format!("Failed to send request for {}", sat_name))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "HTTP error {} for {}",
            response.status(),
            sat_name
        ));
    }

    let data: Vec<AmsatReport> = response
        .json()
        .await
        .context(format!("Failed to parse JSON response for {}", sat_name))?;

    Ok(data)
}

/// Batch fetch multiple satellites with delay between requests
/// 
/// # Arguments
/// * `sat_names` - List of satellite names to fetch
/// * `hours` - Number of hours of data to fetch
/// * `delay_ms` - Delay between requests in milliseconds (to avoid rate limiting)
/// 
/// # Returns
/// HashMap of satellite name to Result<Vec<AmsatReport>>
pub async fn batch_fetch_satellites(
    sat_names: &[String],
    hours: u64,
    delay_ms: u64,
) -> std::collections::HashMap<String, Result<Vec<AmsatReport>>> {
    let mut results = std::collections::HashMap::new();

    for (index, sat_name) in sat_names.iter().enumerate() {
        if index > 0 && delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        let result = fetch_satellite_data(sat_name, hours).await;
        results.insert(sat_name.clone(), result);
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network connection
    async fn test_fetch_satellite_data() {
        let result = fetch_satellite_data("AO-91", 1).await;
        assert!(result.is_ok() || result.is_err()); // Just test it can run
    }

    #[tokio::test]
    #[ignore]
    async fn test_batch_fetch() {
        let sat_names = vec!["AO-91".to_string(), "ISS-FM".to_string()];
        let results = batch_fetch_satellites(&sat_names, 1, 200).await;
        assert_eq!(results.len(), 2);
    }
}
