//! P4: Data source expansion — new intelligence APIs.
//!
//! Provides:
//! - OpenAlex: 250M+ academic works (free, replaces Semantic Scholar for breadth)
//! - Shodan: exposed services, CVEs per IP (freemium)
//! - HaveIBeenPwned: breach data for OSINT (freemium)
//! - ACLED: armed conflict locations & events (free for research)

use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;
use crate::tools::types::LimitInput;
use crate::tools::types_base::OutputOptions;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ─── OpenAlex ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OpenAlexSearchInput {
    pub query: String,
    pub limit: Option<u32>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OpenAlexWork {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub citation_count: Option<i32>,
    pub doi: Option<String>,
    pub url: String,
    pub abstract_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OpenAlexSearchOutput {
    pub query: String,
    pub total: usize,
    pub works: Vec<OpenAlexWork>,
}

/// Search OpenAlex for 250M+ academic works. Free API, no key required.
pub async fn openalex_search(input: OpenAlexSearchInput) -> Result<OpenAlexSearchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let limit = input.limit.unwrap_or(25).min(200);
    let query_enc = urlencoding(&input.query);
    let url = format!(
        "https://api.openalex.org/works?search={}&per-page={}&select=id,title,authorships,publication_year,cited_by_count,doi,abstract_inverted_index",
        query_enc, limit
    );

    let outcome = http.fetch(&url, None, "bypass").await
        .map_err(|e| format!("OpenAlex API error: {}", e))?;
    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("bypass never returns Cached") };

    let json: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("OpenAlex JSON parse error: {}", e))?;

    let works: Vec<OpenAlexWork> = json["results"]
        .as_array()
        .map(|arr| {
            arr.iter().map(|w| {
                let authors: Vec<String> = w["authorships"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|au| au["author"]["display_name"].as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let abstract_text = w["abstract_inverted_index"]
                    .as_object()
                    .map(|inv_idx| {
                        let mut positions: Vec<(usize, &str)> = Vec::new();
                        for (word, pos_list) in inv_idx {
                            if let Some(positions_arr) = pos_list.as_array() {
                                for pos in positions_arr {
                                    if let Some(p) = pos.as_u64() {
                                        positions.push((p as usize, word));
                                    }
                                }
                            }
                        }
                        positions.sort_by_key(|(p, _)| *p);
                        positions.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" ")
                    });

                OpenAlexWork {
                    id: w["id"].as_str().unwrap_or("").to_string(),
                    title: w["title"].as_str().unwrap_or("").to_string(),
                    authors,
                    year: w["publication_year"].as_i64().map(|y| y as i32),
                    citation_count: w["cited_by_count"].as_i64().map(|c| c as i32),
                    doi: w["doi"].as_str().map(|s| s.to_string()),
                    url: w["id"].as_str().unwrap_or("").to_string(),
                    abstract_text,
                }
            }).collect()
        })
        .unwrap_or_default();

    let total = works.len();
    Ok(OpenAlexSearchOutput {
        query: input.query,
        total,
        works,
    })
}

// ─── Shodan ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShodanSearchInput {
    /// Shodan search query (e.g., "apache country:US")
    pub query: String,
    /// Shodan API key
    pub api_key: String,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShodanResult {
    pub ip: String,
    pub port: u16,
    pub organization: Option<String>,
    pub country: Option<String>,
    pub product: Option<String>,
    pub vulns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShodanSearchOutput {
    pub query: String,
    pub total: usize,
    pub results: Vec<ShodanResult>,
}

/// Search Shodan for exposed services. Requires API key.
pub async fn shodan_search(input: ShodanSearchInput) -> Result<ShodanSearchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let limit = input.limits.limit.unwrap_or(25).min(100);
    let query_enc = urlencoding(&input.query);
    let url = format!(
        "https://api.shodan.io/shodan/host/search?key={}&query={}&page=1",
        input.api_key, query_enc
    );
    let _ = limit; // Shodan pagination is page-based

    let outcome = http.fetch(&url, None, "bypass").await
        .map_err(|e| format!("Shodan API error: {}", e))?;
    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("bypass never returns Cached") };

    let json: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("Shodan JSON parse error: {}", e))?;

    let results: Vec<ShodanResult> = json["matches"]
        .as_array()
        .map(|arr| {
            arr.iter().map(|m| {
                let vulns: Vec<String> = m["vulns"]
                    .as_array()
                    .map(|v| v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default();

                ShodanResult {
                    ip: m["ip_str"].as_str().unwrap_or("").to_string(),
                    port: m["port"].as_u64().unwrap_or(0) as u16,
                    organization: m["org"].as_str().map(|s| s.to_string()),
                    country: m["location"]["country_name"].as_str().map(|s| s.to_string()),
                    product: m["product"].as_str().map(|s| s.to_string()),
                    vulns,
                }
            }).collect()
        })
        .unwrap_or_default();

    let total = results.len();
    Ok(ShodanSearchOutput {
        query: input.query,
        total,
        results,
    })
}

// ─── HaveIBeenPwned ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HibpBreachInput {
    /// Email address to check
    pub email: String,
    /// HIBP API key
    pub api_key: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HibpBreach {
    pub name: String,
    pub domain: String,
    pub breach_date: String,
    pub pwn_count: u64,
    pub description: Option<String>,
    pub data_classes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HibpBreachOutput {
    pub email: String,
    pub total: usize,
    pub breaches: Vec<HibpBreach>,
}

/// Check if an email has been in any known data breach via HaveIBeenPwned.
pub async fn hibp_check(input: HibpBreachInput) -> Result<HibpBreachOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let url = format!("https://haveibeenpwned.com/api/v3/breachedaccount/{}", input.email);
    let headers = std::collections::HashMap::from([
        ("hibp-api-key".into(), input.api_key),
        ("user-agent".into(), "IGS-MCP".into()),
    ]);

    let outcome = http.fetch(&url, Some(&headers), "bypass").await
        .map_err(|e| format!("HIBP API error: {}", e))?;
    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("bypass never returns Cached") };

    // 404 means no breaches — that's a success
    if resp.status == 404 {
        return Ok(HibpBreachOutput {
            email: input.email,
            total: 0,
            breaches: vec![],
        });
    }

    let breaches_arr: Vec<serde_json::Value> = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("HIBP JSON parse error: {}", e))?;

    let breaches: Vec<HibpBreach> = breaches_arr
        .iter()
        .map(|b| HibpBreach {
            name: b["Name"].as_str().unwrap_or("").to_string(),
            domain: b["Domain"].as_str().unwrap_or("").to_string(),
            breach_date: b["BreachDate"].as_str().unwrap_or("").to_string(),
            pwn_count: b["PwnCount"].as_u64().unwrap_or(0),
            description: b["Description"].as_str().map(|s| s.to_string()),
            data_classes: b["DataClasses"]
                .as_array()
                .map(|dc| dc.iter().filter_map(|d| d.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
        })
        .collect();

    let total = breaches.len();
    Ok(HibpBreachOutput {
        email: input.email,
        total,
        breaches,
    })
}

// ─── ACLED (Armed Conflict Location & Event Data) ─────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AcledSearchInput {
    /// Country name or code (e.g., "Ukraine", "SY")
    pub country: Option<String>,
    /// Event type filter (e.g., "Battles", "Protests", "Violence against civilians")
    pub event_type: Option<String>,
    /// Start date (YYYY-MM-DD)
    pub start_date: Option<String>,
    /// End date (YYYY-MM-DD)
    pub end_date: Option<String>,
    /// ACLED API key (free for research — register at acleddata.com)
    pub api_key: String,
    /// ACLED email (required by API)
    pub email: String,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AcledEvent {
    pub date: String,
    pub event_type: String,
    pub country: String,
    pub location: String,
    pub actor1: String,
    pub actor2: Option<String>,
    pub fatalities: u32,
    pub notes: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AcledSearchOutput {
    pub total: usize,
    pub events: Vec<AcledEvent>,
}

/// Search ACLED for armed conflict events. Requires free API key + email.
pub async fn acled_search(input: AcledSearchInput) -> Result<AcledSearchOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let limit = input.limits.limit.unwrap_or(50).min(500);
    let mut url = format!(
        "https://api.acleddata.com/acled/read?key={}&email={}&limit={}",
        input.api_key, input.email, limit
    );

    if let Some(ref country) = input.country {
        url.push_str(&format!("&country={}", urlencoding(country)));
    }
    if let Some(ref event_type) = input.event_type {
        url.push_str(&format!("&event_type={}", urlencoding(event_type)));
    }
    if let Some(ref start) = input.start_date {
        url.push_str(&format!("&event_date={}|{}", start, input.end_date.as_deref().unwrap_or("")));
    }

    let outcome = http.fetch(&url, None, "bypass").await
        .map_err(|e| format!("ACLED API error: {}", e))?;
    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("bypass never returns Cached") };

    let json: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("ACLED JSON parse error: {}", e))?;

    let events: Vec<AcledEvent> = json["data"]
        .as_array()
        .map(|arr| {
            arr.iter().map(|e| AcledEvent {
                date: e["event_date"].as_str().unwrap_or("").to_string(),
                event_type: e["event_type"].as_str().unwrap_or("").to_string(),
                country: e["country"].as_str().unwrap_or("").to_string(),
                location: e["location"].as_str().unwrap_or("").to_string(),
                actor1: e["actor1"].as_str().unwrap_or("").to_string(),
                actor2: e["actor2"].as_str().map(|s| s.to_string()),
                fatalities: e["fatalities"].as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                notes: e["notes"].as_str().unwrap_or("").to_string(),
                latitude: e["latitude"].as_str().and_then(|s| s.parse().ok()),
                longitude: e["longitude"].as_str().and_then(|s| s.parse().ok()),
            }).collect()
        })
        .unwrap_or_default();

    let total = events.len();
    Ok(AcledSearchOutput { total, events })
}
