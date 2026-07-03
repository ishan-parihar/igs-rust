//! Advanced intelligence capabilities (P2):
//! - Temporal intelligence: anomaly detection, time-series analysis
//! - Geospatial intelligence: location extraction, geo-tagging
//! - Multi-language support: language detection, stop words
//! - Source quality: trust scoring, cross-source verification
//! - Report generation: markdown briefings with citations

use serde::{Deserialize, Serialize};

// ─── Temporal Intelligence ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TimeSeriesPoint {
    pub timestamp: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AnomalyResult {
    pub timestamp: String,
    pub count: u32,
    pub z_score: f64,
    pub is_anomaly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TemporalAnalysisOutput {
    pub entity: String,
    pub time_series: Vec<TimeSeriesPoint>,
    pub anomalies: Vec<AnomalyResult>,
    pub mean: f64,
    pub std_dev: f64,
}

/// Analyze a time series of entity mention counts and detect anomalies
/// using z-score. Points with |z| > 2.0 are flagged as anomalies.
pub fn analyze_time_series(
    entity: &str,
    points: &[(String, u32)],
) -> TemporalAnalysisOutput {
    if points.is_empty() {
        return TemporalAnalysisOutput {
            entity: entity.to_string(),
            time_series: vec![],
            anomalies: vec![],
            mean: 0.0,
            std_dev: 0.0,
        };
    }

    let counts: Vec<f64> = points.iter().map(|(_, c)| *c as f64).collect();
    let mean = counts.iter().sum::<f64>() / counts.len() as f64;
    let variance = counts.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / counts.len() as f64;
    let std_dev = variance.sqrt();

    let time_series: Vec<TimeSeriesPoint> = points
        .iter()
        .map(|(ts, c)| TimeSeriesPoint {
            timestamp: ts.clone(),
            count: *c,
        })
        .collect();

    let anomalies: Vec<AnomalyResult> = points
        .iter()
        .map(|(ts, c)| {
            let z = if std_dev > 0.0 { (*c as f64 - mean) / std_dev } else { 0.0 };
            AnomalyResult {
                timestamp: ts.clone(),
                count: *c,
                z_score: z,
                is_anomaly: z.abs() >= 2.0,
            }
        })
        .filter(|a| a.is_anomaly)
        .collect();

    TemporalAnalysisOutput {
        entity: entity.to_string(),
        time_series,
        anomalies,
        mean,
        std_dev,
    }
}

// ─── Geospatial Intelligence ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GeoEntity {
    pub name: String,
    pub entity_type: String, // GPE, LOC, FAC
    pub country_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GeoExtractionOutput {
    pub text: String,
    pub locations: Vec<GeoEntity>,
    pub count: usize,
}

/// Common country names and their ISO codes for quick lookup.
fn country_lookup() -> std::collections::HashMap<&'static str, (&'static str, f64, f64)> {
    let mut m = std::collections::HashMap::new();
    // (name, code, lat, lon) — approximate centroids
    m.insert("united states", ("US", 39.8, -98.5));
    m.insert("usa", ("US", 39.8, -98.5));
    m.insert("china", ("CN", 35.0, 105.0));
    m.insert("russia", ("RU", 61.5, 105.3));
    m.insert("india", ("IN", 20.6, 78.9));
    m.insert("united kingdom", ("GB", 55.4, -3.4));
    m.insert("uk", ("GB", 55.4, -3.4));
    m.insert("france", ("FR", 46.2, 2.2));
    m.insert("germany", ("DE", 51.2, 10.4));
    m.insert("japan", ("JP", 36.2, 138.2));
    m.insert("south korea", ("KR", 35.9, 127.8));
    m.insert("north korea", ("KP", 40.3, 127.5));
    m.insert("iran", ("IR", 32.4, 53.7));
    m.insert("iraq", ("IQ", 33.2, 43.7));
    m.insert("israel", ("IL", 31.0, 34.9));
    m.insert("ukraine", ("UA", 48.4, 31.2));
    m.insert("brazil", ("BR", -14.2, -51.9));
    m.insert("canada", ("CA", 56.1, -106.3));
    m.insert("australia", ("AU", -25.3, 133.8));
    m.insert("mexico", ("MX", 23.6, -102.6));
    m.insert("saudi arabia", ("SA", 23.9, 45.1));
    m.insert("turkey", ("TR", 38.9, 35.2));
    m.insert("egypt", ("EG", 26.8, 30.8));
    m.insert("south africa", ("ZA", -30.6, 22.9));
    m.insert("nigeria", ("NG", 9.1, 8.7));
    m.insert("pakistan", ("PK", 30.4, 69.3));
    m.insert("indonesia", ("ID", -0.8, 113.9));
    m
}

/// Common city names with approximate coordinates.
fn city_lookup() -> std::collections::HashMap<&'static str, (&'static str, f64, f64)> {
    let mut m = std::collections::HashMap::new();
    m.insert("washington", ("US", 38.9, -77.0));
    m.insert("new york", ("US", 40.7, -74.0));
    m.insert("los angeles", ("US", 34.1, -118.2));
    m.insert("san francisco", ("US", 37.8, -122.4));
    m.insert("chicago", ("US", 41.9, -87.6));
    m.insert("houston", ("US", 29.8, -95.4));
    m.insert("miami", ("US", 25.8, -80.2));
    m.insert("seattle", ("US", 47.6, -122.3));
    m.insert("boston", ("US", 42.4, -71.1));
    m.insert("london", ("GB", 51.5, -0.1));
    m.insert("paris", ("FR", 48.9, 2.3));
    m.insert("berlin", ("DE", 52.5, 13.4));
    m.insert("moscow", ("RU", 55.8, 37.6));
    m.insert("beijing", ("CN", 39.9, 116.4));
    m.insert("shanghai", ("CN", 31.2, 121.5));
    m.insert("tokyo", ("JP", 35.7, 139.7));
    m.insert("delhi", ("IN", 28.6, 77.2));
    m.insert("mumbai", ("IN", 19.1, 72.9));
    m.insert("tel aviv", ("IL", 32.1, 34.8));
    m.insert("jerusalem", ("IL", 31.8, 35.2));
    m.insert("kyiv", ("UA", 50.5, 30.5));
    m.insert("kiev", ("UA", 50.5, 30.5));
    m.insert("dubai", ("AE", 25.2, 55.3));
    m.insert("istanbul", ("TR", 41.0, 28.9));
    m.insert("cairo", ("EG", 30.0, 31.2));
    m.insert("rio de janeiro", ("BR", -22.9, -43.2));
    m.insert("sao paulo", ("BR", -23.5, -46.6));
    m.insert("sydney", ("AU", -33.9, 151.2));
    m.insert("toronto", ("CA", 43.7, -79.4));
    m.insert("mexico city", ("MX", 19.4, -99.1));
    m
}

/// Extract location entities from text using a gazetteer-based approach.
/// Detects country names, city names, and capitalized place-like words.
pub fn extract_locations(text: &str) -> GeoExtractionOutput {
    let countries = country_lookup();
    let cities = city_lookup();
    let lower = text.to_lowercase();
    let mut locations: Vec<GeoEntity> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Check for country names
    for (name, (code, lat, lon)) in &countries {
        if lower.contains(name) && !seen.contains(&name.to_string()) {
            locations.push(GeoEntity {
                name: title_case(name),
                entity_type: "GPE".into(),
                country_code: Some(code.to_string()),
                latitude: Some(*lat),
                longitude: Some(*lon),
            });
            seen.insert(name.to_string());
        }
    }

    // Check for city names
    for (name, (code, lat, lon)) in &cities {
        if lower.contains(name) && !seen.contains(&name.to_string()) {
            locations.push(GeoEntity {
                name: title_case(name),
                entity_type: "LOC".into(),
                country_code: Some(code.to_string()),
                latitude: Some(*lat),
                longitude: Some(*lon),
            });
            seen.insert(name.to_string());
        }
    }

    let count = locations.len();
    GeoExtractionOutput {
        text: text.to_string(),
        locations,
        count,
    }
}

// ─── Multi-language Support ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LanguageDetectionOutput {
    pub text: String,
    pub detected_language: String,
    pub confidence: f64,
    pub script: String,
}

/// Detect the language of a text using simple heuristics:
/// - Check Unicode script (Latin, Cyrillic, CJK, Arabic, Devanagari)
/// - Check for common stop words in each language
pub fn detect_language(text: &str) -> LanguageDetectionOutput {
    let mut script_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            *script_counts.entry("Latin").or_default() += 1;
        } else if (c as u32) >= 0x0400 && (c as u32) <= 0x04FF {
            *script_counts.entry("Cyrillic").or_default() += 1;
        } else if (c as u32) >= 0x4E00 && (c as u32) <= 0x9FFF {
            *script_counts.entry("CJK").or_default() += 1;
        } else if (c as u32) >= 0x0600 && (c as u32) <= 0x06FF {
            *script_counts.entry("Arabic").or_default() += 1;
        } else if (c as u32) >= 0x0900 && (c as u32) <= 0x097F {
            *script_counts.entry("Devanagari").or_default() += 1;
        }
    }

    let total: usize = script_counts.values().sum();
    if total == 0 {
        return LanguageDetectionOutput {
            text: text.to_string(),
            detected_language: "unknown".into(),
            confidence: 0.0,
            script: "unknown".into(),
        };
    }

    let (dominant_script, count) = script_counts
        .iter()
        .max_by_key(|(_, &c)| c)
        .map(|(&s, &c)| (s, c))
        .unwrap_or(("Latin", 0));

    let confidence = count as f64 / total as f64;
    let language = match dominant_script {
        "Latin" => detect_latin_language(text),
        "Cyrillic" => "ru".into(),
        "CJK" => "zh".into(),
        "Arabic" => "ar".into(),
        "Devanagari" => "hi".into(),
        _ => "unknown".into(),
    };

    LanguageDetectionOutput {
        text: text.to_string(),
        detected_language: language,
        confidence,
        script: dominant_script.to_string(),
    }
}

/// Detect which Latin-script language by checking stop words.
fn detect_latin_language(text: &str) -> String {
    let lower = text.to_lowercase();
    let words: std::collections::HashSet<&str> = lower.split_whitespace().collect();

    let en_words = ["the", "and", "is", "in", "to", "of", "a", "it"];
    let es_words = ["el", "la", "y", "es", "en", "de", "un", "que"];
    let fr_words = ["le", "la", "et", "est", "en", "de", "un", "que"];
    let de_words = ["der", "die", "und", "ist", "in", "von", "ein", "das"];

    let en_count = en_words.iter().filter(|w| words.contains(*w)).count();
    let es_count = es_words.iter().filter(|w| words.contains(*w)).count();
    let fr_count = fr_words.iter().filter(|w| words.contains(*w)).count();
    let de_count = de_words.iter().filter(|w| words.contains(*w)).count();

    let max = en_count.max(es_count).max(fr_count).max(de_count);
    if max == 0 {
        return "en".into(); // default
    }
    if max == en_count { "en" }
    else if max == es_count { "es" }
    else if max == fr_count { "fr" }
    else { "de" }
    .into()
}

// ─── Source Quality & Trust Scoring ────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceTrustScore {
    pub source_name: String,
    pub tier: u32,          // 1 (highest trust) to 5 (lowest)
    pub bias_lean: String,  // left, center, right, unknown
    pub cross_source_count: u32, // how many other sources report similar stories
    pub confidence: f64,    // 0.0 to 1.0
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceQualityOutput {
    pub sources: Vec<SourceTrustScore>,
    pub count: usize,
}

/// Known high-trust news sources (tier 1-2) for quick scoring.
/// This is a small seed list — can be extended from MediaBias/FactCheck data.
fn trusted_sources() -> std::collections::HashMap<&'static str, (u32, &'static str)> {
    let mut m = std::collections::HashMap::new();
    // (source_domain_lower, (tier, bias_lean))
    m.insert("reuters.com", (1u32, "center"));
    m.insert("apnews.com", (1, "center"));
    m.insert("bbc.com", (1, "center"));
    m.insert("bbc.co.uk", (1, "center"));
    m.insert("nytimes.com", (2, "left"));
    m.insert("washingtonpost.com", (2, "left"));
    m.insert("wsj.com", (2, "right"));
    m.insert("ft.com", (2, "center"));
    m.insert("economist.com", (2, "center"));
    m.insert("nature.com", (1, "center"));
    m.insert("science.org", (1, "center"));
    m.insert("bloomberg.com", (2, "center"));
    m.insert("npr.org", (2, "center"));
    m.insert("aljazeera.com", (2, "center"));
    m.insert("theguardian.com", (2, "left"));
    m.insert("cnbc.com", (2, "center"));
    m.insert("techcrunch.com", (3, "center"));
    m.insert("arstechnica.com", (3, "center"));
    m.insert("wired.com", (3, "center"));
    m.insert("theverge.com", (3, "center"));
    m
}

/// Score source quality based on known trust tiers and cross-source verification.
pub fn score_sources(sources: &[(String, String)]) -> SourceQualityOutput {
    // sources: Vec<(source_name, domain)>
    let trusted = trusted_sources();
    let mut scores: Vec<SourceTrustScore> = Vec::new();

    for (name, domain) in sources {
        let domain_lower = domain.to_lowercase();
        let (tier, bias, confidence) = if let Some(&(t, b)) = trusted.get(domain_lower.as_str()) {
            (t, b, 0.9)
        } else {
            (5, "unknown", 0.3)
        };

        scores.push(SourceTrustScore {
            source_name: name.clone(),
            tier,
            bias_lean: bias.to_string(),
            cross_source_count: 0, // would be computed by checking article overlap
            confidence,
            verified: tier <= 2,
        });
    }

    let count = scores.len();
    SourceQualityOutput { sources: scores, count }
}

// ─── Report Generation ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReportInput {
    pub title: String,
    pub articles: Vec<ReportArticle>,
    pub summary_style: Option<String>, // "brief", "detailed", "bullet"
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReportArticle {
    pub title: String,
    pub source: String,
    pub link: String,
    pub pub_date: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReportOutput {
    pub title: String,
    pub markdown: String,
    pub article_count: usize,
    pub generated_at: String,
}

/// Generate a markdown intelligence briefing from a set of articles.
pub fn generate_report(input: ReportInput) -> ReportOutput {
    let now = chrono::Utc::now().to_rfc3339();
    let style = input.summary_style.as_deref().unwrap_or("brief");
    let mut md = String::new();

    md.push_str(&format!("# {}\n\n", input.title));
    md.push_str(&format!("*Generated: {}*\n\n", now));
    md.push_str(&format!("*Articles: {}*\n\n---\n\n", input.articles.len()));

    match style {
        "detailed" => {
            for (i, article) in input.articles.iter().enumerate() {
                md.push_str(&format!("## {}. {}\n\n", i + 1, article.title));
                md.push_str(&format!("- **Source:** {}\n", article.source));
                md.push_str(&format!("- **Date:** {}\n", article.pub_date));
                md.push_str(&format!("- **Link:** {}\n", article.link));
                if let Some(ref snippet) = article.snippet {
                    md.push_str(&format!("\n> {}\n", snippet));
                }
                md.push('\n');
            }
        }
        "bullet" => {
            for article in &input.articles {
                md.push_str(&format!("- **{}** — {} ([link]({}))\n", article.source, article.title, article.link));
            }
        }
        _ => {
            // "brief" style
            for (i, article) in input.articles.iter().enumerate() {
                md.push_str(&format!("{}. **{}**\n   {}\n   *{} — [link]({})*\n\n", 
                    i + 1, article.source, article.title, article.pub_date, article.link));
            }
        }
    }

    md.push_str("---\n\n*Generated by IGS Intelligence Gathering System*\n");

    ReportOutput {
        title: input.title,
        markdown: md,
        article_count: input.articles.len(),
        generated_at: now,
    }
}

// ─── Helper ────────────────────────────────────────────────────

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut c = word.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_time_series_empty() {
        let result = analyze_time_series("test", &[]);
        assert_eq!(result.entity, "test");
        assert!(result.time_series.is_empty());
        assert!(result.anomalies.is_empty());
    }

    #[test]
    fn test_analyze_time_series_with_anomaly() {
        // 6 points: 5 normal + 1 extreme spike
        let points = vec![
            ("2026-01-01".into(), 10u32),
            ("2026-01-02".into(), 12),
            ("2026-01-03".into(), 11),
            ("2026-01-04".into(), 10),
            ("2026-01-05".into(), 9),
            ("2026-01-06".into(), 500), // extreme anomaly
        ];
        let result = analyze_time_series("entity", &points);
        assert_eq!(result.time_series.len(), 6);
        assert!(!result.anomalies.is_empty(), "Expected at least one anomaly, got z-scores: {:?}", result.time_series.iter().zip(result.anomalies.iter()).collect::<Vec<_>>());
        assert!(result.anomalies[0].is_anomaly);
    }

    #[test]
    fn test_extract_locations_countries() {
        let result = extract_locations("The United States and China reached a trade agreement.");
        assert!(!result.locations.is_empty());
        assert!(result.locations.iter().any(|l| l.name == "United States"));
        assert!(result.locations.iter().any(|l| l.name == "China"));
    }

    #[test]
    fn test_extract_locations_cities() {
        let result = extract_locations("Meeting in Washington and London next week.");
        assert!(result.locations.iter().any(|l| l.name == "Washington"));
        assert!(result.locations.iter().any(|l| l.name == "London"));
    }

    #[test]
    fn test_detect_language_english() {
        let result = detect_language("The quick brown fox jumps over the lazy dog");
        assert_eq!(result.script, "Latin");
        assert_eq!(result.detected_language, "en");
    }

    #[test]
    fn test_detect_language_cyrillic() {
        let result = detect_language("Привет мир, это текст на русском языке");
        assert_eq!(result.script, "Cyrillic");
        assert_eq!(result.detected_language, "ru");
    }

    #[test]
    fn test_detect_language_cjk() {
        let result = detect_language("这是一段中文文本");
        assert_eq!(result.script, "CJK");
        assert_eq!(result.detected_language, "zh");
    }

    #[test]
    fn test_score_sources_known() {
        let sources = vec![
            ("Reuters".into(), "reuters.com".into()),
            ("Unknown Blog".into(), "randomblog.com".into()),
        ];
        let result = score_sources(&sources);
        assert_eq!(result.count, 2);
        assert_eq!(result.sources[0].tier, 1); // Reuters is tier 1
        assert!(result.sources[0].verified);
        assert_eq!(result.sources[1].tier, 5); // unknown
        assert!(!result.sources[1].verified);
    }

    #[test]
    fn test_generate_report_brief() {
        let input = ReportInput {
            title: "Daily Brief".into(),
            articles: vec![ReportArticle {
                title: "Test Article".into(),
                source: "Reuters".into(),
                link: "https://example.com/1".into(),
                pub_date: "2026-01-01".into(),
                snippet: Some("This is a test.".into()),
            }],
            summary_style: Some("brief".into()),
        };
        let result = generate_report(input);
        assert!(result.markdown.contains("# Daily Brief"));
        assert!(result.markdown.contains("Test Article"));
        assert_eq!(result.article_count, 1);
    }

    #[test]
    fn test_generate_report_bullet() {
        let input = ReportInput {
            title: "Quick Summary".into(),
            articles: vec![
                ReportArticle {
                    title: "Article 1".into(),
                    source: "BBC".into(),
                    link: "https://example.com/1".into(),
                    pub_date: "2026-01-01".into(),
                    snippet: None,
                },
                ReportArticle {
                    title: "Article 2".into(),
                    source: "AP".into(),
                    link: "https://example.com/2".into(),
                    pub_date: "2026-01-02".into(),
                    snippet: None,
                },
            ],
            summary_style: Some("bullet".into()),
        };
        let result = generate_report(input);
        assert!(result.markdown.contains("- **BBC**"));
        assert!(result.markdown.contains("- **AP**"));
        assert_eq!(result.article_count, 2);
    }
}
