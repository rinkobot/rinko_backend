///! Search engine for satellite name matching
use super::types::SatelliteList;
use strsim::jaro_winkler;

/// Default similarity threshold for fuzzy matching
pub const DEFAULT_THRESHOLD: f64 = 0.85;

/// Search for satellites matching the query
/// 
/// # Search Priority
/// 1. Exact match on official name
/// 2. Exact match on aliases
/// 3. Catalog number match (if available)
/// 4. Fuzzy match (Jaro-Winkler similarity >= threshold)
/// 
/// # Arguments
/// * `query` - Search query string
/// * `satellite_list` - List of satellites to search
/// * `threshold` - Similarity threshold (0.0 to 1.0)
/// 
/// # Returns
/// Vec of matching satellite official names, ordered by relevance
pub fn search_satellites(
    query: &str,
    satellite_list: &SatelliteList,
    threshold: f64,
) -> Vec<String> {
    // First try hard match (exact match)
    let hard_matches = hard_match(query, satellite_list);
    if !hard_matches.is_empty() {
        return hard_matches;
    }
    
    // Then try fuzzy match
    let fuzzy_matches = fuzzy_match(query, satellite_list, threshold);
    fuzzy_matches.into_iter().map(|(_, name)| name).collect()
}

/// Hard match: exact match on official name, aliases, or catalog number
fn hard_match(query: &str, satellite_list: &SatelliteList) -> Vec<String> {
    let normalized_query = normalize_string(query);
    let mut results = Vec::new();
    
    for sat in &satellite_list.satellites {
        // Check official name
        if normalize_string(&sat.official_name) == normalized_query {
            results.push(sat.official_name.clone());
            continue;
        }
        
        // Check aliases
        if sat.aliases.iter().any(|alias| normalize_string(alias) == normalized_query) {
            results.push(sat.official_name.clone());
            continue;
        }
        
        // Check catalog number
        if let Some(ref catalog_num) = sat.catalog_number {
            if normalize_string(catalog_num) == normalized_query {
                results.push(sat.official_name.clone());
            }
        }
    }
    
    results
}

/// Fuzzy match using Jaro-Winkler similarity
fn fuzzy_match(
    query: &str,
    satellite_list: &SatelliteList,
    threshold: f64,
) -> Vec<(f64, String)> {
    let query_lower = query.to_lowercase();
    let mut matches: Vec<(f64, String)> = Vec::new();
    
    for sat in &satellite_list.satellites {
        let mut best_score: f64 = 0.0;
        
        // Check official name
        let score = jaro_winkler(&query_lower, &sat.official_name.to_lowercase());
        best_score = best_score.max(score);
        
        // Check aliases
        for alias in &sat.aliases {
            let score = jaro_winkler(&query_lower, &alias.to_lowercase());
            best_score = best_score.max(score);
        }
        
        if best_score >= threshold {
            matches.push((best_score, sat.official_name.clone()));
        }
    }
    
    // Sort by score descending  
    matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    
    matches
}

/// Normalize string for matching (lowercase, remove punctuation and whitespace)
pub fn normalize_string(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_ascii_punctuation() && !c.is_whitespace())
        .collect()
}

/// Special search keywords that return predefined groups
pub fn check_special_keywords(query: &str) -> Option<Vec<String>> {
    let normalized = normalize_string(query);
    
    match normalized.as_str() {
        "fm" => Some(vec![
            "AO-91".to_string(),
            "PO-101[FM]".to_string(),
            "ISS-FM".to_string(),
            "SO-50".to_string(),
            "AO-123 FM".to_string(),
            "SO-124".to_string(),
            "SO-125".to_string(),
            "RS95s".to_string(),
        ]),
        "linear" | "lin" => Some(vec![
            "AO-7".to_string(),
            "AO-27".to_string(),
            "FO-29".to_string(),
            "RS-44".to_string(),
            "QO-100".to_string(),
            "JO-97".to_string(),
        ]),
        _ => None,
    }
}

/// Search satellites with special keywords support
/// 
/// This function first checks for special keywords, then falls back
/// to normal search if no special keyword is found.
pub fn search_with_keywords(
    query: &str,
    satellite_list: &SatelliteList,
    threshold: f64,
) -> Vec<String> {
    // Check for special keywords
    if let Some(satellites) = check_special_keywords(query) {
        return satellites;
    }
    
    // Normal search
    search_satellites(query, satellite_list, threshold)
}

/// Search multiple queries (separated by '/')
/// 
/// # Arguments
/// * `input` - Input string with queries separated by '/'
/// * `satellite_list` - List of satellites to search
/// * `threshold` - Similarity threshold
/// 
/// # Returns
/// Vec of unique matching satellite names
pub fn search_multiple(
    input: &str,
    satellite_list: &SatelliteList,
    threshold: f64,
) -> Vec<String> {
    let queries: Vec<&str> = input.split('/').map(|s| s.trim()).collect();
    let mut results = Vec::new();
    
    for query in queries {
        if query.is_empty() {
            continue;
        }
        
        let matches = search_with_keywords(query, satellite_list, threshold);
        for sat_name in matches {
            if !results.contains(&sat_name) {
                results.push(sat_name);
            }
        }
    }
    
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{SatelliteEntry};

    fn create_test_list() -> SatelliteList {
        let mut list = SatelliteList::default();
        
        let mut ao91 = SatelliteEntry::new("AO-91");
        ao91.aliases = vec!["Fox-1B".to_string(), "RadFxSat".to_string()];
        ao91.catalog_number = Some("43017".to_string());
        list.satellites.push(ao91);
        
        let mut iss = SatelliteEntry::new("ISS-FM");
        iss.aliases = vec!["ISS".to_string(), "国际空间站".to_string()];
        iss.catalog_number = Some("25544".to_string());
        list.satellites.push(iss);
        
        list.satellites.push(SatelliteEntry::new("FO-29"));
        
        list
    }

    #[test]
    fn test_exact_match() {
        let list = create_test_list();
        let results = search_satellites("AO-91", &list, 0.85);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "AO-91");
    }

    #[test]
    fn test_alias_match() {
        let list = create_test_list();
        let results = search_satellites("Fox-1B", &list, 0.85);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "AO-91");
    }

    #[test]
    fn test_catalog_number_match() {
        let list = create_test_list();
        let results = search_satellites("43017", &list, 0.85);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "AO-91");
    }

    #[test]
    fn test_case_insensitive() {
        let list = create_test_list();
        let results = search_satellites("ao-91", &list, 0.85);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "AO-91");
    }

    #[test]
    fn test_fuzzy_match() {
        let list = create_test_list();
        let results = search_satellites("AO91", &list, 0.80);
        assert!(!results.is_empty());
        assert!(results.contains(&"AO-91".to_string()));
    }

    #[test]
    fn test_special_keyword_fm() {
        let list = create_test_list();
        let results = search_with_keywords("fm", &list, 0.85);
        assert!(!results.is_empty());
        assert!(results.contains(&"AO-91".to_string()));
    }

    #[test]
    fn test_multiple_queries() {
        let list = create_test_list();
        let results = search_multiple("AO-91/ISS-FM", &list, 0.85);
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"AO-91".to_string()));
        assert!(results.contains(&"ISS-FM".to_string()));
    }

    #[test]
    fn test_normalize_string() {
        assert_eq!(normalize_string("AO-91"), "ao91");
        assert_eq!(normalize_string("Fox 1B"), "fox1b");
        assert_eq!(normalize_string("  Test  "), "test");
    }
}
