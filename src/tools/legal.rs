use crate::config;
use super::helpers::urlencoding;
use super::types::*;

pub async fn legal_search_cases(input: LegalSearchInput) -> Result<LegalSearchOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let api_key = settings.courtlistener.as_ref()
        .and_then(|cl| cl.api_key.as_deref())
        .ok_or_else(|| "CourtListener API token not configured. Set courtlistener.apiKey in settings.yml.".to_string())?;

    let client = reqwest::Client::new();
    let query = urlencoding(&input.query);
    let limit = input.limit.unwrap_or(20).clamp(1, 100);

    let mut url = format!(
        "https://www.courtlistener.com/api/rest/v4/search/?q={}&type=o&order_by=dateFiled%20desc&format=json",
        query
    );

    if let Some(ref court) = input.court {
        url = format!("{}&court={}", url, urlencoding(court));
    }

    url = format!("{}&page_size={}", url, limit);

    let resp = client
        .get(&url)
        .header("Authorization", format!("Token {}", api_key))
        .header("User-Agent", "IGS-MCP/0.4")
        .send()
        .await
        .map_err(|e| format!("CourtListener API error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("CourtListener API returned status {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    if let Some(err) = data["detail"].as_str() {
        return Err(format!("CourtListener error: {}", err));
    }

    let mut cases = Vec::new();
    if let Some(results) = data["results"].as_array() {
        for r in results {
            cases.push(LegalCase {
                id: r["id"].as_u64().unwrap_or(0) as u32,
                case_name: r["caseName"].as_str().unwrap_or("").to_string(),
                court: r["court"].as_str().unwrap_or("").to_string(),
                date_filed: r["dateFiled"].as_str().unwrap_or("").to_string(),
                citation: r["citeCount"].as_u64().unwrap_or(0),
                url: format!("https://www.courtlistener.com{}", r["absolute_url"].as_str().unwrap_or("")),
            });
        }
    }

    let total = data["count"].as_u64().unwrap_or(cases.len() as u64) as usize;

    Ok(LegalSearchOutput {
        query: input.query,
        total,
        cases,
    })
}

pub async fn legal_case_details(input: LegalCaseDetailsInput) -> Result<LegalCaseDetailsOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let api_key = settings.courtlistener.as_ref()
        .and_then(|cl| cl.api_key.as_deref())
        .ok_or_else(|| "CourtListener API token not configured. Set courtlistener.apiKey in settings.yml.".to_string())?;

    let client = reqwest::Client::new();

    let url = format!(
        "https://www.courtlistener.com/api/rest/v4/dockets/{}/?format=json",
        input.case_id
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Token {}", api_key))
        .header("User-Agent", "IGS-MCP/0.4")
        .send()
        .await
        .map_err(|e| format!("CourtListener API error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("CourtListener API returned status {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    if let Some(err) = data["detail"].as_str() {
        return Err(format!("CourtListener error: {}", err));
    }

    let judges = data["judges"].as_array()
        .map(|arr| arr.iter().filter_map(|j| j.as_str().map(String::from)).collect())
        .unwrap_or_default();

    Ok(LegalCaseDetailsOutput {
        id: data["id"].as_u64().unwrap_or(0) as u32,
        case_name: data["caseName"].as_str().unwrap_or("").to_string(),
        court: data["court"].as_str().unwrap_or("").to_string(),
        date_filed: data["dateFiled"].as_str().unwrap_or("").to_string(),
        date_terminated: data["dateTerminated"].as_str().unwrap_or("").to_string(),
        judges,
        nature_of_suit: data["natureOfSuit"].as_str().unwrap_or("").to_string(),
        url: format!("https://www.courtlistener.com{}", data["absolute_url"].as_str().unwrap_or("")),
    })
}
