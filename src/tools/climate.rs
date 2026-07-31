use super::types::*;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;
use std::collections::HashMap;
use crate::error::{AppError, AppResult};

pub async fn climate_noaa_observations(
    input: ClimateNoaaInput,
    http: &HttpClient,
    settings: &crate::types::Settings,
) -> AppResult<ClimateNoaaOutput> {
    let api_key = settings
        .noaa
        .as_ref()
        .and_then(|n| n.api_key.as_deref())
        .ok_or_else(|| {
            "NOAA API token not configured. Set noaa.apiKey in settings.yml.".to_string()
        })?;

    let dataset = input.dataset.as_deref().unwrap_or("GHCND");
    let location = input.location.as_deref().unwrap_or("FIPS:US");
    let start_date = input.start_date.as_deref().unwrap_or("2024-01-01");
    let end_date = input.end_date.as_deref().unwrap_or("2024-01-07");
    let limit = input.limit.unwrap_or(20).clamp(1, 1000);

    let mut url = format!(
        "https://www.ncei.noaa.gov/cdo-web/api/v2/data?datasetid={}&locationid={}&startdate={}&enddate={}&limit={}&datatypeid=TMAX,TMIN,PRCP",
        dataset, urlencoding(location), start_date, end_date, limit
    );

    if let Some(ref station) = input.station {
        url = format!("{}&stationid={}", url, urlencoding(station));
    }

    let mut headers = HashMap::new();
    headers.insert("token".to_string(), api_key.to_string());

    let outcome = http
        .fetch(&url, Some(&headers), "bypass")
        .await
        .map_err(|e| format!("NOAA API error: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome else {
        return Err(AppError::other("unexpected cached response for bypass mode"));
    };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: ${e}"))?;

    if let Some(err) = data["message"].as_str() {
        return Err(AppError::other(format!("NOAA error: {}", err)));
    }

    let mut observations = Vec::new();
    if let Some(results) = data["results"].as_array() {
        for r in results {
            observations.push(NoaaObservation {
                date: r["date"].as_str().unwrap_or("").to_string(),
                station: r["station"].as_str().unwrap_or("").to_string(),
                datatype: r["datatype"].as_str().unwrap_or("").to_string(),
                value: r["value"].as_f64().unwrap_or(0.0),
                attributes: r["attributes"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(ClimateNoaaOutput {
        query: format!("{} from {} to {}", dataset, start_date, end_date),
        total: observations.len(),
        observations,
    })
}

pub async fn climate_noaa_stations(
    input: ClimateNoaaStationsInput,
    http: &HttpClient,
    settings: &crate::types::Settings,
) -> AppResult<ClimateNoaaStationsOutput> {
    let api_key = settings
        .noaa
        .as_ref()
        .and_then(|n| n.api_key.as_deref())
        .ok_or_else(|| {
            "NOAA API token not configured. Set noaa.apiKey in settings.yml.".to_string()
        })?;

    let location = input.location.as_deref().unwrap_or("FIPS:US");
    let limit = input.limit.unwrap_or(20).clamp(1, 1000);

    let url = format!(
        "https://www.ncei.noaa.gov/cdo-web/api/v2/stations?locationid={}&limit={}&sortfield=datacoverage&sortorder=desc",
        urlencoding(location), limit
    );

    let mut headers = HashMap::new();
    headers.insert("token".to_string(), api_key.to_string());

    let outcome = http
        .fetch(&url, Some(&headers), "bypass")
        .await
        .map_err(|e| format!("NOAA API error: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome else {
        return Err(AppError::other("unexpected cached response for bypass mode"));
    };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: ${e}"))?;

    if let Some(err) = data["message"].as_str() {
        return Err(AppError::other(format!("NOAA error: {}", err)));
    }

    let mut stations = Vec::new();
    if let Some(results) = data["results"].as_array() {
        for r in results {
            stations.push(NoaaStation {
                id: r["id"].as_str().unwrap_or("").to_string(),
                name: r["name"].as_str().unwrap_or("").to_string(),
                latitude: r["latitude"].as_f64().unwrap_or(0.0),
                longitude: r["longitude"].as_f64().unwrap_or(0.0),
                elevation: r["elevation"].as_f64().unwrap_or(0.0),
                mindate: r["mindate"].as_str().unwrap_or("").to_string(),
                maxdate: r["maxdate"].as_str().unwrap_or("").to_string(),
                datacoverage: r["datacoverage"].as_f64().unwrap_or(0.0),
            });
        }
    }

    Ok(ClimateNoaaStationsOutput {
        query: location.to_string(),
        total: stations.len(),
        stations,
    })
}
