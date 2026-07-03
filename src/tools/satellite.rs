use super::types::*;
use crate::config;
use crate::http::{self as http_mod, HttpClient};

pub async fn satellite_firms_fires(
    input: SatelliteFirmsInput,
) -> Result<SatelliteFirmsOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let west = input.west;
    let south = input.south;
    let east = input.east;
    let north = input.north;
    let source = input.source.as_deref().unwrap_or("VIIRS_SNPP_NRT");

    // Validate bounding box
    if west >= east || south >= north {
        return Err("Invalid bounding box: west must be < east, south must be < north".into());
    }

    let url = format!(
        "https://firms.modaps.eosdis.nasa.gov/api/area/csv/DEMO_KEY/{}/{},{},{},{}/1",
        source, west, south, east, north
    );

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("NASA FIRMS API error: {}", e))?;

    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
        else { unreachable!("bypass cache mode never returns Cached") };

    // Parse CSV response
    let lines: Vec<&str> = resp.body_text.lines().collect();
    if lines.len() < 2 {
        return Ok(SatelliteFirmsOutput {
            query: format!("{},{},{},{}", west, south, east, north),
            source: source.to_string(),
            total: 0,
            hotspots: vec![],
        });
    }

    // Parse header
    let headers: Vec<&str> = lines[0].split(',').map(|s| s.trim()).collect();
    let mut hotspots = Vec::new();

    for line in &lines[1..] {
        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if fields.len() < headers.len() {
            continue;
        }

        let get_field = |name: &str| -> String {
            headers
                .iter()
                .position(|h| *h == name)
                .and_then(|i| fields.get(i))
                .unwrap_or(&"")
                .to_string()
        };

        hotspots.push(FireHotspot {
            latitude: get_field("latitude").parse().unwrap_or(0.0),
            longitude: get_field("longitude").parse().unwrap_or(0.0),
            bright_ti4: get_field("bright_ti4").parse().unwrap_or(0.0),
            scan: get_field("scan").parse().unwrap_or(0.0),
            track: get_field("track").parse().unwrap_or(0.0),
            acq_date: get_field("acq_date"),
            acq_time: get_field("acq_time"),
            satellite: get_field("satellite"),
            confidence: get_field("confidence"),
            frp: get_field("frp").parse().unwrap_or(0.0),
            daynight: get_field("daynight"),
        });
    }

    Ok(SatelliteFirmsOutput {
        query: format!("{},{},{},{}", west, south, east, north),
        source: source.to_string(),
        total: hotspots.len(),
        hotspots,
    })
}
