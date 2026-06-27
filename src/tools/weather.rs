use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::types::*;

/// Get weather forecast for a location using OpenWeatherMap 5-day/3-hour forecast API.
pub async fn weather_forecast(
    input: WeatherForecastInput,
) -> Result<WeatherForecastOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let api_key = settings
        .openweather
        .as_ref()
        .and_then(|ow| ow.api_key.as_deref())
        .ok_or_else(|| {
            "OpenWeatherMap API key not configured. Set openweather.apiKey in settings.yml."
                .to_string()
        })?;

    let location = input.location.clone();
    let days = input.days.unwrap_or(3).clamp(1, 5);
    let limit = days * 8; // 3-hour intervals, 8 per day

    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let url = format!(
        "https://api.openweathermap.org/data/2.5/forecast?q={}&appid={}&units=metric&cnt={}",
        urlencoding(&location),
        api_key,
        limit,
    );

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("OpenWeatherMap API error: {}", e))?;

    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("OpenWeatherMap returned cached response".into()),
    };

    let json: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("Failed to parse OpenWeatherMap response: {}", e))?;

    if json["cod"].as_str().unwrap_or("") != "200" && json["cod"].as_i64().unwrap_or(0) != 200 {
        return Err(format!(
            "OpenWeatherMap error: {}",
            json["message"].as_str().unwrap_or("unknown error")
        ));
    }

    let city_name = json["city"]["name"]
        .as_str()
        .unwrap_or(&location)
        .to_string();
    let country = json["city"]["country"].as_str().unwrap_or("").to_string();

    let mut forecasts = Vec::new();
    if let Some(list) = json["list"].as_array() {
        for entry in list {
            let dt_txt = entry["dt_txt"].as_str().unwrap_or("");
            let main = &entry["main"];
            let weather = entry["weather"]
                .as_array()
                .and_then(|w| w.first())
                .map(|w| {
                    (
                        w["main"].as_str().unwrap_or("Unknown").to_string(),
                        w["description"].as_str().unwrap_or("").to_string(),
                    )
                })
                .unwrap_or_default();
            let wind = &entry["wind"];

            forecasts.push(WeatherDay {
                date: dt_txt.to_string(),
                temp_high: main["temp_max"].as_f64().unwrap_or(0.0),
                temp_low: main["temp_min"].as_f64().unwrap_or(0.0),
                condition: weather.0,
                description: weather.1,
                humidity: main["humidity"].as_u64().unwrap_or(0) as u32,
                wind_speed: wind["speed"].as_f64().unwrap_or(0.0),
                precipitation_pct: entry["pop"].as_f64().unwrap_or(0.0).mul_add(100.0, 0.0) as u32,
            });
        }
    }

    Ok(WeatherForecastOutput {
        location: city_name,
        country,
        forecasts,
    })
}

/// Get current weather for a location using OpenWeatherMap current weather API.
pub async fn weather_current(input: WeatherCurrentInput) -> Result<WeatherCurrentOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let api_key = settings
        .openweather
        .as_ref()
        .and_then(|ow| ow.api_key.as_deref())
        .ok_or_else(|| {
            "OpenWeatherMap API key not configured. Set openweather.apiKey in settings.yml."
                .to_string()
        })?;

    let location = input.location.clone();

    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let url = format!(
        "https://api.openweathermap.org/data/2.5/weather?q={}&appid={}&units=metric",
        urlencoding(&location),
        api_key,
    );

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("OpenWeatherMap API error: {}", e))?;

    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("OpenWeatherMap returned cached response".into()),
    };

    let json: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("Failed to parse OpenWeatherMap response: {}", e))?;

    if json["cod"].as_i64().unwrap_or(0) != 200 {
        return Err(format!(
            "OpenWeatherMap error: {}",
            json["message"].as_str().unwrap_or("unknown error")
        ));
    }

    let city_name = json["name"].as_str().unwrap_or(&location).to_string();
    let country = json["sys"]["country"].as_str().unwrap_or("").to_string();
    let main = &json["main"];
    let weather = json["weather"]
        .as_array()
        .and_then(|w| w.first())
        .map(|w| {
            (
                w["main"].as_str().unwrap_or("Unknown").to_string(),
                w["description"].as_str().unwrap_or("").to_string(),
            )
        })
        .unwrap_or_default();
    let wind = &json["wind"];

    Ok(WeatherCurrentOutput {
        location: city_name,
        country,
        temp: main["temp"].as_f64().unwrap_or(0.0),
        feels_like: main["feels_like"].as_f64().unwrap_or(0.0),
        condition: weather.0,
        description: weather.1,
        humidity: main["humidity"].as_u64().unwrap_or(0) as u32,
        wind_speed: wind["speed"].as_f64().unwrap_or(0.0),
        visibility: json["visibility"].as_u64().unwrap_or(0) as u32,
    })
}

/// Get weather alerts for a lat/lon location using OpenWeatherMap One Call API.
pub async fn weather_alerts(input: WeatherAlertsInput) -> Result<WeatherAlertsOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let api_key = settings
        .openweather
        .as_ref()
        .and_then(|ow| ow.api_key.as_deref())
        .ok_or_else(|| {
            "OpenWeatherMap API key not configured. Set openweather.apiKey in settings.yml."
                .to_string()
        })?;

    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let url = format!(
        "https://api.openweathermap.org/data/2.5/onecall?lat={}&lon={}&appid={}&exclude=minutely,hourly,daily",
        input.latitude,
        input.longitude,
        api_key,
    );

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("OpenWeatherMap API error: {}", e))?;

    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("OpenWeatherMap returned cached response".into()),
    };

    let json: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("Failed to parse OpenWeatherMap response: {}", e))?;

    if json["cod"].as_i64().is_some() && json["cod"].as_i64().unwrap_or(200) != 200 {
        return Err(format!(
            "OpenWeatherMap error: {}",
            json["message"].as_str().unwrap_or("unknown error")
        ));
    }

    let location_label = format!("{}, {}", input.latitude, input.longitude);

    let mut alerts = Vec::new();
    if let Some(alerts_arr) = json["alerts"].as_array() {
        for alert in alerts_arr {
            alerts.push(WeatherAlert {
                sender: alert["sender_name"].as_str().unwrap_or("").to_string(),
                event: alert["event"].as_str().unwrap_or("").to_string(),
                start: alert["start"]
                    .as_i64()
                    .map(|ts| {
                        chrono::DateTime::from_timestamp(ts, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default(),
                end: alert["end"]
                    .as_i64()
                    .map(|ts| {
                        chrono::DateTime::from_timestamp(ts, 0)
                            .map(|dt| dt.to_rfc3339())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default(),
                description: alert["description"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    Ok(WeatherAlertsOutput {
        location: location_label,
        alerts,
    })
}

fn urlencoding(s: &str) -> String {
    s.replace(' ', "%20").replace(',', "%2C")
}
