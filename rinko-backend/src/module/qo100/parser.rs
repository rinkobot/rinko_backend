///! QO-100 DX Cluster JSON parser
///!
///! Fetches the QO-100 Dx Club cluster data via its AJAX JSON API
///! (https://qo100dx.club/cluster/) and parses it into structured data.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Deserialize;

use super::types::{Qo100Snapshot, Qo100Spot};

/// Raw JSON spot returned by the QO-100 DX Club API
#[derive(Debug, Deserialize)]
struct RawSpot {
    #[allow(dead_code)]
    key: String,
    datetime: String,
    frequency: f64,
    de: String,
    dx: String,
    comment: String,
    #[allow(dead_code)]
    grid_de: Option<String>,
    #[allow(dead_code)]
    grid_dx: Option<String>,
    spot_source: String,
}

/// Wrapper for the JSON response
#[derive(Debug, Deserialize)]
struct ClusterResponse {
    spots: Vec<RawSpot>,
}

/// Convert raw frequency (Hz) to the display format used on the site.
///
/// Frequencies in the 10489.5–10490.0 MHz range are shown as
/// ".XXX" (offset from 10489 MHz), e.g. 10489740 → ".740".
/// Anything outside that range is shown as "--".
fn format_frequency(freq_hz: f64) -> String {
    let freq = freq_hz.round() as i64;
    if freq >= 10_489_500 && freq < 10_490_000 {
        format!(".{}", freq - 10_489_000)
    } else {
        "--".to_string()
    }
}

/// Parse the QO-100 DX Cluster JSON into a [`Qo100Snapshot`].
pub fn parse_qo100_json(json: &str) -> Result<Qo100Snapshot> {
    let resp: ClusterResponse =
        serde_json::from_str(json).context("Failed to deserialize QO-100 cluster JSON")?;

    let mut spots: Vec<Qo100Spot> = resp
        .spots
        .into_iter()
        .map(|raw| Qo100Spot {
            datetime: raw.datetime,
            dx:       raw.dx,
            freq:     format_frequency(raw.frequency),
            comments: raw.comment.split_whitespace().collect::<Vec<_>>().join(" "),
            spotter:  raw.de,
            source:   raw.spot_source,
        })
        .collect();

    // API returns oldest-first; reverse so newest is first (like the web page)
    spots.reverse();

    Ok(Qo100Snapshot {
        fetched_at: Utc::now(),
        spots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_frequency() {
        assert_eq!(format_frequency(10_489_740.0), ".740");
        assert_eq!(format_frequency(10_489_520.0), ".520");
        assert_eq!(format_frequency(10_489_900.0), ".900");
        assert_eq!(format_frequency(29_041.7), "--");
        assert_eq!(format_frequency(10_490_000.0), "--");
        assert_eq!(format_frequency(10_489_500.0), ".500");
    }

    #[test]
    fn test_parse_json_basic() {
        let json = r#"{"spots":[
            {"key":"aaa","datetime":"2026-02-18 14:21","frequency":10489740,"de":"OM0AAO","dx":"PY5ZUE/P","comment":"QO-100 HI24","grid_de":"KN09","grid_dx":null,"spot_source":"DXCluster"}
        ]}"#;
        let snapshot = parse_qo100_json(json).unwrap();
        assert_eq!(snapshot.spots.len(), 1);
        let s = &snapshot.spots[0];
        assert_eq!(s.datetime, "2026-02-18 14:21");
        assert_eq!(s.dx, "PY5ZUE/P");
        assert_eq!(s.freq, ".740");
        assert_eq!(s.comments, "QO-100 HI24");
        assert_eq!(s.spotter, "OM0AAO");
        assert_eq!(s.source, "DXCluster");
    }

    #[test]
    fn test_parse_json_multiple_reversed() {
        let json = r#"{"spots":[
            {"key":"a","datetime":"2026-02-17 10:00","frequency":10489520,"de":"DL4CH","dx":"AA1BB","comment":"first","grid_de":null,"grid_dx":null,"spot_source":"DXCluster"},
            {"key":"b","datetime":"2026-02-18 14:21","frequency":10489740,"de":"OM0AAO","dx":"PY5ZUE/P","comment":"second","grid_de":"KN09","grid_dx":null,"spot_source":"DXCluster"}
        ]}"#;
        let snapshot = parse_qo100_json(json).unwrap();
        assert_eq!(snapshot.spots.len(), 2);
        // Newest first after reverse
        assert_eq!(snapshot.spots[0].dx, "PY5ZUE/P");
        assert_eq!(snapshot.spots[1].dx, "AA1BB");
    }

    #[test]
    fn test_parse_json_out_of_band_freq() {
        let json = r#"{"spots":[
            {"key":"c","datetime":"2026-02-14 12:10","frequency":29041.7,"de":"SP3OTZ","dx":"ZS1A","comment":"QO-100","grid_de":"JO82","grid_dx":null,"spot_source":"DXCluster"}
        ]}"#;
        let snapshot = parse_qo100_json(json).unwrap();
        assert_eq!(snapshot.spots[0].freq, "--");
    }
}
