use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;
use super::types::*;

pub async fn patents_search(input: PatentSearchInput) -> Result<PatentSearchOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    
    let office = input.office.as_deref().unwrap_or("USPTO");
    let limit = input.years_back.unwrap_or(20).clamp(1, 100);

    match office {
        "USPTO" => {
            let query_json = serde_json::json!({"_contains": input.query});
            let fields = r#"["patent_number","patent_title","patent_date","patent_abstract"]"#;
            let opts = format!(r#"{{"per_page":{}}}"#, limit);
            
            let q = urlencoding(&query_json.to_string());
            let f = urlencoding(fields);
            let o = urlencoding(&opts);
            let url = format!(
                "https://api.patentsview.org/patents/query?q={}&f={}&o={}",
                q, f, o
            );
            
            let outcome = http.fetch(&url, None, "bypass").await
                .map_err(|e| format!("PatentsView API error: {}", e))?;

            let resp = match outcome {
                http_mod::FetchOutcome::Response(r, _, _) => r,
                _ => return Err("PatentsView returned cached response".into()),
            };

            let data: serde_json::Value = serde_json::from_str(&resp.body_text)
                .map_err(|e| format!("JSON parse error: {}", e))?;

            if let Some(err) = data["error"].as_str() {
                return Err(format!("PatentsView error: {}", err));
            }

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
        _ => Err(format!("Unsupported patent office: {}. Use USPTO.", office)),
    }
}

pub async fn patents_details(input: PatentDetailsInput) -> Result<PatentDetailsOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    
    let query_json = serde_json::json!({"patent_number": input.patent_id});
    let fields = r#"["patent_number","patent_title","patent_date","patent_abstract","patent_claims"]"#;
    
    let q = urlencoding(&query_json.to_string());
    let f = urlencoding(fields);
    let url = format!(
        "https://api.patentsview.org/patents/query?q={}&f={}",
        q, f
    );

    let outcome = http.fetch(&url, None, "bypass").await
        .map_err(|e| format!("PatentsView API error: {}", e))?;

    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("PatentsView returned cached response".into()),
    };

    let data: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    if let Some(patents) = data["patents"].as_array().and_then(|p| p.first()) {
        Ok(PatentDetailsOutput {
            id: patents["patent_number"].as_str().unwrap_or("").to_string(),
            title: patents["patent_title"].as_str().unwrap_or("").to_string(),
            date: patents["patent_date"].as_str().unwrap_or("").to_string(),
            abstract_text: patents["patent_abstract"].as_str().unwrap_or("").to_string(),
            claims: patents["patent_claims"].as_u64().unwrap_or(0) as u32,
            url: format!("https://patents.google.com/patent/{}", input.patent_id),
        })
    } else {
        Err(format!("Patent {} not found", input.patent_id))
    }
}
