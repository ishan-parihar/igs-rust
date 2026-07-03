use super::types::*;
use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;

pub async fn govt_bills(input: GovtBillsInput) -> Result<GovtBillsOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let query = urlencoding(&input.query);
    let congress = input.congress.unwrap_or(118);

    let url = format!(
        "https://api.congress.gov/v3/bill?api_key=DEMO_KEY&query={}&congress={}&limit=20&format=json",
        query, congress
    );

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("Congress.gov API error: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("bypass cache mode never returns Cached") };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: {}", e))?;

    let mut bills = Vec::new();
    if let Some(bills_arr) = data["bills"].as_array() {
        for b in bills_arr {
            bills.push(BillEntry {
                number: b["number"].as_u64().unwrap_or(0) as u32,
                title: b["title"].as_str().unwrap_or("").to_string(),
                sponsor: b["sponsors"]
                    .as_array()
                    .and_then(|s| s.first())
                    .and_then(|s| s["fullName"].as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                introduced_date: b["introducedDate"].as_str().unwrap_or("").to_string(),
                latest_action: b["latestAction"].as_str().unwrap_or("").to_string(),
                url: b["url"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(GovtBillsOutput {
        query: input.query,
        congress,
        total: bills.len(),
        bills,
    })
}

pub async fn govt_regulations(
    input: GovtRegulationsInput,
) -> Result<GovtRegulationsOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let query = urlencoding(&input.query);

    let url = format!(
        "https://api.federalregister.gov/v1/articles.json?per_page=20&conditions[term]={}",
        query
    );

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("Federal Register API error: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("bypass cache mode never returns Cached") };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: {}", e))?;

    let mut regulations = Vec::new();
    if let Some(results) = data["results"].as_array() {
        for r in results {
            regulations.push(RegulationEntry {
                document_number: r["document_number"].as_str().unwrap_or("").to_string(),
                title: r["title"].as_str().unwrap_or("").to_string(),
                abstract_text: r["abstract"].as_str().unwrap_or("").to_string(),
                publication_date: r["publication_date"].as_str().unwrap_or("").to_string(),
                agency: r["agencies"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|a| a["name"].as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                url: r["html_url"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(GovtRegulationsOutput {
        query: input.query,
        total: regulations.len(),
        regulations,
    })
}
