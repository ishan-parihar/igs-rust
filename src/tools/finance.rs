use super::types::*;
use crate::config;
use crate::http::{self as http_mod, HttpClient};

pub async fn finance_market(input: FinanceMarketInput) -> Result<FinanceMarketOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    let mut quotes = Vec::new();

    for symbol in &input.symbols {
        let url = format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d",
            symbol
        );
        let outcome = http
            .fetch(&url, None, "bypass")
            .await
            .map_err(|e| format!("Yahoo Finance error: {}", e))?;

        if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
            let data: serde_json::Value = serde_json::from_str(&resp.body_text)
                .map_err(|e| format!("JSON parse error: {}", e))?;

            if let Some(result) = data["chart"]["result"].as_array().and_then(|r| r.first()) {
                let meta = &result["meta"];
                let price = meta["regularMarketPrice"].as_f64().unwrap_or(0.0);
                let prev_close = meta["previousClose"].as_f64().unwrap_or(price);
                let change = price - prev_close;
                let change_pct = if prev_close > 0.0 {
                    (change / prev_close) * 100.0
                } else {
                    0.0
                };
                let name = meta["shortName"].as_str().unwrap_or(symbol).to_string();
                let volume = meta["regularMarketVolume"].as_u64().unwrap_or(0);

                quotes.push(MarketQuote {
                    symbol: symbol.clone(),
                    name,
                    price,
                    change,
                    change_pct,
                    volume,
                    market_cap: None,
                });
            }
        }
    }

    Ok(FinanceMarketOutput { quotes })
}

pub async fn finance_crypto(input: FinanceCryptoInput) -> Result<FinanceCryptoOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let ids = if input.ids.is_empty() {
        input.symbols.clone()
    } else {
        input.ids
    };
    let ids_str = ids.join(",");
    let url = format!("https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_vol=true&include_24hr_change=true&include_market_cap=true", ids_str);

    let outcome = http
        .fetch(&url, None, "bypass")
        .await
        .map_err(|e| format!("CoinGecko API error: {}", e))?;

    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("CoinGecko returned cached response".into()),
    };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: {}", e))?;

    let mut prices = Vec::new();
    for id in &input.symbols {
        if let Some(coin) = data.get(id) {
            prices.push(CryptoPrice {
                id: id.clone(),
                symbol: id.clone(),
                name: id.clone(),
                price_usd: coin["usd"].as_f64().unwrap_or(0.0),
                change_24h_pct: coin["usd_24h_change"].as_f64().unwrap_or(0.0),
                market_cap: coin["usd_market_cap"].as_u64().unwrap_or(0),
                volume_24h: coin["usd_24h_vol"].as_f64().unwrap_or(0.0) as u64,
            });
        }
    }

    Ok(FinanceCryptoOutput { prices })
}

pub async fn finance_trending(
    _input: FinanceTrendingInput,
) -> Result<FinanceTrendingOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let url = "https://api.coingecko.com/api/v3/search/trending";
    let outcome = http
        .fetch(url, None, "bypass")
        .await
        .map_err(|e| format!("CoinGecko API error: {}", e))?;

    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("CoinGecko returned cached response".into()),
    };

    let data: serde_json::Value =
        serde_json::from_str(&resp.body_text).map_err(|e| format!("JSON parse error: {}", e))?;

    let mut trending = Vec::new();
    if let Some(coins) = data["coins"].as_array() {
        for coin in coins {
            let item = &coin["item"];
            trending.push(TrendingCoin {
                name: item["name"].as_str().unwrap_or("").to_string(),
                symbol: item["symbol"].as_str().unwrap_or("").to_string(),
                market_cap_rank: item["market_cap_rank"].as_u64().unwrap_or(0) as u32,
                score: item["score"].as_f64().unwrap_or(0.0),
            });
        }
    }

    Ok(FinanceTrendingOutput { trending })
}
