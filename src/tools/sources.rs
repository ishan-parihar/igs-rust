use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::find_feed_url;
use crate::tools::types::*;
use crate::types::*;
use std::collections::HashMap;

/// List sources with optional pool/active filters
pub async fn sources_list(params: SourceListInput) -> Result<SourceListOutput, String> {
    match config::load_sources().await {
        Ok(sf) => {
            let mut list = sf.sources;
            if let Some(ref pools) = params.pools {
                list.retain(|s| s.pools.iter().any(|p| pools.contains(p)));
            }
            if params.active_only.unwrap_or(false) {
                list.retain(|s| s.is_active.unwrap_or(true));
            }
            Ok(SourceListOutput { sources: list })
        }
        Err(e) => Err(format!("Failed to load sources: {}", e)),
    }
}

/// Create or update a source
pub async fn sources_upsert(input: SourceUpsertInput) -> Result<SourceUpsertOutput, String> {
    match config::load_sources().await {
        Ok(mut sf) => {
            let id = input.id.unwrap_or_else(|| {
                input.name.to_lowercase().replace(|c: char| !c.is_alphanumeric() && c != '_', "_")
            });
            let src = Source {
                id: id.clone(),
                name: input.name,
                source_type: input.source_type,
                url: input.url,
                headers: input.headers,
                parser: input.parser,
                parser_config: None,
                pools: input.pools.unwrap_or_default(),
                countries: input.countries.unwrap_or_default(),
                cities: input.cities.unwrap_or_default(),
                domains: input.domains.unwrap_or_default(),
                is_active: input.is_active,
                platform: None,
                tier: None,
                rate_limit: None,
                source_category: None,
            };
            if let Some(idx) = sf.sources.iter().position(|s| s.id == id) {
                sf.sources[idx] = src;
            } else {
                sf.sources.push(src);
            }
            config::save_sources(&sf).await.map_err(|e| format!("Save failed: {}", e))?;
            Ok(SourceUpsertOutput { id })
        }
        Err(e) => Err(format!("Failed to load sources: {}", e)),
    }
}

/// Delete a source by id
pub async fn sources_delete(input: SourceDeleteInput) -> Result<SourceDeleteOutput, String> {
    match config::load_sources().await {
        Ok(mut sf) => {
            let before = sf.sources.len();
            sf.sources.retain(|s| s.id != input.id);
            let removed = sf.sources.len() < before;
            config::save_sources(&sf).await.map_err(|e| format!("Save failed: {}", e))?;
            Ok(SourceDeleteOutput { removed })
        }
        Err(e) => Err(format!("Failed to load sources: {}", e)),
    }
}

/// Auto-discover feeds/selectors from a homepage URL
pub async fn sources_autodiscover(input: AutodiscoverInput) -> Result<AutodiscoverOutput, String> {
    match config::load_settings().await {
        Ok(settings) => {
            let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
            let http = HttpClient::new(&settings.http, &cache_dir);
            match http.fetch(&input.url, None, "bypass").await {
                Ok(outcome) => {
                    let body = match outcome {
                        http_mod::FetchOutcome::Cached(_) => return Err("Unexpected cache hit".into()),
                        http_mod::FetchOutcome::Response(resp, _, _) => resp.body_text,
                    };
                    let feed_url = find_feed_url(&body, &input.url);

                    if let Some(feed) = feed_url {
                        Ok(AutodiscoverOutput {
                            kind: "rss".into(),
                            url: Some(feed),
                            sample: vec![],
                        })
                    } else {
                        let sitemap_url = format!("{}/sitemap.xml", input.url.trim_end_matches('/'));
                        match http.fetch(&sitemap_url, None, "bypass").await {
                            Ok(_) => Ok(AutodiscoverOutput {
                                kind: "sitemap".into(),
                                url: Some(sitemap_url),
                                sample: vec![],
                            }),
                            Err(_) => Ok(AutodiscoverOutput {
                                kind: "none".into(),
                                url: None,
                                sample: vec![],
                            }),
                        }
                    }
                }
                Err(e) => Err(format!("Fetch failed: {}", e)),
            }
        }
        Err(e) => Err(format!("Settings load failed: {}", e)),
    }
}

/// Enable generic HTML scraping for a source
pub async fn sources_enable_scraper(input: EnableScraperInput) -> Result<EnableScraperOutput, String> {
    match config::load_sources().await {
        Ok(mut sf) => {
            if let Some(idx) = sf.sources.iter().position(|s| s.id == input.id) {
                let s = &mut sf.sources[idx];
                s.parser = Some("generic_html".into());
                s.parser_config = Some(SourceParserConfig {
                    list_url: input.list_url,
                    selectors: input.selectors.map(|sel_map| Selectors {
                        item: sel_map.get("item").cloned().unwrap_or_default(),
                        title: sel_map.get("title").cloned(),
                        link: sel_map.get("link").cloned(),
                        date: sel_map.get("date").cloned(),
                        desc: sel_map.get("desc").cloned(),
                    }),
                });
                config::save_sources(&sf).await.map_err(|e| format!("Save failed: {}", e))?;
                Ok(EnableScraperOutput { updated: true })
            } else {
                Err(format!("Source not found: {}", input.id))
            }
        }
        Err(e) => Err(format!("Failed to load sources: {}", e)),
    }
}

/// List countries with available source counts
pub async fn sources_countries() -> Result<CountriesOutput, String> {
    let countries = config::load_countries().await.unwrap_or(serde_json::json!({"countries": []}));
    let sources = config::load_sources().await.unwrap_or(SourcesFile { sources: vec![] });
    let out: Vec<CountryInfo> = countries["countries"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let name = c["name"].as_str()?.to_string();
                    let code = c["code"].as_str()?.to_string();
                    let count = sources
                        .sources
                        .iter()
                        .filter(|s| {
                            s.is_active.unwrap_or(true)
                                && s.countries.iter().any(|sc| sc.to_uppercase() == code.to_uppercase())
                        })
                        .count();
                    Some(CountryInfo { name, code, source_count: count })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(CountriesOutput { countries: out })
}

/// List cities with available source counts
pub async fn sources_cities() -> Result<CitiesOutput, String> {
    let sources = config::load_sources().await.unwrap_or(SourcesFile { sources: vec![] });
    let mut city_map: HashMap<String, usize> = HashMap::new();
    for s in &sources.sources {
        if s.is_active.unwrap_or(true) {
            for c in &s.cities {
                *city_map.entry(c.clone()).or_default() += 1;
            }
        }
    }
    let mut cities: Vec<CityInfo> = city_map
        .into_iter()
        .map(|(name, count)| CityInfo { name, source_count: count })
        .collect();
    cities.sort_by(|a, b| b.source_count.cmp(&a.source_count));
    Ok(CitiesOutput { cities })
}

/// List domains with available source counts
pub async fn sources_domains() -> Result<DomainsOutput, String> {
    let sources = config::load_sources().await.unwrap_or(SourcesFile { sources: vec![] });
    let mut domain_map: HashMap<String, usize> = HashMap::new();
    for s in &sources.sources {
        if s.is_active.unwrap_or(true) {
            for d in &s.domains {
                *domain_map.entry(d.clone()).or_default() += 1;
            }
        }
    }
    let mut domains: Vec<DomainInfoCount> = domain_map
        .into_iter()
        .map(|(name, count)| DomainInfoCount { name, source_count: count })
        .collect();
    domains.sort_by(|a, b| b.source_count.cmp(&a.source_count));
    Ok(DomainsOutput { domains })
}
