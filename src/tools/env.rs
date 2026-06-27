use super::types::*;
use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;

pub async fn env_epa_facilities(
    input: EnvEpaFacilitiesInput,
) -> Result<EnvEpaFacilitiesOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let state = input.state.as_deref().unwrap_or("US");
    let limit = input.limits.limit.unwrap_or(20).clamp(1, 100);

    let mut url = format!(
        "https://data.epa.gov/efservice/SEATTLE_ECHO_FACILITY/STATE_CODE/{}/JSON/rows/0:{}/LIST",
        urlencoding(state),
        limit
    );

    if let Some(ref name) = input.name {
        url = format!(
            "https://data.epa.gov/efservice/SEATTLE_ECHO_FACILITY/FACILITY_NAME/{}/rows/0:{}/LIST",
            urlencoding(name),
            limit
        );
    }

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("EPA API error: {}", e))?;

    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("EPA returned cached response".into()),
    };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: ${e}"))?;

    let mut facilities = Vec::new();
    if let Some(results) = data.as_array() {
        for r in results {
            facilities.push(EpaFacility {
                name: r["FACILITY_NAME"].as_str().unwrap_or("").to_string(),
                address: r["LOCATION_ADDRESS"].as_str().unwrap_or("").to_string(),
                city: r["CITY_NAME"].as_str().unwrap_or("").to_string(),
                state: r["STATE_CODE"].as_str().unwrap_or("").to_string(),
                zip: r["ZIP_CODE"].as_str().unwrap_or("").to_string(),
                county: r["COUNTY_NAME"].as_str().unwrap_or("").to_string(),
                latitude: r["LATITUDE"].as_f64().unwrap_or(0.0),
                longitude: r["LONGITUDE"].as_f64().unwrap_or(0.0),
                registry_id: r["REGISTRY_ID"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(EnvEpaFacilitiesOutput {
        query: state.to_string(),
        total: facilities.len(),
        facilities,
    })
}

pub async fn env_epa_emissions(
    input: EnvEpaEmissionsInput,
) -> Result<EnvEpaEmissionsOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let state = input.state.as_deref().unwrap_or("US");
    let limit = input.limits.limit.unwrap_or(20).clamp(1, 100);

    let url = format!(
        "https://data.epa.gov/efservice/TRI_FACILITY/ST/rows/0:{}/JSON",
        limit
    );

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("EPA API error: {}", e))?;

    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("EPA returned cached response".into()),
    };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: ${e}"))?;

    let mut facilities = Vec::new();
    if let Some(results) = data.as_array() {
        for r in results {
            facilities.push(EpaEmissionsFacility {
                name: r["FACILITY_NAME"].as_str().unwrap_or("").to_string(),
                state: r["ST"].as_str().unwrap_or("").to_string(),
                county: r["COUNTY"].as_str().unwrap_or("").to_string(),
                latitude: r["LATITUDE"].as_f64().unwrap_or(0.0),
                longitude: r["LONGITUDE"].as_f64().unwrap_or(0.0),
                registry_id: r["TRI_FACILITY_ID"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(EnvEpaEmissionsOutput {
        query: state.to_string(),
        total: facilities.len(),
        facilities,
    })
}
