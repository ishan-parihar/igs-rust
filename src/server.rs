use crate::config;
use crate::lightpanda::LightpandaManager;
use crate::lightpanda_mcp::LightpandaMcpClient;
use crate::persistence;
use crate::tools::{helpers::toon_encode, insights, intelligence, lp_mcp, news, parsers as parsers_tools, pools, reddit, research, sources, types::*, web};
#[allow(unused_imports)]
use crate::types::*;
use rmcp::{
    Json,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
};
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

impl HasFormat for SourceListInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for GeoListInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for NewsFetchInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for NewsTestInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for NewsEnrichInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for RedditSearchInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for RedditFeedInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for ResearchSearchInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for ResearchDownloadInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for WebSearchInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for WebScrapeInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for WebCrawlInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for WebMapInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for InsightAllConnectionsInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for InsightTrendingInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

impl HasFormat for IntelligenceCollectInput {
    fn format(&self) -> &Option<String> { &self.output.format }
}

// ─── Server State ────────────────────────────────────────────────

#[derive(Clone)]
pub struct IgsMcpServer {
    tool_router: ToolRouter<IgsMcpServer>,
    insights: Arc<Mutex<InsightStorage>>,
    lightpanda_mcp: Arc<Mutex<Option<LightpandaMcpClient>>>,
}

// ─── Tool Router ────────────────────────────────────────────────

impl Default for IgsMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl IgsMcpServer {
    /// Resolve the output format from any input type that carries format options.
    pub fn resolve_format(params: &impl HasFormat) -> String {
        params.format().as_deref().unwrap_or("toon").to_string()
    }
}

#[tool_router(router = tool_router)]
impl IgsMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            insights: Arc::new(Mutex::new(InsightStorage::new())),
            lightpanda_mcp: Arc::new(Mutex::new(None)),
        }
    }

    // ── Pool Tools ──────────────────────────────────────────────

    #[tool(name = "pools.list", description = "List all configured source pools. Pools group related news sources (e.g. GLOBAL_TECH_CYBER, INDIA_NATIONAL_BASE). Use pool IDs as filters in news.fetch. Returns Pool[] with id, name, description, is_active.")]
    async fn pools_list(&self) -> Result<Json<PoolListOutput>, String> {
        let result: PoolListOutput = pools::pools_list().await?;
        Ok(Json(result))
    }

    #[tool(name = "pools.upsert", description = "Create or update a source pool. Pools group related news sources for batch fetching. Input: id (unique identifier like GLOBAL_TECH_CYBER), name (display name), description (what the pool covers), is_active (default true). Use pools.list to see existing pools.")]
    async fn pools_upsert(&self, params: Parameters<PoolUpsertInput>) -> Result<Json<PoolUpsertOutput>, String> {
        let result: PoolUpsertOutput = pools::pools_upsert(params.0).await?;
        Ok(Json(result))
    }

    #[tool(name = "pools.delete", description = "Permanently delete a pool by ID. Does not delete sources in the pool — only removes the grouping. Use pools.list to find the pool ID first.")]
    async fn pools_delete(&self, params: Parameters<PoolDeleteInput>) -> Result<Json<PoolDeleteOutput>, String> {
        let result: PoolDeleteOutput = pools::pools_delete(params.0).await?;
        Ok(Json(result))
    }

    // ── Source Tools ────────────────────────────────────────────

    #[tool(name = "sources.list", description = "List configured news sources (410+ across 47 countries). Filter by pools (pool IDs) or active_only=true. Returns Source[] with id, name, type, url, parser, pools, countries, cities, domains. Default output: TOON.")]
    async fn sources_list(&self, params: Parameters<SourceListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = sources::sources_list(params.0).await?;
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "sources.upsert", description = "Create or update a news source. Required: name, type (rss/generic_html/ofac/who_dons/newslaundry), url. Optional: id (auto-generated from name), headers (custom HTTP headers), parser (key from parsers.list), pools (pool IDs), countries (ISO codes), cities, domains, is_active. Use sources.autodiscover to auto-detect feeds first.")]
    async fn sources_upsert(&self, params: Parameters<SourceUpsertInput>) -> Result<Json<SourceUpsertOutput>, String> {
        sources::sources_upsert(params.0).await.map(Json)
    }

    #[tool(name = "sources.delete", description = "Permanently delete a source by ID from sources.yml. Cannot be undone. Use sources.list to find the source ID first.")]
    async fn sources_delete(&self, params: Parameters<SourceDeleteInput>) -> Result<Json<SourceDeleteOutput>, String> {
        sources::sources_delete(params.0).await.map(Json)
    }

    #[tool(name = "sources.autodiscover", description = "Auto-discover RSS/Atom feeds or sitemap from a homepage URL. Fetches the URL, looks for <link rel='alternate'> RSS/Atom tags, falls back to /sitemap.xml. Returns kind (rss/sitemap/none), url, sample items. Optionally adds discovered source to sources.yml with pools and name.")]
    async fn sources_autodiscover(&self, params: Parameters<AutodiscoverInput>) -> Result<Json<AutodiscoverOutput>, String> {
        sources::sources_autodiscover(params.0).await.map(Json)
    }

    #[tool(name = "sources.enableGenericScraper", description = "Enable generic HTML scraping for a source. Sets parser to generic_html with CSS selectors. Input: source id, optional list_url (page to scrape), selectors (item, title, link, date, desc CSS selectors). Use sources.autodiscover first to find the source, then enable scraping for non-RSS sources.")]
    async fn sources_enable_scraper(&self, params: Parameters<EnableScraperInput>) -> Result<Json<EnableScraperOutput>, String> {
        sources::sources_enable_scraper(params.0).await.map(Json)
    }

    #[tool(name = "sources.countries", description = "List countries with source counts. Returns CountryInfo[] with name, ISO code, and source_count. Use ISO codes (IN, US, GB, etc.) as filters in news.fetch countries parameter. Default output: TOON.")]
    async fn sources_countries(&self, params: Parameters<GeoListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = sources::sources_countries().await?;
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "sources.cities", description = "List cities with source counts. Returns CityInfo[] with name and source_count. Use city names as filters in news.fetch cities parameter. Sorted by source count descending. Default output: TOON.")]
    async fn sources_cities(&self, params: Parameters<GeoListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = sources::sources_cities().await?;
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "sources.domains", description = "List domains with source counts. Returns DomainInfoCount[] with name and source_count. Domains are topical tags (geopolitics, business, tech, cyber, defense, health, etc.). Use domain names as filters in news.fetch domains parameter. Default output: TOON.")]
    async fn sources_domains(&self, params: Parameters<GeoListInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = sources::sources_domains().await?;
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    // ── Parser Tools ────────────────────────────────────────────

    #[tool(name = "parsers.list", description = "List available source parser keys. Use these keys in sources.upsert parser field. Available: rss (RSS/Atom feeds), ofac (US Treasury OFAC), who_dons (WHO Disease Outbreak News), newslaundry (Newslaundry JSON-in-script), generic_html (CSS selector-based HTML scraping), ussf_cfc (US Space Force), hackernews (HN Algolia JSON), youtube (YouTube Atom RSS), github (GitHub releases/search), bluesky (Bluesky AT Protocol), semantic_scholar (Semantic Scholar API). Auto-detects if parser not specified.")]
    async fn parsers_list(&self) -> Result<Json<ParserListOutput>, String> {
        parsers_tools::parsers_list().await.map(Json)
    }

    // ── News Tools ──────────────────────────────────────────────

    #[tool(name = "news.fetch", description = "Fetch news from 410+ configured sources across 47 countries. Filter by pools (e.g. GLOBAL_TECH_CYBER, INDIA_NATIONAL_BASE), countries (ISO codes), cities, domains, time range, and keywords. Supports keyword clusters (OR within, AND across). Use depth='deep' for source site crawl with date_confidence scoring; depth='full' for multi-source enrichment. Returns NewsItem[] with date_confidence, freshness_score. At depth='deep' with >=5 items, returns ClusterInfo with entity clusters. Weighted RRF fusion auto-deduplicates across sources. Per-author dedup caps items per author. Default output: TOON (token-efficient). Use format='json' for standard JSON.")]
    async fn news_fetch(&self, params: Parameters<NewsFetchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.filters.pools.as_ref().and_then(|p| p.first()).cloned().unwrap_or_else(|| "news".to_string());
        let output = news::news_fetch(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "news.fetch",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "news.testSource", description = "Test a single source and return up to 10 items. Input: source ID (from sources.list). Useful for debugging source configuration, parser issues, or verifying a new source works. Returns NewsItem[].")]
    async fn news_test_source(&self, params: Parameters<NewsTestInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.id.clone();
        let output = news::news_test_source(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "news.testSource",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "news.enrich", description = "Offline NLP enrichment for news items. Input: items from news.fetch output (map id, title, link, pub_date, source_name, pool_id, content_snippet). Output: items with topics (word frequency), entities (capitalized word sequences), sentiment (keyword-based), summary (first sentence). Pass extract=['topics','entities','sentiment','summary'] to select what to extract, or omit for all. Add 'diversity' to extract for source diversity metrics. No external API calls. Use with insights.indexArticles to enable cross-article analysis.")]
    async fn news_enrich(&self, params: Parameters<NewsEnrichInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = format!("enrich-{}", params.0.items.len());
        let output = news::news_enrich(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "news.enrich",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    // ── Reddit Tools ────────────────────────────────────────────

    #[tool(name = "reddit.search", description = "Search Reddit posts via reddit.com JSON API. Supports subreddits filter (e.g. [\"worldnews\",\"technology\"]), sort (relevance/hot/top/new), time (hour/day/week/month/year/all). Returns NewsItem[] compatible with news.enrich and insights.indexArticles for cross-platform analysis.")]
    async fn reddit_search(&self, params: Parameters<RedditSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.subreddits.as_ref().and_then(|s| s.first()).cloned().unwrap_or_else(|| params.0.query.clone());
        let output = reddit::reddit_search(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "reddit.search",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "reddit.feed", description = "Fetch latest posts from subreddits via RSS feeds (old.reddit.com/r/{sub}/.rss). Reliable cross-platform access that works without API keys or residential IPs. Pass subreddit names without r/ prefix. Returns NewsItem[] compatible with news.enrich and insights.indexArticles.")]
    async fn reddit_feed(&self, params: Parameters<RedditFeedInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.subreddits.first().cloned().unwrap_or_default();
        let output = reddit::reddit_feed(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "reddit.feed",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    // ── Research Tools ──────────────────────────────────────────

    #[tool(name = "research.search", description = "Search academic papers from arXiv and Semantic Scholar. Supports categories (e.g. cs.AI, cs.CL), year_from, year_to filtering. Returns ResearchPaper[] with id (format: arxiv:XXXX or semanticscholar:XXXX), title, authors, abstract, year, citation_count, pdf_url. Use research.paper for details or research.download for PDF.")]
    async fn research_search(&self, params: Parameters<ResearchSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.query.clone();
        let output = research::research_search(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "research.search",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "research.paper", description = "Get detailed paper information by ID. ID format: arxiv:XXXX.XXXXX or semanticscholar:XXXX. Returns PaperDetail with title, authors, abstract, year, citations, references, pdf_url. Optionally include_citations, include_references, extract_pdf.")]
    async fn research_paper(&self, params: Parameters<ResearchPaperInput>) -> Result<Json<ResearchPaperOutput>, String> {
        let _subject = params.0.paper_id.clone();
        let output = research::research_paper(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "research.paper",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        Ok(Json(output))
    }

    #[tool(name = "research.download", description = "Download a research paper PDF to disk. ID format: arxiv:XXXX.XXXXX or semanticscholar:XXXX. For Semantic Scholar, fetches PDF URL from API first. Optional output_path (default: {paper_id}.pdf) and format. Returns file path and size.")]
    async fn research_download(&self, params: Parameters<ResearchDownloadInput>) -> Result<Json<ResearchDownloadOutput>, String> {
        research::research_download(params.0).await.map(Json)
    }

    // ── Web Tools ───────────────────────────────────────────────

    #[tool(name = "web.search", description = "Realtime web search via Tavily (default) or Firecrawl API. Requires tavily.enabled=true or firecrawl.enabled=true in settings.yml with API key. Supports include_domains, exclude_domains, days, include_answer. Returns results array with title, url, content, score. Default output: TOON. Use format='json' for structured JSON.")]
    async fn web_search(&self, params: Parameters<WebSearchInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = params.0.query.clone();
        let output = web::web_search(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "web.search",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "web.scrape", description = "Scrape a URL and return structured markdown with metadata (title, headings, og:description, link count). Provider 'default' uses HTTP+html-to-markdown. Provider 'lightpanda' renders JavaScript — set lightpanda.enabled=true in settings.yml first. Lightpanda supports wait_selector, strip_mode, wait_until, include_frames for JS-heavy sites. Default output: TOON.")]
    async fn web_scrape(&self, params: Parameters<WebScrapeInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = url::Url::parse(&params.0.url).map(|u| u.host_str().unwrap_or("unknown").to_string()).unwrap_or_else(|_| params.0.url.clone());
        let output = web::web_scrape(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "web.scrape",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "web.crawl", description = "BFS crawl a website using Lightpanda headless browser. Renders JavaScript. Requires lightpanda.enabled=true in settings.yml (binary auto-downloads). Supports max_depth (default 2), max_pages (default 20), obey_robots, dump_format (markdown/html/semantic_tree), wait_until, wait_selector, strip_mode, include_frames. Returns pages with depth and status. Default output: TOON.")]
    async fn web_crawl(&self, params: Parameters<WebCrawlInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = url::Url::parse(&params.0.url).map(|u| u.host_str().unwrap_or("unknown").to_string()).unwrap_or_else(|_| params.0.url.clone());
        let output = web::web_crawl(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "web.crawl",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "web.map", description = "Discover URLs on a website by parsing sitemap.xml. Fetches /sitemap.xml, extracts <loc> URLs. Supports limit (default 100) and search filter. Returns WebMapOutput with links array containing url and optional title. Default output: TOON.")]
    async fn web_map(&self, params: Parameters<WebMapInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let _subject = url::Url::parse(&params.0.url).map(|u| u.host_str().unwrap_or("unknown").to_string()).unwrap_or_else(|_| params.0.url.clone());
        let output = web::web_map(params.0).await?;
        #[cfg(not(test))]
        {
            if let Ok(settings) = crate::config::load_settings().await {
                crate::tools::dump::maybe_dump(
                    &settings,
                    "web.map",
                    &_subject,
                    &toon_encode(&output),
                );
            }
        }
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    // ── Insight Tools ───────────────────────────────────────────

    #[tool(name = "insights.findConnections", description = "Find cross-domain entity connections in indexed articles. Requires articles indexed via insights.indexArticles or intelligence.collect. Returns EntityConnection with domain breakdown and article IDs. Use min_domains to filter (default 2).")]
    async fn insights_find_connections(&self, params: Parameters<InsightConnectionInput>) -> Result<Json<InsightConnectionOutput>, String> {
        insights::insights_find_connections(&self.insights, params.0).await.map(Json)
    }

    #[tool(name = "insights.findAllConnections", description = "Discover all entities appearing across multiple domains in indexed articles. Returns EntityConnection[] sorted by connection_strength. Requires articles indexed via insights.indexArticles or intelligence.collect. Use min_domains (default 2) and limit to control output size.")]
    async fn insights_find_all_connections(&self, params: Parameters<InsightAllConnectionsInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = insights::insights_find_all_connections(&self.insights, params.0).await?;
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "insights.trendingEntities", description = "Detect entities with increasing mention frequency in indexed articles. Compares current time window vs previous. Requires articles indexed via insights.indexArticles or intelligence.collect. Use time_window_hours (default 24), min_growth (default 2.0), min_current_mentions (default 3).")]
    async fn insights_trending(&self, params: Parameters<InsightTrendingInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = insights::insights_trending(&self.insights, params.0).await?;
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    #[tool(name = "insights.indexArticles", description = "Index articles in the in-memory insight engine for cross-article entity analysis. Input: articles with id, title, pub_date, source_name, and optionally domains (Vec<DomainInfo>) and entities (Vec<EntityInfo>). Use intelligence.collect to automate fetch→enrich→index pipeline. After indexing, use insights.findConnections or insights.trendingEntities.")]
    async fn insights_index(&self, params: Parameters<InsightIndexInput>) -> Result<Json<InsightIndexOutput>, String> {
        insights::insights_index(&self.insights, params.0).await.map(Json)
    }

    #[tool(name = "insights.getStats", description = "Get insight engine statistics. Returns total_articles, total_entities, total_domains, avg_entities_per_article, avg_domains_per_article. Use to check what's been indexed before running insights.findConnections or insights.trendingEntities.")]
    async fn insights_stats(&self) -> Result<Json<InsightStatsOutput>, String> {
        insights::insights_stats(&self.insights).await.map(Json)
    }

    #[tool(name = "insights.clearIndex", description = "Clear all indexed articles from the in-memory insight engine. Resets all entity connections, trending data, and statistics. Use insights.getStats first to see what will be lost.")]
    async fn insights_clear(&self) -> Result<Json<InsightClearOutput>, String> {
        insights::insights_clear(&self.insights).await.map(Json)
    }

    // ── Intelligence Pipeline Tools ─────────────────────────────

    #[tool(name = "intelligence.collect", description = "Full intelligence pipeline in one call: fetch news → enrich with NLP → index in insight engine. Combines news.fetch, news.enrich, and insights.indexArticles. Use skip_enrich=true or skip_index=true to skip steps. Pass depth='deep' to enable source site crawl with date_confidence scoring and entity clustering. After indexing, use insights.findConnections or insights.trendingEntities for cross-article analysis.")]
    async fn intelligence_collect(&self, params: Parameters<IntelligenceCollectInput>) -> Result<CallToolResult, String> {
        let format = Self::resolve_format(&params.0);
        let output = intelligence::intelligence_collect(&self.insights, params.0).await?;
        let text = if format == "json" {
            serde_json::to_string_pretty(&output).unwrap_or_default()
        } else {
            toon_encode(&output)
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    // ── Lightpanda MCP Browser Automation Tools ──────────────────

    #[tool(name = "lightpanda.goto", description = "Navigate to a URL using Lightpanda headless browser. Renders JavaScript. Spawns persistent browser session on first call. Use wait_until to control when page is considered loaded.")]
    async fn lp_goto(&self, params: Parameters<LpGotoInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_goto(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.markdown", description = "Get the current page content as structured markdown. Use after lightpanda.goto to extract content. Supports strip_mode to remove js/css/ui elements.")]
    async fn lp_markdown(&self, params: Parameters<LpMarkdownInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_markdown(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.links", description = "Extract all links from the current page. Returns URLs and link text. Use after lightpanda.goto.")]
    async fn lp_links(&self, params: Parameters<LpLinksInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_links(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.evaluate", description = "Execute JavaScript in the current page context. Returns the result of the expression. Use after lightpanda.goto. Example: expressions like document.title, document.querySelectorAll('h1').length")]
    async fn lp_evaluate(&self, params: Parameters<LpEvaluateInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_evaluate(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.semantic_tree", description = "Get the semantic DOM tree of the current page. AI-friendly representation of page structure. Use after lightpanda.goto.")]
    async fn lp_semantic_tree(&self, params: Parameters<LpSemanticTreeInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_semantic_tree(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.structuredData", description = "Extract structured data from the current page: JSON-LD, OpenGraph metadata, microdata. Use after lightpanda.goto.")]
    async fn lp_structured_data(&self, params: Parameters<LpStructuredDataInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_structured_data(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.detectForms", description = "Detect forms on the current page. Returns form fields, actions, and methods. Use after lightpanda.goto to find forms for filling.")]
    async fn lp_detect_forms(&self, params: Parameters<LpDetectFormsInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_detect_forms(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.click", description = "Click an element on the current page by CSS selector. Optionally wait for navigation. Use after lightpanda.goto.")]
    async fn lp_click(&self, params: Parameters<LpClickInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_click(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.fill", description = "Fill a form field on the current page. Use CSS selector to target the field. Use after lightpanda.goto and optionally lightpanda.detectForms.")]
    async fn lp_fill(&self, params: Parameters<LpFillInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_fill(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.scroll", description = "Scroll the current page. Direction: up/down/left/right. Pixels: amount to scroll. Use after lightpanda.goto.")]
    async fn lp_scroll(&self, params: Parameters<LpScrollInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_scroll(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.waitForSelector", description = "Wait for a CSS selector to appear on the page. Useful for SPAs that load content dynamically. Use after lightpanda.goto.")]
    async fn lp_wait_for_selector(&self, params: Parameters<LpWaitForSelectorInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
            .ensure_ready().await.map_err(|e| format!("{}", e))?;
        lp_mcp::lp_wait_for_selector(&self.lightpanda_mcp, &binary, params.0).await.map(Json)
    }

    #[tool(name = "lightpanda.interactiveElements", description = "Find interactive elements on the current page (buttons, links, inputs). Returns clickable/fillable elements. Use after lightpanda.goto.")]
    async fn lp_interactive_elements(&self, params: Parameters<LpInteractiveElementsInput>) -> Result<Json<LpToolOutput>, String> {
        let binary = LightpandaManager::new(&config::load_settings().await.map_err(|e| format!("{}", e))?.lightpanda)
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
