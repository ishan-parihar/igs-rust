use super::helpers::urlencoding;
use super::types::*;
use anyhow::Result;

pub async fn govt_bills(input: GovtBillsInput) -> Result<GovtBillsOutput, String> {
    let client = reqwest::Client::new();
    let query = urlencoding(&input.query);
    let congress = input.congress.unwrap_or(118);

    let url = format!(
        "https://api.congress.gov/v3/bill?api_key=DEMO_KEY&query={}&congress={}&limit=20&format=json",
        query, congress
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "IGS-MCP/0.4")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Congress.gov API returned {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON error: {}", e))?;
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
                introduced_date: b["introducedDate"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                latest_action: b["latestAction"].as_str().unwrap_or("").to_string(),
                url: b["url"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(GovtBillsOutput {
        query: input.query,
        congress: congress as u32,
        total: bills.len(),
        bills,
    })
}

pub async fn govt_regulations(input: GovtRegulationsInput) -> Result<GovtRegulationsOutput, String> {
    let client = reqwest::Client::new();
    let query = urlencoding(&input.query);

    let url = format!(
        "https://api.federalregister.gov/v1/articles.json?per_page=20&conditions[term]={}",
        query
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "IGS-MCP/0.4")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Federal Register API returned {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON error: {}", e))?;
    let mut regulations = Vec::new();

    if let Some(results) = data["results"].as_array() {
        for r in results {
            regulations.push(RegulationEntry {
                document_number: r["document_number"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                title: r["title"].as_str().unwrap_or("").to_string(),
                abstract_text: r["abstract"].as_str().unwrap_or("").to_string(),
                publication_date: r["publication_date"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
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
