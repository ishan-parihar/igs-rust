use crate::config;
use crate::http::HttpClient;
use crate::obscura::ObscuraManager;
use crate::persistence;
use crate::tools::{helpers::toon_encode, climate, env, finance, govt, health, insights, legal, lp_mcp, news, parsers as parsers_tools, patents, pools, politics, reddit, research, satellite, security, sop, sources, tool_guide, twitter, types::*, web, weather, youtube};
#[allow(unused_imports)]
use crate::types::*;
use rmcp::{
    Json, RoleServer,
    ErrorData,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

// ─── Internal Server State ──────────────────────────────────────

#[allow(dead_code)]
pub struct InsightStorage {
    articles: Vec<ArticleInsight>,
    entity_index: std::collections::HashMap<String, Vec<usize>>,
    domain_index: std::collections::HashMap<String, Vec<usize>>,
    db: Option<rusqlite::Connection>,
}

impl Default for InsightStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl InsightStorage {
    fn rebuild_indices(articles: &[ArticleInsight]) -> (std::collections::HashMap<String, Vec<usize>>, std::collections::HashMap<String, Vec<usize>>) {
        let mut entity_index: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        let mut domain_index: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
        for (i, article) in articles.iter().enumerate() {
            for e in &article.entities {
                entity_index.entry(e.name.to_lowercase()).or_default().push(i);
            }
            for d in &article.domains {
                domain_index.entry(d.domain.clone()).or_default().push(i);
            }
        }
        (entity_index, domain_index)
    }

    pub fn new() -> Self {
        // Try to open SQLite database for persistence
        let db_path = persistence::default_db_path();
        let db = match persistence::open_db(&db_path) {
            Ok(conn) => {
                // Load existing articles
                match persistence::load_articles(&conn) {
                    Ok(articles) => {
                        tracing::info!("Loaded {} articles from {}", articles.len(), db_path.display());
                        let (entity_index, domain_index) = Self::rebuild_indices(&articles);
                        return Self { articles, entity_index, domain_index, db: Some(conn) };
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
        Self { articles: vec![], entity_index: std::collections::HashMap::new(), domain_index: std::collections::HashMap::new(), db }
    }

    pub fn add_article(&mut self, article: ArticleInsight) {
        // Save to SQLite if available
        if let Some(ref conn) = self.db {
            if let Err(e) = persistence::save_article(conn, &article) {
                tracing::warn!("Failed to persist article {}: {}", article.id, e);
            }
        }
        let idx = self.articles.len();
        for e in &article.entities {
            self.entity_index.entry(e.name.to_lowercase()).or_default().push(idx);
        }
        for d in &article.domains {
            self.domain_index.entry(d.domain.clone()).or_default().push(idx);
        }
        self.articles.push(article);
    }

    pub fn add_articles_batch(&mut self, articles: Vec<ArticleInsight>) {
        if let Some(ref conn) = self.db {
            let tx = match conn.unchecked_transaction() {
                Ok(tx) => tx,
                Err(e) => {
                    tracing::warn!("Failed to start transaction: {}", e);
                    for article in articles {
                        self.articles.push(article);
                    }
                    return;
                }
            };

            for article in &articles {
                if let Err(e) = persistence::save_article(&tx, article) {
                    tracing::warn!("Failed to persist article {}: {}", article.id, e);
                }
            }

            if let Err(e) = tx.commit() {
                tracing::warn!("Failed to commit transaction: {}", e);
            }
        }

        let base = self.articles.len();
        for (i, article) in articles.iter().enumerate() {
            let idx = base + i;
            for e in &article.entities {
                self.entity_index.entry(e.name.to_lowercase()).or_default().push(idx);
            }
            for d in &article.domains {
                self.domain_index.entry(d.domain.clone()).or_default().push(idx);
            }
        }
        self.articles.extend(articles);
    }

    pub fn clear(&mut self) {
        // Clear SQLite if available
        if let Some(ref conn) = self.db {
            if let Err(e) = persistence::clear_articles(conn) {
                tracing::warn!("Failed to clear persisted articles: {}", e);
            }
        }
        self.articles.clear();
        self.entity_index.clear();
        self.domain_index.clear();
    }

    pub fn stats(&self) -> InsightStats {
        let total_articles = self.articles.len();
        InsightStats {
            total_articles,
            total_entities: self.entity_index.len(),
            total_domains: self.domain_index.len(),
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
        let key = entity.to_lowercase();
        let mut domain_map: std::collections::HashMap<String, DomainConnection> = std::collections::HashMap::new();
        let mut entity_type = String::new();

        if let Some(indices) = self.entity_index.get(&key) {
            for &idx in indices {
                let article = &self.articles[idx];
                if entity_type.is_empty() {
                    entity_type = article.entities.iter()
                        .find(|e| e.name.to_lowercase() == key)
                        .map(|e| e.entity_type.clone())
                        .unwrap_or_default();
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

        for article in &self.articles {
            let matches_normalized = article.entities.iter().any(|e| {
                e.normalized_id.as_ref().is_some_and(|id| id.to_lowercase() == key)
                    && !e.name.to_lowercase().eq(&key)
            });
            if !matches_normalized { continue; }

            if entity_type.is_empty() {
                entity_type = article.entities.iter()
                    .find(|e| e.normalized_id.as_ref().is_some_and(|id| id.to_lowercase() == key))
                    .map(|e| e.entity_type.clone())
                    .unwrap_or_default();
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

        let domains_vec: Vec<DomainConnection> = domain_map.into_values().collect();
        let ndomains = domains_vec.len();
        if ndomains >= min_domains {
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
        let mut results: Vec<EntityConnection> = Vec::new();

        for (key, indices) in &self.entity_index {
            let mut domain_map: std::collections::HashMap<String, DomainConnection> = std::collections::HashMap::new();
            let mut etype = String::new();

            for &idx in indices {
                let article = &self.articles[idx];
                if etype.is_empty() {
                    etype = article.entities.iter()
                        .find(|e| e.name.to_lowercase() == key.as_str())
                        .map(|e| e.entity_type.clone())
                        .unwrap_or_default();
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

            let nd = domain_map.len();
            if nd >= min_domains {
                results.push(EntityConnection {
                    entity: key.clone(),
                    entity_type: etype,
                    domains: domain_map.into_values().collect(),
                    connection_strength: nd as f64,
                });
            }
        }

        results
    }

    pub fn detect_trending(&self, time_window_ms: i64, min_growth: f64, min_current: u32) -> Vec<TrendingEntity> {
        let now = chrono::Utc::now().timestamp_millis();
        let cutoff = now - time_window_ms;
        let half_cutoff = now - (time_window_ms * 2);

        let mut results: Vec<TrendingEntity> = Vec::new();

        for (name, indices) in &self.entity_index {
            let mut current_count: u32 = 0;
            let mut previous_count: u32 = 0;
            let mut etype = String::new();

            for &idx in indices {
                let article = &self.articles[idx];
                let t = chrono::DateTime::parse_from_rfc3339(&article.pub_date)
                    .ok()
                    .map(|dt| dt.timestamp_millis())
                    .unwrap_or(0);

                if etype.is_empty() {
                    etype = article.entities.iter()
                        .find(|e| e.name.to_lowercase() == name.as_str())
                        .map(|e| e.entity_type.clone())
                        .unwrap_or_default();
                }

                if t >= cutoff {
                    current_count += 1;
                } else if t >= half_cutoff {
                    previous_count += 1;
                }
            }

            if current_count < min_current { continue; }
            let growth = if previous_count > 0 {
                current_count as f64 / previous_count as f64
            } else {
                current_count as f64
            };
            if growth < min_growth { continue; }

            results.push(TrendingEntity {
                entity: name.clone(),
                entity_type: etype,
                current_mentions: current_count,
                previous_mentions: previous_count,
                growth,
                normalized_growth: (growth / (1.0 + growth)).min(1.0),
            });
        }

        results
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
    ResearchSearchInput, ResearchDownloadInput, ResearchPubMedInput,
    WebSearchInput, WebScrapeInput, WebCrawlInput, WebMapInput,
    InsightFindConnectionsInput, InsightTrendingInput,
    WeatherForecastInput, WeatherCurrentInput, WeatherAlertsInput,
    FinanceMarketInput, FinanceCryptoInput, FinanceTrendingInput,
    CveSearchInput, SecurityAdvisoriesInput,
    GovtBillsInput, GovtRegulationsInput,
    PatentSearchInput, PatentDetailsInput,
    SopListInput, SopExecuteInput,
    HealthCdcInput, HealthWhoInput,
    PoliticsFecInput, PoliticsFecCommitteesInput,
    SatelliteFirmsInput,
    ClimateNoaaInput, ClimateNoaaStationsInput,
    EnvEpaFacilitiesInput, EnvEpaEmissionsInput,
    LegalSearchInput, LegalCaseDetailsInput,
    TwitterSearchInput, TwitterReadInput,
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
    obscura: Arc<Mutex<Option<ObscuraManager>>>,
    /// Tool groups for progressive discovery. Empty = all groups available.
    tool_groups: Vec<String>,
    #[allow(dead_code)] // reserved for future tool use
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
            obscura: Arc::new(Mutex::new(None)),
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
            obscura: Arc::new(Mutex::new(None)),
            tool_groups,
            http_client: Arc::new(http_client),
            settings: Arc::new(settings),
        }
    }

    // ── Tool Guide ─────────────────────────────────────────────

    // tool.guide moved to MCP resource (igs://tool-guide) to save ~1.5K tokens

    // ── Pool Tools ──────────────────────────────────────────────

    #[tool(name = "pools.list", description = "List all configured source pools. Returns Pool[] with id, name, description, is_active.")]
    async fn pools_list(&self) -> Result<Json<PoolListOutput>, String> {
        let result: PoolListOutput = pools::pools_list().await?;
        Ok(Json(result))
    }

    #[tool(name = "pools.upsert", description = "Create or update a source pool. Input: id, name, description, is_active.")]
    async fn pools_upsert(&self, params: Parameters<PoolUpsertInput>) -> Result<Json<PoolUpsertOutput>, String> {
        let result: PoolUpsertOutput = pools::pools_upsert(params.0).await?;
        Ok(Json(result))
    }

    #[tool(name = "pools.delete", description = "Permanently delete a pool by ID.")]
    async fn pools_delete(&self, params: Parameters<PoolDeleteInput>) -> Result<Json<PoolDeleteOutput>, String> {
        let result: PoolDeleteOutput = pools::pools_delete(params.0).await?;
        Ok(Json(result))
    }

    // ── Source Tools ────────────────────────────────────────────

    #[tool(name = "sources.list", description = "List configured news sources. Filter by pools or active_only. Returns Source[].")]
    async fn sources_list(&self, params: Parameters<SourceListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let cursor = params.0.cursor.clone();
        let page_size = params.0.page_size.unwrap_or(50);
        let all_output = sources::sources_list(params.0).await?;
        let (page, next_cursor) = paginate(&all_output.sources, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.sources.len() };
        Ok(format_output(&output, &format))
    }

    #[tool(name = "sources.upsert", description = "Create or update a news source. Required: name, type, url. Optional: id, headers, parser, pools, countries, cities, domains, is_active.")]
    async fn sources_upsert(&self, params: Parameters<SourceUpsertInput>) -> Result<Json<SourceUpsertOutput>, String> {
        sources::sources_upsert(params.0).await.map(Json)
    }

    #[tool(name = "sources.delete", description = "Permanently delete a source by ID.")]
    async fn sources_delete(&self, params: Parameters<SourceDeleteInput>) -> Result<Json<SourceDeleteOutput>, String> {
        sources::sources_delete(params.0).await.map(Json)
    }

    #[tool(name = "sources.autodiscover", description = "Auto-discover RSS/Atom feeds from a homepage URL. Returns kind, url, sample items.")]
    async fn sources_autodiscover(&self, params: Parameters<AutodiscoverInput>) -> Result<Json<AutodiscoverOutput>, String> {
        sources::sources_autodiscover(params.0).await.map(Json)
    }

    #[tool(name = "sources.enable_generic_scraper", description = "Enable generic HTML scraping for a source with CSS selectors.")]
    async fn sources_enable_scraper(&self, params: Parameters<EnableScraperInput>) -> Result<Json<EnableScraperOutput>, String> {
        sources::sources_enable_scraper(params.0).await.map(Json)
    }

    #[tool(name = "sources.countries", description = "List countries with source counts. Returns CountryInfo[].")]
    async fn sources_countries(&self, params: Parameters<GeoListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let cursor = params.0.cursor.clone();
        let page_size = params.0.page_size.unwrap_or(50);
        let all_output = sources::sources_countries().await?;
        let (page, next_cursor) = paginate(&all_output.countries, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.countries.len() };
        Ok(format_output(&output, &format))
    }

    #[tool(name = "sources.cities", description = "List cities with source counts. Returns CityInfo[].")]
    async fn sources_cities(&self, params: Parameters<GeoListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let cursor = params.0.cursor.clone();
        let page_size = params.0.page_size.unwrap_or(50);
        let all_output = sources::sources_cities().await?;
        let (page, next_cursor) = paginate(&all_output.cities, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.cities.len() };
        Ok(format_output(&output, &format))
    }

    #[tool(name = "sources.domains", description = "List domains with source counts. Returns DomainInfoCount[].")]
    async fn sources_domains(&self, params: Parameters<GeoListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let cursor = params.0.cursor.clone();
        let page_size = params.0.page_size.unwrap_or(50);
        let all_output = sources::sources_domains().await?;
        let (page, next_cursor) = paginate(&all_output.domains, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.domains.len() };
        Ok(format_output(&output, &format))
    }

    // ── Parser Tools ────────────────────────────────────────────

    #[tool(name = "parsers.list", description = "List available source parser keys (rss, generic_html, semantic_scholar, etc.). Auto-detects if parser not specified in sources.upsert.")]
    async fn parsers_list(&self, params: Parameters<ParserListInput>) -> Result<CallToolResult, String> {
        let cursor = params.0.cursor.clone();
        let page_size = params.0.page_size.unwrap_or(50);
        let all_output = parsers_tools::parsers_list().await?;
        let (page, next_cursor) = paginate(&all_output.parsers, cursor, page_size);
        let output = PaginatedOutput { items: page, next_cursor, total: all_output.parsers.len() };
        Ok(format_output(&output, "toon"))
    }

    // ── News Tools ──────────────────────────────────────────────

    #[tool(name = "news.fetch", description = "Fetch news from sources. Filter by pools, countries, domains, keywords. depth='deep' runs full pipeline.")]
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

    #[tool(name = "news.test_source", description = "Test a single source and return up to 10 items.")]
    async fn news_test_source(&self, params: Parameters<NewsTestInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.id.clone();
        let output = news::news_test_source(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "news.test_source",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "news.enrich", description = "Offline NLP enrichment for news items. Extracts topics, entities, sentiment, and summary. No external API calls. Use with insights.index_articles for cross-article analysis.")]
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

    #[tool(name = "weather.forecast", description = "Get weather forecast for a location. Returns daily forecasts with temp, condition, humidity, wind.")]
    async fn weather_forecast(&self, params: Parameters<WeatherForecastInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = weather::weather_forecast(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "weather.current", description = "Get current weather for a location. Returns temp, feels_like, condition, humidity, wind, visibility.")]
    async fn weather_current(&self, params: Parameters<WeatherCurrentInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = weather::weather_current(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "weather.alerts", description = "Get weather alerts for a lat/lon location. Returns active severe weather warnings.")]
    async fn weather_alerts(&self, params: Parameters<WeatherAlertsInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = weather::weather_alerts(params.0).await?;
        Ok(format_output(&output, &format))
    }

    // ── Reddit Tools ────────────────────────────────────────────

    #[tool(name = "reddit.search", description = "Search Reddit posts. Returns NewsItem[] for cross-platform analysis.")]
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

    #[tool(name = "reddit.feed", description = "Fetch latest posts from subreddits via RSS feeds. Returns NewsItem[].")]
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

    #[tool(name = "research.search", description = "Search academic papers from arXiv and Semantic Scholar. Returns ResearchPaper[].")]
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

    #[tool(name = "research.paper", description = "Get detailed paper information by ID. Returns PaperDetail with citations and references.")]
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

    #[tool(name = "research.download", description = "Download a research paper PDF to disk. Returns file path and size.")]
    async fn research_download(&self, params: Parameters<ResearchDownloadInput>) -> Result<Json<ResearchDownloadOutput>, String> {
        research::research_download(params.0).await.map(Json)
    }

    #[tool(name = "research.pubmed_search", description = "Search PubMed for medical research papers. Returns PMID, title, authors, journal.")]
    async fn research_pubmed_search(&self, params: Parameters<ResearchPubMedInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = research::research_pubmed_search(params.0).await?;
        Ok(format_output(&output, &format))
    }

    // ── Finance Tools ────────────────────────────────────────────

    #[tool(name = "finance.market", description = "Get stock market quotes for given symbols. Returns price, change, volume.")]
    async fn finance_market(&self, params: Parameters<FinanceMarketInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.symbols.join(",");
        let output = finance::finance_market(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "finance.market",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "finance.crypto", description = "Get cryptocurrency prices in USD. Returns price_usd, change_24h_pct, market_cap, volume_24h.")]
    async fn finance_crypto(&self, params: Parameters<FinanceCryptoInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.symbols.join(",");
        let output = finance::finance_crypto(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "finance.crypto",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "finance.trending", description = "Get trending cryptocurrencies on CoinGecko. Returns top 7 trending coins.")]
    async fn finance_trending(&self, params: Parameters<FinanceTrendingInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = finance::finance_trending(params.0).await?;
        Ok(format_output(&output, &format))
    }

    // ── Security Tools ──────────────────────────────────────────

    #[tool(name = "security.cve", description = "Search CVE vulnerabilities from NVD. Returns CVE ID, severity, CVSS score, affected products.")]
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

    #[tool(name = "security.advisories", description = "Search GitHub Security Advisories by ecosystem. Returns advisory ID, CVE ID, severity.")]
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

    // ── Government Tools ────────────────────────────────────────

    #[tool(name = "govt.bills", description = "Search US Congressional bills via Congress.gov API. Returns bill number, title, sponsor.")]
    async fn govt_bills(&self, params: Parameters<GovtBillsInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.query.clone();
        let output = govt::govt_bills(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "govt.bills",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "govt.regulations", description = "Search Federal Register regulations. Returns document number, title, agency.")]
    async fn govt_regulations(&self, params: Parameters<GovtRegulationsInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.query.clone();
        let output = govt::govt_regulations(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "govt.regulations",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    // ── Politics Tools ──────────────────────────────────────────

    #[tool(name = "politics.fec_candidates", description = "Search FEC for campaign finance candidate data. Returns candidate ID, name, party.")]
    async fn politics_fec_candidates(&self, params: Parameters<PoliticsFecInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = politics::politics_fec_candidates(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "politics.fec_committees", description = "Search FEC for campaign finance committee data. Returns committee ID, name, type.")]
    async fn politics_fec_committees(&self, params: Parameters<PoliticsFecCommitteesInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = politics::politics_fec_committees(params.0).await?;
        Ok(format_output(&output, &format))
    }

    // ── Patent Tools ────────────────────────────────────────────

    #[tool(name = "patents.search", description = "Search USPTO patents via PatentsView API. Returns patent number, title, date.")]
    async fn patents_search(&self, params: Parameters<PatentSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.query.clone();
        let output = patents::patents_search(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "patents.search",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(format_output(&output, &format))
    }

    #[tool(name = "patents.details", description = "Get detailed patent information by patent ID. Returns title, date, abstract, claims.")]
    async fn patents_details(&self, params: Parameters<PatentDetailsInput>) -> Result<Json<PatentDetailsOutput>, String> {
        let _subject = params.0.patent_id.clone();
        let output = patents::patents_details(params.0).await?;
        #[cfg(not(test))]
        {
            crate::tools::dump::maybe_dump(
                &self.settings,
                "patents.details",
                &_subject,
                &toon_encode(&output),
            );
        }
        Ok(Json(output))
    }

    // ── Satellite Tools ────────────────────────────────────────

    #[tool(name = "satellite.firms_fires", description = "Query NASA FIRMS active fire data for a geographic bounding box. Returns fire hotspots.")]
    async fn satellite_firms_fires(&self, params: Parameters<SatelliteFirmsInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = satellite::satellite_firms_fires(params.0).await?;
        Ok(format_output(&output, &format))
    }

    // ── Environment Tools ─────────────────────────────────────

    #[tool(name = "env.epa_facilities", description = "Search EPA regulated facilities via Envirofacts API. Returns facility name, address, coordinates.")]
    async fn env_epa_facilities(&self, params: Parameters<EnvEpaFacilitiesInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = env::env_epa_facilities(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "env.epa_emissions", description = "Search EPA Toxic Release Inventory facility emissions data. Returns facility name, state, county.")]
    async fn env_epa_emissions(&self, params: Parameters<EnvEpaEmissionsInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = env::env_epa_emissions(params.0).await?;
        Ok(format_output(&output, &format))
    }

    // ── Legal Tools ───────────────────────────────────────────

    #[tool(name = "legal.search_cases", description = "Search US court cases via CourtListener API. Returns case name, court, date filed.")]
    async fn legal_search_cases(&self, params: Parameters<LegalSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = legal::legal_search_cases(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "legal.case_details", description = "Get detailed case information by case ID from CourtListener. Returns case name, court, judges.")]
    async fn legal_case_details(&self, params: Parameters<LegalCaseDetailsInput>) -> Result<Json<LegalCaseDetailsOutput>, String> {
        legal::legal_case_details(params.0).await.map(Json)
    }

    // ── Health Tools ─────────────────────────────────────────

    #[tool(name = "health.cdc_leading_causes", description = "Query CDC leading causes of death data. Returns cause of death, state, year, deaths.")]
    async fn health_cdc_leading_causes(&self, params: Parameters<HealthCdcInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = health::health_cdc_leading_causes(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "health.who_gho", description = "Query WHO Global Health Observatory data. Returns health indicators for 194 countries.")]
    async fn health_who_gho(&self, params: Parameters<HealthWhoInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = health::health_who_gho(params.0).await?;
        Ok(format_output(&output, &format))
    }

    // ── Climate Tools ─────────────────────────────────────────

    #[tool(name = "climate.noaa_observations", description = "Query NOAA Climate Data Online for historical weather observations. Returns temperature and precipitation.")]
    async fn climate_noaa_observations(&self, params: Parameters<ClimateNoaaInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = climate::climate_noaa_observations(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "climate.noaa_stations", description = "List NOAA weather observation stations for a location. Returns station ID, name, coordinates.")]
    async fn climate_noaa_stations(&self, params: Parameters<ClimateNoaaStationsInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = climate::climate_noaa_stations(params.0).await?;
        Ok(format_output(&output, &format))
    }

    // ── Web Tools ───────────────────────────────────────────────

    #[tool(name = "web.search", description = "Realtime web search via Tavily or Firecrawl API. Returns results with title, url, content, score.")]
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

    #[tool(name = "web.scrape", description = "Scrape a URL and return structured markdown with metadata. Supports Obscura for JS rendering and stealth.")]
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

    #[tool(name = "web.crawl", description = "BFS crawl a website using Obscura headless browser. Returns pages with depth and status.")]
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

    #[tool(name = "web.map", description = "Discover URLs on a website by parsing sitemap.xml. Returns links array with url and title.")]
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

    #[tool(name = "insights.find_connections", description = "Find cross-domain entity connections in indexed articles. Returns EntityConnection with domain breakdown.")]
    async fn insight_find_connections(&self, params: Parameters<InsightFindConnectionsInput>) -> Result<Json<InsightFindConnectionsOutput>, String> {
        insights::insights_find_connections(&self.insights, params.0).await.map(Json)
    }

    #[tool(name = "insights.trending_entities", description = "Detect entities with increasing mention frequency in indexed articles.")]
    async fn insights_trending(&self, params: Parameters<InsightTrendingInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = insights::insights_trending(&self.insights, params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "insights.index_articles", description = "Index articles in the in-memory insight engine for cross-article entity analysis.")]
    async fn insights_index(&self, params: Parameters<InsightIndexInput>) -> Result<Json<InsightIndexOutput>, String> {
        insights::insights_index(&self.insights, params.0).await.map(Json)
    }

    #[tool(name = "insights.get_stats", description = "Get insight engine statistics. Returns total_articles, total_entities, total_domains.")]
    async fn insights_stats(&self) -> Result<Json<InsightStatsOutput>, String> {
        insights::insights_stats(&self.insights).await.map(Json)
    }

    #[tool(name = "insights.clear_index", description = "Clear all indexed articles from the in-memory insight engine.")]
    async fn insights_clear(&self) -> Result<Json<InsightClearOutput>, String> {
        insights::insights_clear(&self.insights).await.map(Json)
    }

    // ── Obscura Browser Automation Tools ─────────────────────────

    #[tool(name = "browser.goto", description = "Navigate to URL. Renders JS, spawns session.")]
    async fn lp_goto(&self, params: Parameters<LpGotoInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_goto(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.markdown", description = "Get page content as markdown. Supports strip_mode.")]
    async fn lp_markdown(&self, params: Parameters<LpMarkdownInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_markdown(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.links", description = "Extract all links from current page.")]
    async fn lp_links(&self, params: Parameters<LpLinksInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_links(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.evaluate", description = "Execute JavaScript in current page. Returns result.")]
    async fn lp_evaluate(&self, params: Parameters<LpEvaluateInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_evaluate(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.semantic_tree", description = "Get semantic DOM tree of current page.")]
    async fn lp_semantic_tree(&self, params: Parameters<LpSemanticTreeInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_semantic_tree(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.structured_data", description = "Extract JSON-LD, OpenGraph, microdata from page.")]
    async fn lp_structured_data(&self, params: Parameters<LpStructuredDataInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_structured_data(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.detect_forms", description = "Detect forms: fields, actions, methods.")]
    async fn lp_detect_forms(&self, params: Parameters<LpDetectFormsInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_detect_forms(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.click", description = "Click element by CSS selector.")]
    async fn lp_click(&self, params: Parameters<LpClickInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_click(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.fill", description = "Fill form field by CSS selector.")]
    async fn lp_fill(&self, params: Parameters<LpFillInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_fill(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.scroll", description = "Scroll page: up/down/left/right + pixels.")]
    async fn lp_scroll(&self, params: Parameters<LpScrollInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_scroll(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.wait_for_selector", description = "Wait for CSS selector to appear on page.")]
    async fn lp_wait_for_selector(&self, params: Parameters<LpWaitForSelectorInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_wait_for_selector(&self.obscura, params.0).await.map(Json)
    }

    #[tool(name = "browser.interactive_elements", description = "Find clickable/fillable elements on page.")]
    async fn lp_interactive_elements(&self, params: Parameters<LpInteractiveElementsInput>) -> Result<Json<LpToolOutput>, String> {
        lp_mcp::lp_interactive_elements(&self.obscura, params.0).await.map(Json)
    }

    // ── SOP Tools ─────────────────────────────────────────────

    #[tool(name = "sop.list", description = "List available SOP chains for composable multi-step intelligence workflows.")]
    async fn sop_list(&self, params: Parameters<SopListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = sop::sop_list();
        Ok(format_output(&output, &format))
    }

    #[tool(name = "sop.execute", description = "Execute a named SOP chain. Chains multiple IGS tools with dependency ordering.")]
    async fn sop_execute(&self, params: Parameters<SopExecuteInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = sop::sop_execute(params.0)?;
        Ok(format_output(&output, &format))
    }

    // ── YouTube Tools ─────────────────────────────────────────

    #[tool(name = "youtube.search", description = "Search YouTube videos by query. Returns video ID, title, URL, channel, and duration.")]
    async fn youtube_search(&self, params: Parameters<YoutubeSearchInput>) -> Result<CallToolResult, String> {
        let output = youtube::youtube_search(params.0).await?;
        Ok(format_output(&output, "json"))
    }

    #[tool(name = "youtube.metadata", description = "Get YouTube video metadata. Returns title, description, channel, duration, views, likes, upload date.")]
    async fn youtube_metadata(&self, params: Parameters<YoutubeMetadataInput>) -> Result<Json<YoutubeMetadataOutput>, String> {
        youtube::youtube_metadata(params.0).await.map(Json)
    }

    #[tool(name = "youtube.subtitles", description = "Extract YouTube video subtitles. Returns subtitle text and language used.")]
    async fn youtube_subtitles(&self, params: Parameters<YoutubeSubtitlesInput>) -> Result<Json<YoutubeSubtitlesOutput>, String> {
        youtube::youtube_subtitles(params.0).await.map(Json)
    }

    // ── Twitter Tools ─────────────────────────────────────────

    #[tool(name = "twitter.search", description = "Search tweets by query. Uses agent-twitter-client for cookie-based access. Returns Tweet[] with id, text, author, url.")]
    async fn twitter_search(&self, params: Parameters<TwitterSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = twitter::twitter_search(params.0).await?;
        Ok(format_output(&output, &format))
    }

    #[tool(name = "twitter.read", description = "Read a tweet by URL or ID. Returns full tweet details including author, text, likes, retweets.")]
    async fn twitter_read(&self, params: Parameters<TwitterReadInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = twitter::twitter_read(params.0).await?;
        Ok(format_output(&output, &format))
    }
}

// ─── MCP Server Handler ────────────────────────────────────────

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for IgsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().enable_resources().build())
            .with_server_info(Implementation::new("igs-rust-mcp", "0.2.0"))
            .with_instructions("Intelligence Gathering System MCP Server. Provides tools for RSS/HTTP source monitoring, news fetching, Reddit search, academic paper research, web search/scraping, and cross-article entity insight analysis.")
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let resource = RawResource::new("igs://tool-guide", "tool-guide")
            .with_description("Categorized tool index with decision tree, categories, and drill-down chains")
            .with_mime_type("application/json");
        Ok(ListResourcesResult {
            resources: vec![Annotated::new(resource, None)],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        if request.uri == "igs://tool-guide" {
            let guide = tool_guide::get_tool_guide().await
                .map_err(|e| ErrorData::internal_error(e, None))?;
            let json = serde_json::to_string(&guide)
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
            Ok(ReadResourceResult::new(vec![
                ResourceContents::text(json, "igs://tool-guide"),
            ]))
        } else {
            Err(ErrorData::resource_not_found(
                format!("Unknown resource: {}", request.uri),
                None,
            ))
        }
    }
}
