use crate::config;
use crate::http::HttpClient;
use crate::lightpanda::LightpandaManager;
use crate::lightpanda_mcp::LightpandaMcpClient;
use crate::persistence;
use crate::tools::{helpers::toon_encode, insights, lp_mcp, news, parsers as parsers_tools, pools, reddit, research, security, sources, tool_guide, types::*, web, weather};
#[allow(unused_imports)]
use crate::types::*;
use rmcp::{
    Json,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

// ─── Internal Server State ──────────────────────────────────────

#[allow(dead_code)]
pub struct InsightStorage {
    articles: Vec<ArticleInsight>,
    db: Option<rusqlite::Connection>,
}

impl Default for InsightStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl InsightStorage {
    pub fn new() -> Self {
        // Try to open SQLite database for persistence
        let db_path = persistence::default_db_path();
        let db = match persistence::open_db(&db_path) {
            Ok(conn) => {
                // Load existing articles
                match persistence::load_articles(&conn) {
                    Ok(articles) => {
                        tracing::info!("Loaded {} articles from {}", articles.len(), db_path.display());
                        return Self { articles, db: Some(conn) };
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load articles: {}", e);
                        Some(conn)
                    }
                }
            }
            Err(e) => {
                tracing::warn!("SQLite persistence unavailable: {}", e);
                None
            }
        };
        Self { articles: vec![], db }
    }

    pub fn add_article(&mut self, article: ArticleInsight) {
        // Save to SQLite if available
        if let Some(ref conn) = self.db {
            if let Err(e) = persistence::save_article(conn, &article) {
                tracing::warn!("Failed to persist article {}: {}", article.id, e);
            }
        }
        self.articles.push(article);
    }

    pub fn clear(&mut self) {
        // Clear SQLite if available
        if let Some(ref conn) = self.db {
            if let Err(e) = persistence::clear_articles(conn) {
                tracing::warn!("Failed to clear persisted articles: {}", e);
            }
        }
        self.articles.clear();
    }

    pub fn stats(&self) -> InsightStats {
        let total_articles = self.articles.len();
        let mut entities = std::collections::HashSet::new();
        let mut domains = std::collections::HashSet::new();
        for a in &self.articles {
            for e in &a.entities {
                entities.insert(e.name.clone());
            }
            for d in &a.domains {
                domains.insert(d.domain.clone());
            }
        }
        InsightStats {
            total_articles,
            total_entities: entities.len(),
            total_domains: domains.len(),
            avg_entities_per_article: if total_articles > 0 {
                self.articles.iter().map(|a| a.entities.len() as f64).sum::<f64>() / total_articles as f64
            } else {
                0.0
            },
            avg_domains_per_article: if total_articles > 0 {
                self.articles.iter().map(|a| a.domains.len() as f64).sum::<f64>() / total_articles as f64
            } else {
                0.0
            },
        }
    }

    pub fn find_inter_domain_connections(&self, entity: &str, min_domains: usize) -> Vec<EntityConnection> {
        let mut domain_map: std::collections::HashMap<String, DomainConnection> = std::collections::HashMap::new();
        for article in &self.articles {
            let matches_entity = article.entities.iter().any(|e| {
                e.name.to_lowercase() == entity.to_lowercase()
                    || e.normalized_id.as_ref().is_some_and(|id| id.to_lowercase() == entity.to_lowercase())
            });
            if !matches_entity { continue; }

            for d in &article.domains {
                let entry = domain_map.entry(d.domain.clone()).or_insert_with(|| DomainConnection {
                    domain: d.domain.clone(),
                    article_ids: vec![],
                    article_titles: vec![],
                });
                entry.article_ids.push(article.id.clone());
                entry.article_titles.push(article.title.clone());
            }
        }

        let domains_vec: Vec<DomainConnection> = domain_map.into_values().collect();
        let ndomains = domains_vec.len();
        if ndomains >= min_domains {
            let entity_type = self.articles.iter()
                .flat_map(|a| a.entities.iter())
                .find(|e| e.name.to_lowercase() == entity.to_lowercase())
                .map(|e| e.entity_type.clone())
                .unwrap_or_default();
            vec![EntityConnection {
                entity: entity.to_string(),
                entity_type,
                domains: domains_vec,
                connection_strength: ndomains as f64,
            }]
        } else {
            vec![]
        }
    }

    pub fn find_all_inter_domain_connections(&self, min_domains: usize) -> Vec<EntityConnection> {
        let mut entity_domains: std::collections::HashMap<String, (String, std::collections::HashMap<String, DomainConnection>)> = std::collections::HashMap::new();
        for article in &self.articles {
            for e in &article.entities {
                let key = e.name.to_lowercase();
                let (etype, domain_map) = entity_domains.entry(key).or_insert_with(|| {
                    (e.entity_type.clone(), std::collections::HashMap::new())
                });
                if etype.is_empty() {
                    *etype = e.entity_type.clone();
                }
                for d in &article.domains {
                    let entry = domain_map.entry(d.domain.clone()).or_insert_with(|| DomainConnection {
                        domain: d.domain.clone(),
                        article_ids: vec![],
                        article_titles: vec![],
                    });
                    entry.article_ids.push(article.id.clone());
                    entry.article_titles.push(article.title.clone());
                }
            }
        }

        entity_domains
            .into_iter()
            .filter_map(|(key, (etype, dm))| {
                let nd = dm.len();
                if nd < min_domains { return None; }
                let d2: Vec<DomainConnection> = dm.into_values().collect();
                Some(EntityConnection {
                    entity: key,
                    entity_type: etype,
                    domains: d2,
                    connection_strength: nd as f64,
                })
            })
            .collect()
    }

    pub fn detect_trending(&self, time_window_ms: i64, min_growth: f64, min_current: u32) -> Vec<TrendingEntity> {
        let now = chrono::Utc::now().timestamp_millis();
        let cutoff = now - time_window_ms;
        let half_cutoff = now - (time_window_ms * 2);

        let mut current: std::collections::HashMap<String, (u32, String)> = std::collections::HashMap::new();
        let mut previous: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

        for article in &self.articles {
            let t = chrono::DateTime::parse_from_rfc3339(&article.pub_date)
                .ok()
                .map(|dt| dt.timestamp_millis())
                .unwrap_or(0);

            for e in &article.entities {
                let name = e.name.to_lowercase();
                if t >= cutoff {
                    let (count, _etype) = current.entry(name).or_insert((0, e.entity_type.clone()));
                    *count += 1;
                } else if t >= half_cutoff {
                    *previous.entry(name).or_insert(0) += 1;
                }
            }
        }

        current
            .into_iter()
            .filter_map(|(name, (current_count, etype))| {
                if current_count < min_current { return None; }
                let prev_count = previous.get(&name).copied().unwrap_or(0);
                let growth = if prev_count > 0 {
                    current_count as f64 / prev_count as f64
                } else {
                    current_count as f64
                };
                if growth < min_growth { return None; }
                Some(TrendingEntity {
                    entity: name,
                    entity_type: etype,
                    current_mentions: current_count,
                    previous_mentions: prev_count,
                    growth,
                    normalized_growth: (growth / (1.0 + growth)).min(1.0),
                })
            })
            .collect()
    }
}

// ─── Format Resolution Trait ─────────────────────────────────────

/// Trait for input types that carry output format options.
pub trait HasFormat {
    /// Return a reference to the optional format string.
    fn format(&self) -> &Option<String>;
}

macro_rules! impl_has_format {
    ($($ty:ty),* $(,)?) => {
        $(
            impl HasFormat for $ty {
                fn format(&self) -> &Option<String> { &self.output.format }
            }
        )*
    };
}

impl_has_format!(
    SourceListInput, GeoListInput,
    NewsFetchInput, NewsTestInput, NewsEnrichInput,
    RedditSearchInput, RedditFeedInput,
    ResearchSearchInput, ResearchDownloadInput,
    WebSearchInput, WebScrapeInput, WebCrawlInput, WebMapInput,
    InsightFindConnectionsInput, InsightTrendingInput,
    WeatherForecastInput, WeatherCurrentInput, WeatherAlertsInput,
    CveSearchInput, SecurityAdvisoriesInput,
);

// ─── Sync Settings Loader ───────────────────────────────────────

/// Load settings synchronously (for use in non-async constructors).
/// Replicates config::load_settings() using std::fs.
fn load_settings_sync() -> Result<Settings, String> {
    let user_dir = config::user_config_dir();
    let _ = std::fs::create_dir_all(&user_dir);

    let file = user_dir.join("settings.yml");
    let raw = std::fs::read_to_string(&file)
        .map_err(|e| format!("Failed to read {}: {}", file.display(), e))?;

    // Expand env vars (same logic as config::expand_env_vars)
    let mut expanded = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let mut var_name = String::new();
            for ch in chars.by_ref() {
                if ch == '}' { break; }
                var_name.push(ch);
            }
            match std::env::var(&var_name) {
                Ok(val) => expanded.push_str(&val),
                Err(_) => {
                    expanded.push_str("${");
                    expanded.push_str(&var_name);
                    expanded.push('}');
                }
            }
        } else {
            expanded.push(c);
        }
    }

    serde_yaml::from_str(&expanded)
        .map_err(|e| format!("Failed to parse {}: {}", file.display(), e))
}

// ─── Format Output Helper ───────────────────────────────────────

/// Serialize a value to the requested format (TOON or JSON) and wrap in CallToolResult.
fn format_output<T: Serialize>(value: &T, format: &str) -> CallToolResult {
    let text = if format == "json" {
        serde_json::to_string_pretty(value).unwrap_or_default()
    } else {
        toon_encode(value)
    };
    CallToolResult::success(vec![Content::text(text)])
}

// ─── Server State ────────────────────────────────────────────────

#[derive(Clone)]
pub struct IgsMcpServer {
    tool_router: ToolRouter<IgsMcpServer>,
    insights: Arc<Mutex<InsightStorage>>,
    lightpanda_mcp: Arc<Mutex<Option<LightpandaMcpClient>>>,
    /// Tool groups for progressive discovery. Empty = all groups available.
    tool_groups: Vec<String>,
    http_client: Arc<HttpClient>,
    settings: Arc<Settings>,
}

// ─── Tool Router ────────────────────────────────────────────────

impl Default for IgsMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl IgsMcpServer {
    pub fn resolve_format(params: &impl HasFormat) -> String {
        params.format().as_deref().unwrap_or("toon").to_string()
    }

    pub fn filtered_tool_names(&self, all_tools: Vec<String>) -> Vec<String> {
        if self.tool_groups.is_empty() {
            return all_tools;
        }
        let mut result = Vec::new();
        for group_name in &self.tool_groups {
            if let Some(group_tools) = crate::tools::registry::get_group_tools(group_name) {
                for tool in &all_tools {
                    if group_tools.contains(&tool.as_str()) && !result.contains(tool) {
                        result.push(tool.clone());
                    }
                }
            }
        }
        result
    }
}

#[tool_router(router = tool_router)]
impl IgsMcpServer {
    pub fn new() -> Self {
        let settings = load_settings_sync().expect("Failed to load settings");
        let cache_dir = crate::http::resolve_cache_dir(&settings, &config::user_config_dir());
        let http_client = HttpClient::new(&settings.http, &cache_dir);
        Self {
            tool_router: Self::tool_router(),
            insights: Arc::new(Mutex::new(InsightStorage::new())),
            lightpanda_mcp: Arc::new(Mutex::new(None)),
            tool_groups: Vec::new(),
            http_client: Arc::new(http_client),
            settings: Arc::new(settings),
        }
    }

    pub fn new_with_groups(tool_groups: Vec<String>) -> Self {
        let settings = load_settings_sync().expect("Failed to load settings");
        let cache_dir = crate::http::resolve_cache_dir(&settings, &config::user_config_dir());
        let http_client = HttpClient::new(&settings.http, &cache_dir);
        Self {
            tool_router: Self::tool_router(),
            insights: Arc::new(Mutex::new(InsightStorage::new())),
            lightpanda_mcp: Arc::new(Mutex::new(None)),
            tool_groups,
            http_client: Arc::new(http_client),
            settings: Arc::new(settings),
        }
    }

    // ── Tool Guide ─────────────────────────────────────────────

    #[tool(name = "tool.guide", description = "Categorized tool index with decision tree. Call this FIRST to find the right tool for your task. Returns decision_tree (maps questions to tools), categories (tools grouped by domain), and drill_down_chains (multi-step investigation sequences).")]
    async fn tool_guide(&self) -> Result<Json<ToolGuideOutput>, String> {
        tool_guide::get_tool_guide().await.map(Json::<ToolGuideOutput>)
    }

    // ── Pool Tools ──────────────────────────────────────────────

    #[tool(name = "pools.list", description = "List all configured source pools. Pools group related news sources (e.g. GLOBAL_TECH_CYBER, INDIA_NATIONAL_BASE). Use pool IDs as filters in news.fetch. Returns Pool[] with id, name, description, is_active. Do NOT use to fetch news content — use news.fetch instead.")]
    async fn pools_list(&self) -> Result<Json<PoolListOutput>, String> {
        let result: PoolListOutput = pools::pools_list().await?;
        Ok(Json(result))
    }

    #[tool(name = "pools.upsert", description = "Create or update a source pool. Pools group related news sources for batch fetching. Input: id (unique identifier like GLOBAL_TECH_CYBER), name (display name), description (what the pool covers), is_active (default true). Use pools.list to see existing pools. Do NOT use to fetch news — use news.fetch.")]
    async fn pools_upsert(&self, params: Parameters<PoolUpsertInput>) -> Result<Json<PoolUpsertOutput>, String> {
        let result: PoolUpsertOutput = pools::pools_upsert(params.0).await?;
        Ok(Json(result))
    }

    #[tool(name = "pools.delete", description = "Permanently delete a pool by ID. Does not delete sources in the pool — only removes the grouping. Use pools.list to find the pool ID first. Do NOT use to modify sources — use sources.delete.")]
    async fn pools_delete(&self, params: Parameters<PoolDeleteInput>) -> Result<Json<PoolDeleteOutput>, String> {
        let result: PoolDeleteOutput = pools::pools_delete(params.0).await?;
        Ok(Json(result))
    }

    // ── Source Tools ────────────────────────────────────────────

    #[tool(name = "sources.list", description = "List configured news sources (410+ across 47 countries). Filter by pools (pool IDs) or active_only=true. Returns Source[] with id, name, type, url, parser, pools, countries, cities, domains. Default output: TOON. Do NOT use to fetch news — use news.fetch.")]
    async fn sources_list(&self, params: Parameters<SourceListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let cursor = params.0.pagination.cursor.clone();
        let page_size = params.0.pagination.page_size.unwrap_or(50);
        let all_output = sources::sources_list(params.0).await?;
        let (page, next_cursor) = paginate(&all_output.sources, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.sources.len() };
        Ok(format_output(&output, &format))
    }

    #[tool(name = "sources.upsert", description = "Create or update a news source. Required: name, type (rss/generic_html/ofac/who_dons/newslaundry), url. Optional: id (auto-generated from name), headers (custom HTTP headers), parser (key from parsers.list), pools (pool IDs), countries (ISO codes), cities, domains, is_active. Use sources.autodiscover to auto-detect feeds first. Do NOT use to fetch news — use news.fetch.")]
    async fn sources_upsert(&self, params: Parameters<SourceUpsertInput>) -> Result<Json<SourceUpsertOutput>, String> {
        sources::sources_upsert(params.0).await.map(Json)
    }

    #[tool(name = "sources.delete", description = "Permanently delete a source by ID from sources.yml. Cannot be undone. Use sources.list to find the source ID first. Do NOT use to modify pools — use pools.delete.")]
    async fn sources_delete(&self, params: Parameters<SourceDeleteInput>) -> Result<Json<SourceDeleteOutput>, String> {
        sources::sources_delete(params.0).await.map(Json)
    }

    #[tool(name = "sources.autodiscover", description = "Auto-discover RSS/Atom feeds or sitemap from a homepage URL. Fetches the URL, looks for <link rel='alternate'> RSS/Atom tags, falls back to /sitemap.xml. Returns kind (rss/sitemap/none), url, sample items. Optionally adds discovered source to sources.yml with pools and name. Do NOT use to search the web — use web.search.")]
    async fn sources_autodiscover(&self, params: Parameters<AutodiscoverInput>) -> Result<Json<AutodiscoverOutput>, String> {
        sources::sources_autodiscover(params.0).await.map(Json)
    }

    #[tool(name = "sources.enableGenericScraper", description = "Enable generic HTML scraping for a source. Sets parser to generic_html with CSS selectors. Input: source id, optional list_url (page to scrape), selectors (item, title, link, date, desc CSS selectors). Use sources.autodiscover first to find the source, then enable scraping for non-RSS sources. Do NOT use for RSS feeds — RSS sources work automatically.")]
    async fn sources_enable_scraper(&self, params: Parameters<EnableScraperInput>) -> Result<Json<EnableScraperOutput>, String> {
        sources::sources_enable_scraper(params.0).await.map(Json)
    }

    #[tool(name = "sources.countries", description = "List countries with source counts. Returns CountryInfo[] with name, ISO code, and source_count. Use ISO codes (IN, US, GB, etc.) as filters in news.fetch countries parameter. Default output: TOON. Do NOT use for city-level data — use sources.cities.")]
    async fn sources_countries(&self, params: Parameters<GeoListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let cursor = params.0.pagination.cursor.clone();
        let page_size = params.0.pagination.page_size.unwrap_or(50);
        let all_output = sources::sources_countries().await?;
        let (page, next_cursor) = paginate(&all_output.countries, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.countries.len() };
        Ok(format_output(&output, &format))
    }

    #[tool(name = "sources.cities", description = "List cities with source counts. Returns CityInfo[] with name and source_count. Use city names as filters in news.fetch cities parameter. Sorted by source count descending. Default output: TOON. Do NOT use for country-level data — use sources.countries.")]
    async fn sources_cities(&self, params: Parameters<GeoListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let cursor = params.0.pagination.cursor.clone();
        let page_size = params.0.pagination.page_size.unwrap_or(50);
        let all_output = sources::sources_cities().await?;
        let (page, next_cursor) = paginate(&all_output.cities, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.cities.len() };
        Ok(format_output(&output, &format))
    }

    #[tool(name = "sources.domains", description = "List domains with source counts. Returns DomainInfoCount[] with name and source_count. Domains are topical tags (geopolitics, business, tech, cyber, defense, health, etc.). Use domain names as filters in news.fetch domains parameter. Default output: TOON. Do NOT use to search — use web.search or news.fetch.")]
    async fn sources_domains(&self, params: Parameters<GeoListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let cursor = params.0.pagination.cursor.clone();
        let page_size = params.0.pagination.page_size.unwrap_or(50);
        let all_output = sources::sources_domains().await?;
        let (page, next_cursor) = paginate(&all_output.domains, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.domains.len() };
        Ok(format_output(&output, &format))
    }

    // ── Parser Tools ────────────────────────────────────────────

    #[tool(name = "parsers.list", description = "List available source parser keys (rss, generic_html, semantic_scholar, etc.). Auto-detects if parser not specified in sources.upsert.")]
    async fn parsers_list(&self, params: Parameters<ParserListInput>) -> Result<CallToolResult, String> {
        let cursor = params.0.pagination.cursor.clone();
        let page_size = params.0.pagination.page_size.unwrap_or(50);
        let all_output = parsers_tools::parsers_list().await?;
        let (page, next_cursor) = paginate(&all_output.parsers, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.parsers.len() };
        Ok(format_output(&output, "toon"))
    }

    // ── News Tools ──────────────────────────────────────────────

    #[tool(name = "news.fetch", description = "Fetch news from sources. Filter by pools, countries, cities, domains, time range, and keywords. depth='deep' runs full pipeline (fetch→enrich→index). Default output: TOON, use format='json' for JSON.")]
    async fn news_fetch(&self, params: Parameters<NewsFetchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let depth = params.0.depth_opts.depth.clone().unwrap_or_else(|| "default".to_string());

        if depth == "deep" {
            let output = news::fetch_news_intelligent(params.0, &self.insights).await?;
            Ok(format_output(&output, &format))
        } else {
            let _subject = params.0.filters.pools.as_ref().and_then(|p| p.first()).cloned().unwrap_or_else(|| "news".to_string());
            let output = news::news_fetch(params.0).await?;
            #[cfg(not(test))]
            {
                crate::tools::dump::maybe_dump(
                    &self.settings,
                    "news.fetch",
                    &_subject,
                    &toon_encode(&output),
                );
            }
            Ok(format_output(&output, &format))
        }
    }

    #[tool(name = "news.testSource", description = "Test a single source and return up to 10 items. Input: source ID (from sources.list). Useful for debugging source configuration, parser issues, or verifying a new source works. Returns NewsItem[]. Do NOT use to fetch multiple articles — use news.fetch.")]
    async fn news_test_source(&self, params: Parameters<NewsTestInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.id.clone();
        let output = news::news_test_source(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "news.testSource",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "news.enrich", description = "Offline NLP enrichment for news items. Extracts topics, entities, sentiment, and summary. No external API calls. Use with insights.indexArticles for cross-article analysis.")]
    async fn news_enrich(&self, params: Parameters<NewsEnrichInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = format!("enrich-{}", params.0.items.len());
        let output = news::news_enrich(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "news.enrich",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    // ── Weather Tools ──────────────────────────────────────────

    #[tool(name = "weather.forecast", description = "Get weather forecast for a location. Returns daily forecasts with temp, condition, humidity, wind. Uses OpenWeatherMap API (free tier). Location: city name or lat,lon. Days: 1-5 (default 3). Default output: TOON.")]
    async fn weather_forecast(&self, params: Parameters<WeatherForecastInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = weather::weather_forecast(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "weather.current", description = "Get current weather for a location. Returns temp, feels_like, condition, humidity, wind, visibility. Uses OpenWeatherMap API. Location: city name or lat,lon. Default output: TOON.")]
    async fn weather_current(&self, params: Parameters<WeatherCurrentInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = weather::weather_current(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "weather.alerts", description = "Get weather alerts for a lat/lon location. Returns active severe weather warnings. Uses OpenWeatherMap One Call API. Input: latitude and longitude. Default output: TOON.")]
    async fn weather_alerts(&self, params: Parameters<WeatherAlertsInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = weather::weather_alerts(params.0).await?;
        Ok(format_output(&output, &format))
    }

    // ── Reddit Tools ────────────────────────────────────────────

    #[tool(name = "reddit.search", description = "Search Reddit posts via reddit.com JSON API. Supports subreddits filter (e.g. [\"worldnews\",\"technology\"]), sort (relevance/hot/top/new), time (hour/day/week/month/year/all). Returns NewsItem[] compatible with news.enrich and insights.indexArticles for cross-platform analysis. Do NOT use for general web search, news articles, or academic papers — use web.search, news.fetch, or research.* respectively.")]
    async fn reddit_search(&self, params: Parameters<RedditSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.subreddits.as_ref().and_then(|s| s.first()).cloned().unwrap_or_else(|| params.0.query.clone());
        let output = reddit::reddit_search(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "reddit.search",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "reddit.feed", description = "Fetch latest posts from subreddits via RSS feeds (old.reddit.com/r/{sub}/.rss). Reliable cross-platform access that works without API keys or residential IPs. Pass subreddit names without r/ prefix. Returns NewsItem[] compatible with news.enrich and insights.indexArticles. Do NOT use to search — use reddit.search for queries.")]
    async fn reddit_feed(&self, params: Parameters<RedditFeedInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.subreddits.first().cloned().unwrap_or_default();
        let output = reddit::reddit_feed(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "reddit.feed",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    // ── Research Tools ──────────────────────────────────────────

    #[tool(name = "research.search", description = "Search academic papers from arXiv and Semantic Scholar. Supports categories (e.g. cs.AI, cs.CL), year_from, year_to filtering. Returns ResearchPaper[] with id (format: arxiv:XXXX or semanticscholar:XXXX), title, authors, abstract, year, citation_count, pdf_url. Use research.paper for details or research.download for PDF. Do NOT use for general web search, news articles, or Reddit discussions — use web.search, news.fetch, or reddit.* respectively.")]
    async fn research_search(&self, params: Parameters<ResearchSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.query.clone();
        let output = research::research_search(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "research.search",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "research.paper", description = "Get detailed paper information by ID. ID format: arxiv:XXXX.XXXXX or semanticscholar:XXXX. Returns PaperDetail with title, authors, abstract, year, citations, references, pdf_url. Optionally include_citations, include_references, extract_pdf. Do NOT use to search — use research.search first.")]
    async fn research_paper(&self, params: Parameters<ResearchPaperInput>) -> Result<Json<ResearchPaperOutput>, String> {
        let _subject = params.0.paper_id.clone();
        let output = research::research_paper(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "research.paper",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(Json(output))
    }

    #[tool(name = "research.download", description = "Download a research paper PDF to disk. ID format: arxiv:XXXX.XXXXX or semanticscholar:XXXX. For Semantic Scholar, fetches PDF URL from API first. Optional output_path (default: {paper_id}.pdf) and format. Returns file path and size. Do NOT use to view abstracts — use research.paper for metadata.")]
    async fn research_download(&self, params: Parameters<ResearchDownloadInput>) -> Result<Json<ResearchDownloadOutput>, String> {
        research::research_download(params.0).await.map(Json)
    }

    // ── Security Tools ──────────────────────────────────────────

    #[tool(name = "security.cve", description = "Search CVE vulnerabilities from NVD (National Vulnerability Database). Returns CVE ID, severity, CVSS score, affected products, references. Use for threat intelligence and vulnerability monitoring. Supports days_back (default 30), severity filter, limit. Default output: TOON.")]
    async fn security_cve(&self, params: Parameters<CveSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.query.clone();
        let output = security::security_cve_search(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "security.cve",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "security.advisories", description = "Search GitHub Security Advisories by ecosystem (npm, pip, maven, go, rust). Returns advisory ID (GHSA), CVE ID, severity, vulnerable version range, patched versions. Use for dependency vulnerability monitoring. Default output: TOON.")]
    async fn security_advisories(&self, params: Parameters<SecurityAdvisoriesInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.ecosystem.clone();
        let output = security::security_advisories(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "security.advisories",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    // ── Web Tools ───────────────────────────────────────────────

    #[tool(name = "web.search", description = "Realtime web search via Tavily (default) or Firecrawl API. Requires tavily.enabled=true or firecrawl.enabled=true in settings.yml with API key. Supports include_domains, exclude_domains, days, include_answer. Returns results array with title, url, content, score. Default output: TOON. Use format='json' for structured JSON. Do NOT use for academic papers, news articles, or Reddit posts — use research.search, news.fetch, or reddit.* respectively.")]
    async fn web_search(&self, params: Parameters<WebSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.query.clone();
        let output = web::web_search(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "web.search",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "web.scrape", description = "Scrape a URL and return structured markdown with metadata (title, headings, og:description, link count). Provider 'default' uses HTTP+html-to-markdown. Provider 'lightpanda' renders JavaScript — set lightpanda.enabled=true in settings.yml first. Lightpanda supports wait_selector, strip_mode, wait_until, include_frames for JS-heavy sites. Default output: TOON. Do NOT use for multiple pages, search results, or news — use web.crawl, web.search, or news.fetch respectively.")]
    async fn web_scrape(&self, params: Parameters<WebScrapeInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = url::Url::parse(&params.0.url).map(|u| u.host_str().unwrap_or("unknown").to_string()).unwrap_or_else(|_| params.0.url.clone());
        let output = web::web_scrape(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "web.scrape",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "web.crawl", description = "BFS crawl a website using Lightpanda headless browser. Renders JavaScript. Requires lightpanda.enabled=true in settings.yml (binary auto-downloads). Supports max_depth (default 2), max_pages (default 20), obey_robots, dump_format (markdown/html/semantic_tree), wait_until, wait_selector, strip_mode, include_frames. Returns pages with depth and status. Default output: TOON. Do NOT use for single pages, search, or news — use web.scrape, web.search, or news.fetch respectively.")]
    async fn web_crawl(&self, params: Parameters<WebCrawlInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = url::Url::parse(&params.0.url).map(|u| u.host_str().unwrap_or("unknown").to_string()).unwrap_or_else(|_| params.0.url.clone());
        let output = web::web_crawl(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "web.crawl",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "web.map", description = "Discover URLs on a website by parsing sitemap.xml. Fetches /sitemap.xml, extracts <loc> URLs. Supports limit (default 100) and search filter. Returns WebMapOutput with links array containing url and optional title. Default output: TOON. Do NOT use to fetch content — use web.scrape or web.crawl.")]
    async fn web_map(&self, params: Parameters<WebMapInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = url::Url::parse(&params.0.url).map(|u| u.host_str().unwrap_or("unknown").to_string()).unwrap_or_else(|_| params.0.url.clone());
        let output = web::web_map(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "web.map",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    // ── Insight Tools ───────────────────────────────────────────

    #[tool(name = "insights.findConnections", description = "Find cross-domain entity connections in indexed articles. Pass entity to look up specific entity, or omit to discover all cross-domain entities. Requires articles indexed via insights.indexArticles or news.fetch with depth='deep'. Returns EntityConnection with domain breakdown and article IDs. Use min_domains to filter (default 2), limit for max results (default 20). Do NOT use for fetching news, web search, or paper research — use news.fetch, web.search, or research.* respectively.")]
    async fn insight_find_connections(&self, params: Parameters<InsightFindConnectionsInput>) -> Result<Json<InsightFindConnectionsOutput>, String> {
        insights::insights_find_connections(&self.insights, params.0).await.map(Json)
    }

    #[tool(name = "insights.trendingEntities", description = "Detect entities with increasing mention frequency in indexed articles. Compares current time window vs previous. Requires articles indexed via insights.indexArticles. Use time_window_hours (default 24), min_growth (default 2.0), min_current_mentions (default 3). Do NOT use to find connections — use insights.findConnections.")]
    async fn insights_trending(&self, params: Parameters<InsightTrendingInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = insights::insights_trending(&self.insights, params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "insights.indexArticles", description = "Index articles in the in-memory insight engine for cross-article entity analysis. Input: articles with id, title, pub_date, source_name, and optionally domains (Vec<DomainInfo>) and entities (Vec<EntityInfo>). Use news.fetch with depth='deep' to automate fetch→enrich→index pipeline. After indexing, use insights.findConnections or insights.trendingEntities. Do NOT use to search — use insights.findConnections or insights.trendingEntities.")]
    async fn insights_index(&self, params: Parameters<InsightIndexInput>) -> Result<Json<InsightIndexOutput>, String> {
        insights::insights_index(&self.insights, params.0).await.map(Json)
    }

    #[tool(name = "insights.getStats", description = "Get insight engine statistics. Returns total_articles, total_entities, total_domains, avg_entities_per_article, avg_domains_per_article. Use to check what's been indexed before running insights.findConnections or insights.trendingEntities. Do NOT use to find connections — use insights.findConnections.")]
    async fn insights_stats(&self) -> Result<Json<InsightStatsOutput>, String> {
        insights::insights_stats(&self.insights).await.map(Json)
    }

    #[tool(name = "insights.clearIndex", description = "Clear all indexed articles from the in-memory insight engine. Resets all entity connections, trending data, and statistics. Use insights.getStats first to see what will be lost. Do NOT use unless you need to reset the insight engine.")]
    async fn insights_clear(&self) -> Result<Json<InsightClearOutput>, String> {
        insights::insights_clear(&self.insights).await.map(Json)
    }

    // ── Lightpanda MCP Browser Automation Tools ──────────────────

    #[tool(name = "lightpanda.goto", description = "Navigate to a URL using Lightpanda headless browser. Renders JavaScript. Spawns persistent browser session on first call. Use wait_until to control when page is considered loaded. Do NOT use for simple HTTP fetching, API calls, or non-web content — use web.scrape for simple fetching.")]
    async fn lp_goto(&self, params: Parameters<LpGotoInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_goto(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.markdown", description = "Get the current page content as structured markdown. Supports strip_mode to remove js/css/ui elements. Do NOT use to navigate — call lightpanda.goto first.")]
    async fn lp_markdown(&self, params: Parameters<LpMarkdownInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_markdown(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.links", description = "Extract all links from the current page. Returns URLs and link text. Do NOT use to navigate — call lightpanda.goto first.")]
    async fn lp_links(&self, params: Parameters<LpLinksInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_links(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.evaluate", description = "Execute JavaScript in the current page context. Returns the result. Example: document.title, document.querySelectorAll('h1').length. Do NOT use for simple content extraction — use lightpanda.markdown.")]
    async fn lp_evaluate(&self, params: Parameters<LpEvaluateInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_evaluate(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.semantic_tree", description = "Get the semantic DOM tree of the current page. AI-friendly representation of page structure. Do NOT use for full page content — use lightpanda.markdown.")]
    async fn lp_semantic_tree(&self, params: Parameters<LpSemanticTreeInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_semantic_tree(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.structuredData", description = "Extract structured data from the current page: JSON-LD, OpenGraph metadata, microdata. Do NOT use for raw content — use lightpanda.markdown.")]
    async fn lp_structured_data(&self, params: Parameters<LpStructuredDataInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_structured_data(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.detectForms", description = "Detect forms on the current page. Returns form fields, actions, and methods. Do NOT use to fill forms — use lightpanda.fill.")]
    async fn lp_detect_forms(&self, params: Parameters<LpDetectFormsInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_detect_forms(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.click", description = "Click an element on the current page by CSS selector. Optionally wait for navigation. Do NOT use to fill forms — use lightpanda.fill.")]
    async fn lp_click(&self, params: Parameters<LpClickInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_click(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.fill", description = "Fill a form field on the current page. Use CSS selector to target the field. Do NOT use to click buttons — use lightpanda.click.")]
    async fn lp_fill(&self, params: Parameters<LpFillInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_fill(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.scroll", description = "Scroll the current page. Direction: up/down/left/right. Pixels: amount to scroll. Do NOT use for navigation — use lightpanda.goto.")]
    async fn lp_scroll(&self, params: Parameters<LpScrollInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_scroll(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.waitForSelector", description = "Wait for a CSS selector to appear on the page. Useful for SPAs that load content dynamically. Do NOT use for navigation — use lightpanda.goto.")]
    async fn lp_wait_for_selector(&self, params: Parameters<LpWaitForSelectorInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_wait_for_selector(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.interactiveElements", description = "Find interactive elements on the current page (buttons, links, inputs). Returns clickable/fillable elements. Do NOT use to interact — use lightpanda.click or lightpanda.fill.")]
    async fn lp_interactive_elements(&self, params: Parameters<LpInteractiveElementsInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&self.settings.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_interactive_elements(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }
}

// ─── MCP Server Handler ────────────────────────────────────────

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for IgsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("igs-rust-mcp", "0.2.0"))
            .with_instructions("Intelligence Gathering System MCP Server. Provides tools for RSS/HTTP source monitoring, news fetching, Reddit search, academic paper research, web search/scraping, and cross-article entity insight analysis.")
    }
}
