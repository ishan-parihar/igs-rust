use super::helpers::urlencoding;
use super::types::*;
use anyhow::Result;

pub async fn patents_search(input: PatentSearchInput) -> Result<PatentSearchOutput, String> {
    let client = reqwest::Client::new();
    let query = urlencoding(&input.query);
    let years = input.years_back.unwrap_or(5);
    let office = input.office.as_deref().unwrap_or("USPTO");

    match office {
        "USPTO" => {
            let fields = r#"["patent_number","patent_title","patent_date","patent_abstract"]"#;
            let opts = format!(r#"{{"per_page":{}}}"#, years);
            let url = format!(
                "https://api.patentsview.org/patents/query?q={}&f={}&o={}",
                query, fields, opts
            );
            let resp = client
                .get(&url)
                .header("User-Agent", "IGS-MCP/0.4")
                .send()
                .await
                .map_err(|e| format!("HTTP error: {}", e))?;

            if !resp.status().is_success() {
                return Err(format!("PatentsView API returned {}", resp.status()));
            }

            let data: serde_json::Value =
                resp.json().await.map_err(|e| format!("JSON error: {}", e))?;
            let mut patents = Vec::new();

            if let Some(patents_arr) = data["patents"].as_array() {
                for p in patents_arr {
                    let num = p["patent_number"].as_str().unwrap_or("");
                    patents.push(PatentEntry {
                        id: num.to_string(),
                        title: p["patent_title"].as_str().unwrap_or("").to_string(),
                        date: p["patent_date"].as_str().unwrap_or("").to_string(),
                        abstract_text: p["patent_abstract"].as_str().unwrap_or("").to_string(),
                        office: "USPTO".to_string(),
                        url: format!("https://patents.google.com/patent/{}", num),
                    });
                }
            }

            Ok(PatentSearchOutput {
                query: input.query,
                office: office.to_string(),
                total: patents.len(),
                patents,
            })
        }
        _ => Err(format!(
            "Unsupported patent office: {}. Use USPTO.",
            office
        )),
    }
}

pub async fn patents_details(input: PatentDetailsInput) -> Result<PatentDetailsOutput, String> {
    let client = reqwest::Client::new();
    let patent_id = &input.patent_id;

    let q = format!(r#"{{"patent_number":"{}"}}"#, patent_id);
    let fields = r#"["patent_number","patent_title","patent_date","patent_abstract","patent_claims"]"#;
    let url = format!(
        "https://api.patentsview.org/patents/query?q={}&f={}",
        q, fields
    );

    let resp = client
        .get(&url)
        .header("User-Agent", "IGS-MCP/0.4")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("PatentsView API returned {}", resp.status()));
    }

    let data: serde_json::Value =
        resp.json().await.map_err(|e| format!("JSON error: {}", e))?;

    if let Some(patents) = data["patents"].as_array().and_then(|p| p.first()) {
        Ok(PatentDetailsOutput {
            id: patents["patent_number"].as_str().unwrap_or("").to_string(),
            title: patents["patent_title"].as_str().unwrap_or("").to_string(),
            date: patents["patent_date"].as_str().unwrap_or("").to_string(),
            abstract_text: patents["patent_abstract"].as_str().unwrap_or("").to_string(),
            claims: patents["patent_claims"].as_u64().unwrap_or(0) as u32,
            url: format!("https://patents.google.com/patent/{}", patent_id),
        })
    } else {
        Err(format!("Patent {} not found", patent_id))
    }
}
