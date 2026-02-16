///! Web scraper for fetching satellite list from AMSAT status page
use anyhow::{Context, Result};
use reqwest;
use scraper::{Html, Selector};

const AMSAT_STATUS_URL: &str = "https://www.amsat.org/status/";

/// Fetch list of satellite names from AMSAT status page
/// 
/// Scrapes the satellite dropdown menu from the AMSAT status page
/// to get the current list of tracked satellites.
/// 
/// # Returns
/// Vec of satellite names on success, Error on failure
pub async fn fetch_satellite_names() -> Result<Vec<String>> {
    tracing::debug!("Fetching satellite list from {}", AMSAT_STATUS_URL);
    
    // Fetch the page
    let response = reqwest::get(AMSAT_STATUS_URL)
        .await
        .context("Failed to fetch AMSAT status page")?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch AMSAT status page: HTTP {}",
            response.status()
        ));
    }
    
    let html_body = response
        .text()
        .await
        .context("Failed to read AMSAT status page body")?;
    
    // Parse HTML
    let document = Html::parse_document(&html_body);
    
    // Select the satellite dropdown options
    let selector = Selector::parse(r#"select[name="SatName"] > option"#)
        .map_err(|e| anyhow::anyhow!("Invalid CSS selector: {:?}", e))?;
    
    // Extract satellite names
    let mut satellite_names = Vec::new();
    for element in document.select(&selector) {
        if let Some(value) = element.value().attr("value") {
            let trimmed = value.trim();
            // Filter out empty values and placeholder text
            if !trimmed.is_empty() && trimmed != "Select Satellite" {
                satellite_names.push(trimmed.to_string());
            }
        }
    }
    
    tracing::info!(
        "Successfully fetched {} satellite names from AMSAT",
        satellite_names.len()
    );
    
    if satellite_names.is_empty() {
        tracing::warn!("No satellites found in AMSAT status page");
    }
    
    Ok(satellite_names)
}

/// Get a hardcoded list of known AMSAT satellites as fallback
/// 
/// This list is based on common amateur radio satellites and serves
/// as a fallback if the scraper fails.
pub fn get_known_satellites() -> Vec<String> {
    vec![
        "AISAT-1", "AO-123", "AO-16", "AO-27", "AO-73", "AO-7[A]", "AO-7[B]",
        "AO-85", "AO-91", "CAS-2T", "CAS-4A", "CAS-4B", "CatSat", "CUTE-1",
        "DSTAR1", "DUCHIFAT1", "DUCHIFAT3", "EO-79", "EO-80", "ESEO",
        "FloripaSat-1", "FO-118[H/u]", "FO-118[V/u+FM]", "FO-118[V/u]",
        "FO-29", "FO-99", "GO-32", "HA-1", "HO-107", "HO-113", "IO-117",
        "IO-26", "IO-86", "ISS-DATA", "ISS-DATV", "ISS-FM", "ISS-SSTV",
        "JO-97", "K2SAT", "LEDSAT", "LilacSat-2", "LO-19", "LO-87", "LO-90",
        "LO-93", "MO-122", "NO-44", "NO-45", "OUFTI-1", "PO-101[FM]",
        "QO-100", "RS-44", "RS95s", "SO-50", "SO-124", "SO-125",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Fetch satellite names with fallback to known list
/// 
/// Attempts to scrape the AMSAT website, but falls back to a
/// hardcoded list of known satellites if scraping fails.
pub async fn fetch_satellite_names_with_fallback() -> Vec<String> {
    match fetch_satellite_names().await {
        Ok(names) if !names.is_empty() => names,
        Ok(_) => {
            tracing::warn!("Scraper returned empty list, using fallback");
            get_known_satellites()
        }
        Err(e) => {
            tracing::error!("Failed to scrape satellite list: {}", e);
            tracing::info!("Using fallback list of known satellites");
            get_known_satellites()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_satellites_not_empty() {
        let satellites = get_known_satellites();
        assert!(!satellites.is_empty());
        assert!(satellites.contains(&"AO-91".to_string()));
        assert!(satellites.contains(&"ISS-FM".to_string()));
    }

    #[tokio::test]
    #[ignore] // Requires network connection
    async fn test_fetch_satellite_names() {
        let result = fetch_satellite_names().await;
        assert!(result.is_ok());
        if let Ok(names) = result {
            assert!(!names.is_empty());
            println!("Found {} satellites", names.len());
        }
    }

    #[tokio::test]
    async fn test_fallback_always_works() {
        let satellites = fetch_satellite_names_with_fallback().await;
        assert!(!satellites.is_empty());
    }
}
