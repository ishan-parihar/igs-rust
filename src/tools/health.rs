use super::helpers::urlencoding;
use super::types::*;

pub async fn health_cdc_leading_causes(input: HealthCdcInput) -> Result<HealthCdcOutput, String> {
    let client = reqwest::Client::new();
    let year = input.year.unwrap_or(2021);
    let limit = input.limit.unwrap_or(20).min(100);

    let mut url = format!(
        "https://data.cdc.gov/resource/3y38-azbh.json?$where=year='{}'&$order=deaths DESC&$limit={}",
        year, limit
    );

    if let Some(ref state) = input.state {
        let state_enc = urlencoding(state);
        url = format!(
            "https://data.cdc.gov/resource/3y38-azbh.json?$where=year='{}' AND state='{}'&$order=deaths DESC&$limit={}",
            year, state_enc, limit
        );
    }

    let resp = client
        .get(&url)
        .header("User-Agent", "IGS-MCP/0.4")
        .send()
        .await
        .map_err(|e| format!("CDC API error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("CDC API returned {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let mut causes = Vec::new();
    if let Some(results) = data.as_array() {
        for r in results {
            causes.push(HealthCause {
                cause: r["113_cause_of_death"].as_str().unwrap_or("").to_string(),
                state: r["state"].as_str().unwrap_or("").to_string(),
                year: r["year"].as_str().unwrap_or("").to_string(),
                deaths: r["deaths"].as_u64().unwrap_or(0),
                age_adjusted_rate: r["age_adjusted_death_rate"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }

    Ok(HealthCdcOutput {
        query: format!(
            "Leading causes of death ({}, {})",
            input.state.as_deref().unwrap_or("US"),
            year
        ),
        total: causes.len(),
        causes,
    })
}

pub async fn health_cdc_covid(input: HealthCdcCovidInput) -> Result<HealthCdcCovidOutput, String> {
    let client = reqwest::Client::new();
    let limit = input.limit.unwrap_or(20).min(100);

    let mut url = format!(
        "https://data.cdc.gov/resource/9mfq-cb36.json?$order=submission_date DESC&$limit={}",
        limit
    );

    if let Some(ref state) = input.state {
        let state_enc = urlencoding(state);
        url = format!("{}&state={}", url, state_enc);
    }

    let resp = client
        .get(&url)
        .header("User-Agent", "IGS-MCP/0.4")
        .send()
        .await
        .map_err(|e| format!("CDC API error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("CDC API returned {}", resp.status()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let mut records = Vec::new();
    if let Some(results) = data.as_array() {
        for r in results {
            records.push(HealthCovidRecord {
                state: r["state"].as_str().unwrap_or("").to_string(),
                date: r["submission_date"].as_str().unwrap_or("").to_string(),
                cases: r["tot_cases"].as_u64().unwrap_or(0),
                deaths: r["tot_death"].as_u64().unwrap_or(0),
                new_cases: r["new_case"].as_u64().unwrap_or(0),
                new_deaths: r["new_death"].as_u64().unwrap_or(0),
            });
        }
    }

    Ok(HealthCdcCovidOutput {
        query: format!(
            "COVID-19 data ({})",
            input.state.as_deref().unwrap_or("US")
        ),
        total: records.len(),
        records,
    })
}
