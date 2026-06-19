use crate::config;
use crate::http::{self as http_mod, HttpClient};
use super::types::*;

pub async fn supply_chain_trade_flows(input: SupplyChainTradeInput) -> Result<SupplyChainTradeOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let api_key = settings.comtrade.as_ref()
        .and_then(|ct| ct.api_key.as_deref())
        .ok_or_else(|| "UN Comtrade API key not configured. Set comtrade.apiKey in settings.yml.".to_string())?;
    
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    
    let reporter = input.reporter_code.unwrap_or(124); // Default: US
    let partner = input.partner_code.unwrap_or(0); // Default: World
    let period = input.period.as_deref().unwrap_or("2024");
    let cmd_code = input.cmd_code.as_deref().unwrap_or("TOTAL");
    let flow_code = input.flow_code.as_deref().unwrap_or("M"); // M=Import, X=Export
    let limit = input.limit.unwrap_or(20).clamp(1, 500);
    
    let url = format!(
        "https://comtradeapi.un.org/data/v1/get/C/A/HS?reporterCode={}&partnerCode={}&period={}&cmdCode={}&flowCode={}&maxrecords={}&subscription-key={}",
        reporter, partner, period, cmd_code, flow_code, limit, api_key
    );
    
    let outcome = http.fetch(&url, None, "bypass").await
        .map_err(|e| format!("UN Comtrade API error: {}", e))?;
    
    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("UN Comtrade returned cached response".into()),
    };
    
    let data: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    
    if let Some(err) = data["message"].as_str() {
        return Err(format!("UN Comtrade error: {}", err));
    }
    
    let mut flows = Vec::new();
    if let Some(results) = data["data"].as_array() {
        for r in results {
            flows.push(TradeFlow {
                reporter: r["reporterDesc"].as_str().unwrap_or("").to_string(),
                partner: r["partnerDesc"].as_str().unwrap_or("").to_string(),
                period: r["period"].as_str().unwrap_or("").to_string(),
                flow: r["flowDesc"].as_str().unwrap_or("").to_string(),
                commodity: r["cmdDesc"].as_str().unwrap_or("").to_string(),
                value_usd: r["primaryValue"].as_f64().unwrap_or(0.0),
                net_weight_kg: r["netWgt"].as_f64().unwrap_or(0.0),
            });
        }
    }
    
    let total = data["count"].as_u64().unwrap_or(flows.len() as u64) as usize;
    
    Ok(SupplyChainTradeOutput {
        query: format!("Reporter:{} Partner:{} Period:{}", reporter, partner, period),
        total,
        flows,
    })
}
