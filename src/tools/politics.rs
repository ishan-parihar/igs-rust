use super::types::*;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;
use crate::error::{AppError, AppResult};

pub async fn politics_fec_candidates(input: PoliticsFecInput, http: &HttpClient) -> AppResult<PoliticsFecOutput> {
    let name = urlencoding(&input.name);
    let limit = input.limits.limit.unwrap_or(20).clamp(1, 100);

    let mut url = format!(
        "https://api.open.fec.gov/v1/candidates/?name={}&per_page={}&sort=name",
        name, limit
    );

    if let Some(ref office) = input.office {
        url = format!("{}&office={}", url, office);
    }
    if let Some(ref party) = input.party {
        url = format!("{}&party={}", url, party);
    }

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("FEC API error: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome else {
        return Err(AppError::other("unexpected cached response for bypass mode"));
    };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: {}", e))?;

    if let Some(err) = data["error"].as_str() {
        return Err(AppError::other(format!("FEC error: {}", err)));
    }

    let mut candidates = Vec::new();
    if let Some(results) = data["results"].as_array() {
        for c in results {
            candidates.push(FecCandidate {
                id: c["candidate_id"].as_str().unwrap_or("").to_string(),
                name: c["name"].as_str().unwrap_or("").to_string(),
                party: c["party_full"].as_str().unwrap_or("").to_string(),
                office: c["office_full"].as_str().unwrap_or("").to_string(),
                state: c["state"].as_str().unwrap_or("").to_string(),
                total_receipts: c["receipts"].as_f64().unwrap_or(0.0),
                total_disbursements: c["disbursements"].as_f64().unwrap_or(0.0),
                cash_on_hand: c["cash_on_hand_end"].as_f64().unwrap_or(0.0),
            });
        }
    }

    let total = data["pagination"]["count"]
        .as_u64()
        .unwrap_or(candidates.len() as u64) as usize;

    Ok(PoliticsFecOutput {
        query: input.name,
        total,
        candidates,
    })
}

pub async fn politics_fec_committees(
    input: PoliticsFecCommitteesInput,
    http: &HttpClient,
) -> AppResult<PoliticsFecCommitteesOutput> {
    let name = urlencoding(&input.name);
    let limit = input.limits.limit.unwrap_or(20).clamp(1, 100);

    let mut url = format!(
        "https://api.open.fec.gov/v1/committees/?name={}&per_page={}&sort=name",
        name, limit
    );

    if let Some(ref committee_type) = input.committee_type {
        url = format!("{}&committee_type={}", url, committee_type);
    }

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("FEC API error: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome else {
        return Err(AppError::other("unexpected cached response for bypass mode"));
    };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: {}", e))?;

    if let Some(err) = data["error"].as_str() {
        return Err(AppError::other(format!("FEC error: {}", err)));
    }

    let mut committees = Vec::new();
    if let Some(results) = data["results"].as_array() {
        for c in results {
            committees.push(FecCommittee {
                id: c["committee_id"].as_str().unwrap_or("").to_string(),
                name: c["name"].as_str().unwrap_or("").to_string(),
                committee_type: c["committee_type_full"].as_str().unwrap_or("").to_string(),
                party: c["party_full"].as_str().unwrap_or("").to_string(),
                state: c["state"].as_str().unwrap_or("").to_string(),
                total_receipts: c["receipts"].as_f64().unwrap_or(0.0),
                total_disbursements: c["disbursements"].as_f64().unwrap_or(0.0),
            });
        }
    }

    let total = data["pagination"]["count"]
        .as_u64()
        .unwrap_or(committees.len() as u64) as usize;

    Ok(PoliticsFecCommitteesOutput {
        query: input.name,
        total,
        committees,
    })
}
