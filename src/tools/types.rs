use crate::types::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Pool Tool Types ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolListInput {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolListOutput {
    pub pools: Vec<Pool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolUpsertInput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolUpsertOutput {
    pub updated: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolDeleteInput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolDeleteOutput {
    pub removed: bool,
}

// ─── Source Tool Types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceListInput {
    pub pools: Option<Vec<String>>,
    pub active_only: Option<bool>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceListOutput {
    pub sources: Vec<Source>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceUpsertInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: String,
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
    pub parser: Option<String>,
    pub pools: Option<Vec<String>>,
    pub countries: Option<Vec<String>>,
    pub cities: Option<Vec<String>>,
    pub domains: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceUpsertOutput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceDeleteInput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceDeleteOutput {
    pub removed: bool,
}

// ─── Parser Tool Types ────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ParserInfo {
    pub key: String,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ParserListOutput {
    pub parsers: Vec<ParserInfo>,
}

// ─── Autodiscover Types ───────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AutodiscoverInput {
    pub url: String,
    pub pools: Option<Vec<String>>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AutodiscoverOutput {
    pub kind: String,
    pub url: Option<String>,
    pub sample: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnableScraperInput {
    pub id: String,
    pub list_url: Option<String>,
    pub selectors: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnableScraperOutput {
    pub updated: bool,
}

// ─── Country/City/Domain Types ────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GeoListInput {
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CountryInfo {
    pub name: String,
    pub code: String,
    pub source_count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CountriesOutput {
    pub countries: Vec<CountryInfo>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CityInfo {
    pub name: String,
    pub source_count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CitiesOutput {
    pub cities: Vec<CityInfo>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DomainInfoCount {
    pub name: String,
    pub source_count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DomainsOutput {
    pub domains: Vec<DomainInfoCount>,
}

// ─── News Fetch Types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsFetchInput {
    pub pools: Option<Vec<String>>,
    pub sources: Option<Vec<String>>,
    pub countries: Option<Vec<String>>,
    pub cities: Option<Vec<String>>,
    pub domains: Option<Vec<String>>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub keywords: Option<serde_json::Value>,
    pub exclude_keywords: Option<Vec<String>>,
    pub match_all: Option<bool>,
    pub discovery_mode: Option<bool>,
    pub limit: Option<i32>,
    pub cache_mode: Option<String>,
    pub urgency: Option<String>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsFetchMeta {
    pub sources_queried: usize,
    pub sources_succeeded: usize,
    pub sources_failed: usize,
    pub total_sources: usize,
    pub pool_ids: Vec<String>,
    pub keywords: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsFetchOutput {
    pub items: Vec<NewsItem>,
    pub count: usize,
    pub meta: NewsFetchMeta,
}

// ─── News Test Source Types ───────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsTestInput {
    pub id: String,
    pub cache_mode: Option<String>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsTestOutput {
    pub items: Vec<NewsItem>,
    pub count: usize,
}

// ─── News Enrich Types ────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnrichItemInput {
    pub id: String,
    pub title: String,
    pub link: String,
    pub pub_date: String,
    pub source_name: String,
    pub pool_id: String,
    pub content_snippet: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsEnrichInput {
    pub items: Vec<EnrichItemInput>,
    pub extract: Option<Vec<String>>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsEnrichOutput {
    pub items: Vec<serde_json::Value>,
    pub meta: serde_json::Value,
}

// ─── Reddit Search Types ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RedditSearchInput {
    pub query: String,
    pub subreddits: Option<Vec<String>>,
    pub sort: Option<String>,
    pub time: Option<String>,
    pub limit: Option<i32>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RedditSearchMeta {
    pub query: String,
    pub subreddits: Option<Vec<String>>,
    pub sort: String,
    pub time: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RedditSearchOutput {
    pub posts: Vec<NewsItem>,
    pub count: usize,
    pub meta: RedditSearchMeta,
}

// ─── Research Types ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchSearchInput {
    pub query: String,
    pub sources: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    pub limit: Option<i32>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchSearchMeta {
    pub query: String,
    pub sources: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchSearchOutput {
    pub papers: Vec<ResearchPaper>,
    pub count: usize,
    pub total: usize,
    pub meta: ResearchSearchMeta,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchPaperInput {
    pub paper_id: String,
    pub include_citations: Option<bool>,
    pub include_references: Option<bool>,
    pub extract_pdf: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaperCitationEntry {
    pub paper_id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PaperDetail {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub abstract_text: String,
    pub year: Option<i32>,
    pub citations: Option<i32>,
    pub references: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations_list: Option<Vec<PaperCitationEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references_list: Option<Vec<PaperCitationEntry>>,
    pub pdf_url: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchPaperOutput {
    pub paper: PaperDetail,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchDownloadInput {
    pub paper_id: String,
    pub output_path: Option<String>,
    pub format: Option<String>,
    pub convert_to_markdown: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchDownloadOutput {
    pub pdf_path: Option<String>,
    pub markdown_path: Option<String>,
    pub file_size: u64,
    pub metadata: serde_json::Value,
}

// ─── Web Search Types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchInput {
    pub query: String,
    pub provider: Option<String>,
    pub max_results: Option<i32>,
    pub topic: Option<String>,
    pub include_domains: Option<Vec<String>>,
    pub exclude_domains: Option<Vec<String>>,
    pub days: Option<i32>,
    pub include_answer: Option<bool>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchMeta {
    pub provider: String,
    pub query: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub content: Option<String>,
    pub score: Option<f64>,
    pub raw_content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchOutput {
    pub results: Vec<WebSearchResult>,
    pub count: usize,
    pub answer: Option<String>,
    pub meta: WebSearchMeta,
}

// ─── Web Scrape Types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebScrapeInput {
    pub url: String,
    /// Provider: "default" (HTTP+scraper), "lightpanda" (JS rendering)
    pub provider: Option<String>,
    pub formats: Option<Vec<String>>,
    /// Wait for CSS selector before scraping (Lightpanda only)
    pub wait_selector: Option<String>,
    /// Strip mode: "js", "css", "ui", "full" (Lightpanda only)
    pub strip_mode: Option<String>,
    /// Extract structured data (JSON-LD, OpenGraph) (Lightpanda only)
    pub structured_data: Option<bool>,
    /// Include iframe content (Lightpanda only)
    pub include_frames: Option<bool>,
    /// Wait until event: "load", "domcontentloaded", "networkidle", "done" (Lightpanda only)
    pub wait_until: Option<String>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebScrapeOutput {
    pub success: bool,
    pub url: String,
    pub title: Option<String>,
    pub markdown: Option<String>,
    pub metadata: Option<ScrapeMeta>,
    pub meta: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScrapeMeta {
    pub description: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub links_count: usize,
    pub headings: Vec<String>,
}

// ─── Web Crawl Types ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebCrawlInput {
    pub url: String,
    pub provider: Option<String>,
    pub max_depth: Option<i32>,
    pub max_pages: Option<i32>,
    pub obey_robots: Option<bool>,
    pub dump_format: Option<String>,
    pub wait_until: Option<String>,
    pub include_frames: Option<bool>,
    /// Wait for CSS selector before capturing page content
    pub wait_selector: Option<String>,
    /// Strip mode: "js", "css", "ui", "full"
    pub strip_mode: Option<String>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CrawledPage {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    pub depth: i32,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebCrawlMeta {
    pub provider: String,
    pub max_depth: i32,
    pub max_pages: i32,
    pub obey_robots: bool,
    pub dump_format: String,
    pub wait_until: String,
    pub include_frames: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebCrawlOutput {
    pub success: bool,
    pub start_url: String,
    pub pages: Vec<CrawledPage>,
    pub count: usize,
    pub meta: WebCrawlMeta,
}

// ─── Web Map Types ────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebMapInput {
    pub url: String,
    pub provider: Option<String>,
    pub limit: Option<i32>,
    pub search: Option<String>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebMapLink {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebMapMeta {
    pub provider: String,
    pub limit: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebMapOutput {
    pub success: bool,
    pub url: String,
    pub links: Vec<WebMapLink>,
    pub count: usize,
    pub meta: WebMapMeta,
}

// ─── Insight Types ────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightConnectionInput {
    pub entity: String,
    pub min_domains: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightConnectionOutput {
    pub connections: Vec<EntityConnection>,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightAllConnectionsInput {
    pub min_domains: Option<i32>,
    pub limit: Option<i32>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightAllConnectionsOutput {
    pub connections: Vec<EntityConnection>,
    pub total_found: usize,
    pub stats: InsightStats,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightTrendingInput {
    pub time_window_hours: Option<i64>,
    pub min_growth: Option<f64>,
    pub min_current_mentions: Option<u32>,
    /// Output format: "toon" (default, token-efficient) or "json"
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightTrendingOutput {
    pub trending: Vec<TrendingEntity>,
    pub count: usize,
    pub stats: InsightStats,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightIndexArticle {
    pub id: String,
    pub title: String,
    pub pub_date: String,
    pub source_name: String,
    pub domains: Option<Vec<DomainInfo>>,
    pub entities: Option<Vec<EntityInfo>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightIndexInput {
    pub articles: Vec<InsightIndexArticle>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightIndexOutput {
    pub indexed: usize,
    pub stats: InsightStats,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightStatsOutput {
    pub stats: InsightStats,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightClearOutput {
    pub cleared: bool,
}

// ─── Intelligence Pipeline Types ────────────────────────────────

/// Input for intelligence.collect — runs fetch→enrich→index pipeline
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct IntelligenceCollectInput {
    /// Pool IDs to fetch from
    pub pools: Option<Vec<String>>,
    /// Source IDs to fetch from
    pub sources: Option<Vec<String>>,
    /// Country filters
    pub countries: Option<Vec<String>>,
    /// City filters
    pub cities: Option<Vec<String>>,
    /// Domain filters
    pub domains: Option<Vec<String>>,
    /// Time range start (ISO 8601)
    pub start: Option<String>,
    /// Time range end (ISO 8601)
    pub end: Option<String>,
    /// Keywords to filter by (string or array of arrays for clusters)
    pub keywords: Option<serde_json::Value>,
    /// Keywords to exclude
    pub exclude_keywords: Option<Vec<String>>,
    /// Match all keywords
    pub match_all: Option<bool>,
    /// Max items to fetch
    pub limit: Option<i32>,
    /// Cache mode: "prefer", "bypass", "only"
    pub cache_mode: Option<String>,
    /// Skip NLP enrichment step
    pub skip_enrich: Option<bool>,
    /// Skip insight indexing step
    pub skip_index: Option<bool>,
    /// Output format: "toon" (default) or "json"
    pub format: Option<String>,
}

/// Output from intelligence.collect pipeline
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct IntelligenceCollectOutput {
    /// Number of news items fetched
    pub fetched: usize,
    /// Number of items enriched
    pub enriched: usize,
    /// Number of items indexed in insight engine
    pub indexed: usize,
    /// Insight engine stats after indexing
    pub stats: InsightStats,
    /// Fetch metadata
    pub fetch_meta: NewsFetchMeta,
}

// ─── Lightpanda MCP Browser Automation Types ───────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpGotoInput {
    /// URL to navigate to
    pub url: String,
    /// Wait until event: "load", "domcontentloaded", "networkidle", "done"
    pub wait_until: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpMarkdownInput {
    /// Strip mode: "js", "css", "ui", "full"
    pub strip_mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpLinksInput {
    /// CSS selector to scope link extraction
    pub selector: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpEvaluateInput {
    /// JavaScript expression to evaluate
    pub expression: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpSemanticTreeInput {
    /// Include text content in the tree
    pub include_text: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpStructuredDataInput {
    /// Extract JSON-LD
    pub jsonld: Option<bool>,
    /// Extract OpenGraph
    pub opengraph: Option<bool>,
    /// Extract microdata
    pub microdata: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpDetectFormsInput {
    /// CSS selector to scope form detection
    pub selector: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpClickInput {
    /// CSS selector of element to click
    pub selector: String,
    /// Wait for navigation after click
    pub wait_for_navigation: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpFillInput {
    /// CSS selector of form field
    pub selector: String,
    /// Value to fill
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpScrollInput {
    /// Direction: "up", "down", "left", "right"
    pub direction: Option<String>,
    /// Pixels to scroll
    pub pixels: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpWaitForSelectorInput {
    /// CSS selector to wait for
    pub selector: String,
    /// Timeout in ms
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpInteractiveElementsInput {
    /// CSS selector to scope
    pub selector: Option<String>,
}

/// Generic output for Lightpanda MCP tools
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpToolOutput {
    pub success: bool,
    pub content: String,
    pub meta: serde_json::Value,
}
