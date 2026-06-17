use crate::types::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types_base::{DepthOptions, DiscoveryFilters, OutputOptions};

// ─── Pool Tool Types ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolListInput {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolListOutput {
    pub pools: Vec<Pool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolUpsertInput {
    /// Pool ID (e.g., "GLOBAL_TECH_CYBER", "MY_CUSTOM_POOL")
    pub id: String,
    /// Human-readable name for the pool
    pub name: String,
    /// Optional description of the pool's purpose or scope
    pub description: Option<String>,
    /// Whether the pool is active and queries should include it (default: true)
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolUpsertOutput {
    pub updated: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolDeleteInput {
    /// Pool ID to delete (e.g., "MY_CUSTOM_POOL")
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolDeleteOutput {
    pub removed: bool,
}

// ─── Source Tool Types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceListInput {
    /// Filter sources by pool IDs (e.g., ["GLOBAL_TECH_CYBER"]). Returns all if omitted.
    pub pools: Option<Vec<String>>,
    /// If true, return only active sources. Default: false (returns all).
    pub active_only: Option<bool>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceListOutput {
    pub sources: Vec<Source>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceUpsertInput {
    /// Unique identifier for the source (auto-generated from name if omitted, e.g., "reuters")
    pub id: Option<String>,
    /// Human-readable name of the source (e.g., "Reuters", "BBC World")
    pub name: String,
    /// Source type identifier (e.g., "rss", "generic_html", "hackernews", "youtube")
    #[serde(rename = "type")]
    pub source_type: String,
    /// URL of the feed or webpage (e.g., "https://www.reuters.com/world/feed.xml")
    pub url: String,
    /// Custom HTTP headers for requests (e.g., {"Authorization": "Bearer token"})
    pub headers: Option<HashMap<String, String>>,
    /// Parser key: "rss" (default), "generic_html", "hackernews", etc. See parsers.list.
    pub parser: Option<String>,
    /// Pool IDs this source belongs to (e.g., ["GLOBAL_TECH_CYBER", "GLOBAL_BREAKING"])
    pub pools: Option<Vec<String>>,
    /// ISO 3166-1 alpha-2 country codes (e.g., ["US", "IN", "GB"])
    pub countries: Option<Vec<String>>,
    /// City names for local sources (e.g., ["Delhi", "London", "New York"])
    pub cities: Option<Vec<String>>,
    /// Topical domain tags (e.g., ["tech", "cyber", "defense", "health"])
    pub domains: Option<Vec<String>>,
    /// Whether the source is active and fetched on queries (default: true)
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceUpsertOutput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceDeleteInput {
    /// ID of the source to permanently delete (e.g., "reuters", "bbc_world")
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
    /// Homepage URL to discover RSS/Atom feeds or sitemaps from (e.g., "https://example.com")
    pub url: String,
    /// Pool IDs to assign the discovered source to (e.g., ["GLOBAL_TECH_CYBER"])
    pub pools: Option<Vec<String>>,
    /// Optional name override for the discovered source
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
    /// Source ID to enable generic HTML scraping for
    pub id: String,
    /// URL to fetch the listing page from (defaults to the source's base URL)
    pub list_url: Option<String>,
    /// CSS selectors for scraping: {"item": "div.article", "title": "h2 a", "link": "h2 a", "date": ".date", "desc": ".summary"}
    pub selectors: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnableScraperOutput {
    pub updated: bool,
}

// ─── Country/City/Domain Types ────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GeoListInput {
    #[serde(flatten)]
    pub output: OutputOptions,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NewsFetchInput {
    #[serde(flatten)]
    pub filters: DiscoveryFilters,
    /// If true, perform a broad shallow scan across all pools to discover relevant sources
    pub discovery_mode: Option<bool>,
    /// Filter by urgency level (implementation-defined values)
    pub urgency: Option<String>,
    /// Skip NLP enrichment step (only applies when depth > quick)
    pub skip_enrich: Option<bool>,
    /// Skip insight indexing step (only applies when depth > quick)
    pub skip_index: Option<bool>,
    #[serde(flatten)]
    pub depth_opts: DepthOptions,
    #[serde(flatten)]
    pub output: OutputOptions,
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
pub struct ClusterInfo {
    pub representative: NewsItem,
    pub member_count: usize,
    pub entities: Vec<String>,
    pub source_count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsFetchOutput {
    pub items: Vec<NewsItem>,
    pub count: usize,
    pub meta: NewsFetchMeta,
    pub clusters: Option<Vec<ClusterInfo>>,
}

// ─── News Test Source Types ───────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsTestInput {
    /// Source ID to test (e.g., "bbc_world", "techcrunch")
    pub id: String,
    /// Cache mode: "prefer" (use cache if fresh), "bypass" (force fetch), "only" (cache only)
    pub cache_mode: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsTestOutput {
    pub items: Vec<NewsItem>,
    pub count: usize,
}

// ─── News Enrich Types ────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnrichItemInput {
    /// Unique identifier for the news item
    pub id: String,
    /// Title of the news article
    pub title: String,
    /// URL to the full article
    pub link: String,
    /// Publication date as ISO 8601 string (e.g., "2026-01-15T10:30:00Z")
    pub pub_date: String,
    /// Name of the source that published this item (e.g., "Reuters", "TechCrunch")
    pub source_name: String,
    /// Pool ID the item was fetched from (e.g., "GLOBAL_TECH_CYBER")
    pub pool_id: String,
    /// Content snippet or excerpt from the article
    pub content_snippet: Option<String>,
    /// Confidence level of the parsed date: "high", "medium", "low"
    pub date_confidence: Option<String>,
    /// Freshness score: 0.0–100.0 based on recency (exponential decay)
    pub freshness_score: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsEnrichInput {
    /// News items to enrich with NLP processing (sourced from news.fetch output)
    pub items: Vec<EnrichItemInput>,
    /// NLP features to extract: "topics", "entities", "sentiment", "summary", "diversity". Omit for all.
    pub extract: Option<Vec<String>>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

/// A single enriched news item with NLP annotations
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EnrichedItem {
    /// Original news item fields flattened into this struct
    #[serde(flatten)]
    pub item: serde_json::Value,
    /// Extracted topics from the content
    #[serde(default)]
    pub topics: Vec<String>,
    /// Named entities detected in the content
    #[serde(default)]
    pub entities: Vec<EntityInfo>,
    /// Sentiment analysis result
    #[serde(default)]
    pub sentiment: Option<SentimentResult>,
    /// Brief summary of the article
    #[serde(default)]
    pub summary: Option<String>,
}

/// Metadata about the enrichment process
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EnrichmentMeta {
    /// Number of items enriched
    pub enriched_count: usize,
    /// NLP features applied (e.g., ["topics", "entities", "sentiment", "summary"])
    pub features: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsEnrichOutput {
    /// Enriched news items with NLP data
    pub items: Vec<EnrichedItem>,
    /// Metadata about the enrichment process
    pub meta: EnrichmentMeta,
}

// ─── Reddit Search Types ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RedditSearchInput {
    /// Search query string to find Reddit posts
    pub query: String,
    /// Subreddit names to search within (e.g., ["worldnews", "technology"]). Searches all if omitted.
    pub subreddits: Option<Vec<String>>,
    /// Sort order: "relevance" (default), "hot", "top", "new", "comments"
    pub sort: Option<String>,
    /// Time filter: "hour", "day", "week", "month", "year", "all" (default: "all")
    pub time: Option<String>,
    /// Maximum number of results to return (default: 25)
    pub limit: Option<i32>,
    #[serde(flatten)]
    pub output: OutputOptions,
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

// ─── Reddit Feed Types ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RedditFeedInput {
    /// Subreddit name(s) without r/ prefix (e.g. ["worldnews", "technology"])
    pub subreddits: Vec<String>,
    /// Limit per subreddit (default: 25, max: 100)
    pub limit: Option<i32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RedditFeedOutput {
    pub posts: Vec<NewsItem>,
    pub count: usize,
    pub subreddits: Vec<String>,
}

// ─── Research Types ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchSearchInput {
    /// Search query string (e.g., "transformer architecture attention mechanism")
    pub query: String,
    /// Search engines to query: "arxiv", "semanticscholar" (default: both)
    pub sources: Option<Vec<String>>,
    /// arXiv category IDs to filter by (e.g., ["cs.AI", "cs.CL", "cs.LG"])
    pub categories: Option<Vec<String>>,
    /// Earliest publication year (inclusive, e.g., 2020)
    pub year_from: Option<i32>,
    /// Latest publication year (inclusive, e.g., 2024)
    pub year_to: Option<i32>,
    /// Maximum number of results to return (default: 25)
    pub limit: Option<i32>,
    #[serde(flatten)]
    pub output: OutputOptions,
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
    /// Paper ID in format "arxiv:XXXX.XXXXX" or "semanticscholar:XXXX"
    pub paper_id: String,
    /// Include list of papers that cite this paper (default: false)
    pub include_citations: Option<bool>,
    /// Include list of papers referenced by this paper (default: false)
    pub include_references: Option<bool>,
    /// Extract PDF content as plain text (default: false)
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
    /// Paper ID in format "arxiv:XXXX.XXXXX" or "semanticscholar:XXXX"
    pub paper_id: String,
    /// Custom output file path (default: "{paper_id}.pdf" in working directory)
    pub output_path: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Also generate a markdown sidecar file alongside the PDF (default: false)
    pub convert_to_markdown: Option<bool>,
}

/// Metadata about a downloaded research paper
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaperMetadata {
    /// Paper title
    pub title: String,
    /// Paper ID (e.g., "arxiv:2301.00001")
    pub id: String,
    /// Publication year
    pub year: Option<u32>,
    /// Number of pages
    pub pages: Option<u32>,
    /// File size in bytes
    pub file_size: u64,
    /// Local file path where PDF was saved
    pub file_path: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchDownloadOutput {
    pub pdf_path: Option<String>,
    pub markdown_path: Option<String>,
    pub file_size: u64,
    pub metadata: PaperMetadata,
}

// ─── Web Search Types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchInput {
    /// Search query string
    pub query: String,
    /// Search provider: "tavily" (default) or "firecrawl"
    pub provider: Option<String>,
    /// Maximum number of results to return (default: 10, max: 20)
    pub max_results: Option<i32>,
    /// Topic type: "general" (default) or "news"
    pub topic: Option<String>,
    /// Domains to restrict results to (e.g., ["arxiv.org", "github.com"])
    pub include_domains: Option<Vec<String>>,
    /// Domains to exclude from results (e.g., ["reddit.com"])
    pub exclude_domains: Option<Vec<String>>,
    /// Lookback period in days from current date (only applies when topic="news")
    pub days: Option<i32>,
    /// Include an AI-generated answer summary (default: false)
    pub include_answer: Option<bool>,
    #[serde(flatten)]
    pub output: OutputOptions,
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
    /// URL to scrape content from (must be http:// or https://)
    pub url: String,
    /// Provider: "default" (HTTP+scraper), "lightpanda" (JS rendering, requires lightpanda.enabled=true)
    pub provider: Option<String>,
    /// Content formats to return: "markdown", "html", "text", "screenshot" (Lightpanda only)
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
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebScrapeOutput {
    pub success: bool,
    pub url: String,
    pub title: Option<String>,
    pub markdown: Option<String>,
    pub metadata: Option<ScrapeStructuredData>,
    pub meta: ScrapeMeta,
}

/// Structured data extracted from the scraped page (OpenGraph, description, headings)
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScrapeStructuredData {
    pub description: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub links_count: usize,
    pub headings: Vec<String>,
}

/// Metadata about the scrape operation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScrapeMeta {
    /// Final URL after redirects
    pub url: String,
    /// HTTP status code
    pub status: u16,
    /// Content type of the response
    pub content_type: Option<String>,
    /// Time taken in milliseconds
    pub elapsed_ms: u64,
    /// Whether JavaScript was rendered (Lightpanda mode)
    pub js_rendered: bool,
}

// ─── Web Crawl Types ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebCrawlInput {
    /// Starting URL for the BFS crawl (must be http:// or https://)
    pub url: String,
    /// Crawl provider: "default" or "lightpanda" (requires lightpanda.enabled=true, renders JS)
    pub provider: Option<String>,
    /// Maximum BFS depth from the starting URL (default: 2)
    pub max_depth: Option<i32>,
    /// Maximum number of pages to crawl before stopping (default: 20)
    pub max_pages: Option<i32>,
    /// Respect robots.txt directives (default: true)
    pub obey_robots: Option<bool>,
    /// Output content format: "markdown" (default), "html", "semantic_tree"
    pub dump_format: Option<String>,
    /// When to consider the page captured: "load", "domcontentloaded", "networkidle" (default), "done"
    pub wait_until: Option<String>,
    /// Include iframe content in results (default: false, Lightpanda only)
    pub include_frames: Option<bool>,
    /// Wait for CSS selector before capturing page content (Lightpanda only)
    pub wait_selector: Option<String>,
    /// Strip mode: "js", "css", "ui", "full" (Lightpanda only)
    pub strip_mode: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
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
    /// Website URL to discover sitemap entries from (fetches /sitemap.xml)
    pub url: String,
    /// Provider: "default" (HTTP) or "lightpanda" (requires lightpanda.enabled=true)
    pub provider: Option<String>,
    /// Maximum number of links to return (default: 100)
    pub limit: Option<i32>,
    /// Filter discovered URLs containing this substring
    pub search: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
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
pub struct InsightFindConnectionsInput {
    /// Entity to find connections for (omit to discover all cross-domain entities)
    pub entity: Option<String>,
    /// Minimum number of domains for a connection to be included (default: 2)
    pub min_domains: Option<i32>,
    /// Maximum number of results to return (default: 20, only used when entity is omitted)
    pub limit: Option<i32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightFindConnectionsOutput {
    pub connections: Vec<EntityConnection>,
    pub count: usize,
    /// Only present when entity is omitted (all connections mode)
    pub total_found: Option<usize>,
    /// Only present when entity is omitted (all connections mode)
    pub stats: Option<InsightStats>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightTrendingInput {
    /// Time window in hours for trend detection compared to prior window (default: 24)
    pub time_window_hours: Option<i64>,
    /// Minimum growth ratio vs prior period to qualify as trending (default: 2.0)
    pub min_growth: Option<f64>,
    /// Minimum mentions in the current window to be considered (default: 3)
    pub min_current_mentions: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightTrendingOutput {
    pub trending: Vec<TrendingEntity>,
    pub count: usize,
    pub stats: InsightStats,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightIndexArticle {
    /// Unique identifier for the article
    pub id: String,
    /// Article title
    pub title: String,
    /// Publication date as ISO 8601 string (e.g., "2026-01-15T10:30:00Z")
    pub pub_date: String,
    /// Name of the source that published this article
    pub source_name: String,
    /// Topical domains for this article (leave empty for auto-detection)
    pub domains: Option<Vec<DomainInfo>>,
    /// Extracted entities from this article (leave empty for auto-extraction)
    pub entities: Option<Vec<EntityInfo>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightIndexInput {
    /// Articles to index for cross-article entity analysis (use enriched items for best results)
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

/// Metadata about a Lightpanda browser operation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BrowserMeta {
    /// Current URL after operation
    pub url: String,
    /// Page title
    pub title: Option<String>,
    /// Operation type (e.g., "goto", "click", "fill")
    pub operation: String,
    /// Time taken in milliseconds
    pub elapsed_ms: u64,
}

/// Generic output for Lightpanda MCP tools
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpToolOutput {
    pub success: bool,
    pub content: String,
    pub meta: BrowserMeta,
}
