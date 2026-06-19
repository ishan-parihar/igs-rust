use super::types::*;
use anyhow::Result;

pub async fn finance_market(input: FinanceMarketInput) -> Result<FinanceMarketOutput, String> {
    let client = reqwest::Client::new();
    let mut quotes = Vec::new();
    
    for symbol in &input.symbols {
        let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d", symbol);
        let resp = client.get(&url)
            .header("User-Agent", "IGS-MCP/0.4")
            .send()
            .await
            .map_err(|e| format!("HTTP error: {}", e))?;
        
        if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await.map_err(|e| format!("JSON error: {}", e))?;
            if let Some(result) = data["chart"]["result"].as_array().and_then(|r| r.first()) {
                let meta = &result["meta"];
                let price = meta["regularMarketPrice"].as_f64().unwrap_or(0.0);
                let prev_close = meta["previousClose"].as_f64().unwrap_or(price);
                let change = price - prev_close;
                let change_pct = if prev_close > 0.0 { (change / prev_close) * 100.0 } else { 0.0 };
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
    let client = reqwest::Client::new();
    let ids = if input.ids.is_empty() { input.symbols.clone() } else { input.ids };
    let ids_str = ids.join(",");
    let url = format!("https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_vol=true&include_24hr_change=true&include_market_cap=true", ids_str);
    
    let resp = client.get(&url)
        .header("User-Agent", "IGS-MCP/0.4")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;
    
    if !resp.status().is_success() {
        return Err(format!("CoinGecko API returned {}", resp.status()));
    }
    
    let data: serde_json::Value = resp.json().await.map_err(|e| format!("JSON error: {}", e))?;
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

pub async fn finance_trending(_input: FinanceTrendingInput) -> Result<FinanceTrendingOutput, String> {
    let client = reqwest::Client::new();
    let url = "https://api.coingecko.com/api/v3/search/trending";
    
    let resp = client.get(url)
        .header("User-Agent", "IGS-MCP/0.4")
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;
    
    if !resp.status().is_success() {
        return Err(format!("CoinGecko API returned {}", resp.status()));
    }
    
    let data: serde_json::Value = resp.json().await.map_err(|e| format!("JSON error: {}", e))?;
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
