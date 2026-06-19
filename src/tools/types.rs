use crate::types::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types_base::{DepthOptions, DiscoveryFilters, OutputOptions};

// ─── Pagination Types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaginationInput {
    /// Opaque cursor for next page (from previous response's next_cursor)
    pub cursor: Option<String>,
    /// Items per page (default: 50, max: 100)
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaginatedOutput<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub total: usize,
}

pub fn paginate<T: Clone>(items: &[T], cursor: Option<String>, page_size: u32) -> (Vec<T>, Option<String>) {
    let page_size = page_size.min(100) as usize;
    let start = cursor.and_then(|c| c.parse::<usize>().ok()).unwrap_or(0);
    let end = (start + page_size).min(items.len());
    let page = items[start..end].to_vec();
    let next_cursor = if end < items.len() { Some(end.to_string()) } else { None };
    (page, next_cursor)
}

// ─── Tool Guide Types ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ToolGuideInput {}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ToolGuideOutput {
    pub decision_tree: HashMap<String, String>,
    pub categories: HashMap<String, Vec<ToolGuideItem>>,
    pub drill_down_chains: Vec<DrillDownChain>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ToolGuideItem {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DrillDownChain {
    pub name: String,
    pub description: String,
    pub steps: Vec<String>,
}

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
    /// Pool IDs to filter by
    pub pools: Option<Vec<String>>,
    /// Active sources only (default: all)
    pub active_only: Option<bool>,
    #[serde(flatten)]
    pub pagination: PaginationInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceListOutput {
    pub sources: Vec<Source>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceUpsertInput {
    /// Source ID (auto-generated from name if omitted)
    pub id: Option<String>,
    /// Source name
    pub name: String,
    /// Source type (rss/generic_html/hackernews/youtube)
    #[serde(rename = "type")]
    pub source_type: String,
    /// Feed or webpage URL
    pub url: String,
    /// Custom HTTP headers
    pub headers: Option<HashMap<String, String>>,
    /// Parser key (see parsers.list)
    pub parser: Option<String>,
    /// Pool IDs for this source
    pub pools: Option<Vec<String>>,
    /// ISO country codes
    pub countries: Option<Vec<String>>,
    /// City names
    pub cities: Option<Vec<String>>,
    /// Domain tags (tech/cyber/defense/health)
    pub domains: Option<Vec<String>>,
    /// Active and fetched (default: true)
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParserInfo {
    pub key: String,
    pub note: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ParserListOutput {
    pub parsers: Vec<ParserInfo>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ParserListInput {
    #[serde(flatten)]
    pub pagination: PaginationInput,
}

// ─── Autodiscover Types ───────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AutodiscoverInput {
    /// Homepage URL to discover feeds from
    pub url: String,
    /// Pool IDs to assign discovered source to
    pub pools: Option<Vec<String>>,
    /// Name override for discovered source
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
    /// Source ID to enable scraping for
    pub id: String,
    /// Listing page URL (defaults to source URL)
    pub list_url: Option<String>,
    /// CSS selectors: item/title/link/date/desc
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
    pub pagination: PaginationInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CountryInfo {
    pub name: String,
    pub code: String,
    pub source_count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CountriesOutput {
    pub countries: Vec<CountryInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CityInfo {
    pub name: String,
    pub source_count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CitiesOutput {
    pub cities: Vec<CityInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
    /// Broad shallow scan across all pools
    pub discovery_mode: Option<bool>,
    /// Urgency level filter
    pub urgency: Option<String>,
    /// Skip NLP enrichment step
    pub skip_enrich: Option<bool>,
    /// Skip insight indexing step
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
    /// Source ID to test
    pub id: String,
    /// Cache mode: prefer/bypass/only
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
    /// Item ID
    pub id: String,
    /// Article title
    pub title: String,
    /// Article URL
    pub link: String,
    /// Publication date (ISO 8601)
    pub pub_date: String,
    /// Source name
    pub source_name: String,
    /// Pool ID
    pub pool_id: String,
    /// Content snippet
    pub content_snippet: Option<String>,
    /// Date confidence: high/medium/low
    pub date_confidence: Option<String>,
    /// Freshness score (0-100)
    pub freshness_score: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsEnrichInput {
    /// Items to enrich (from news.fetch)
    pub items: Vec<EnrichItemInput>,
    /// NLP features: topics/entities/sentiment/summary/diversity
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
    /// Search query
    pub query: String,
    /// Subreddits to search (omit for all)
    pub subreddits: Option<Vec<String>>,
    /// Sort: relevance/hot/top/new/comments
    pub sort: Option<String>,
    /// Time filter: hour/day/week/month/year/all
    pub time: Option<String>,
    /// Max results (default: 25)
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
    /// Subreddits without r/ prefix
    pub subreddits: Vec<String>,
    /// Per-subreddit limit (default: 25, max: 100)
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
    /// Search query
    pub query: String,
    /// Engines: arxiv/semanticscholar (default: both)
    pub sources: Option<Vec<String>>,
    /// arXiv categories (e.g. ["cs.AI", "cs.CL"])
    pub categories: Option<Vec<String>>,
    /// Earliest year (inclusive)
    pub year_from: Option<i32>,
    /// Latest year (inclusive)
    pub year_to: Option<i32>,
    /// Max results (default: 25)
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
    /// Paper ID (arxiv:XXXX.XXXXX or semanticscholar:XXXX)
    pub paper_id: String,
    /// Include citing papers
    pub include_citations: Option<bool>,
    /// Include referenced papers
    pub include_references: Option<bool>,
    /// Extract PDF text
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
    /// Paper ID (arxiv:XXXX.XXXXX or semanticscholar:XXXX)
    pub paper_id: String,
    /// Output file path (default: {paper_id}.pdf)
    pub output_path: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Generate markdown sidecar alongside PDF
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
    /// Provider: tavily (default) or firecrawl
    pub provider: Option<String>,
    /// Max results (default: 10, max: 20)
    pub max_results: Option<i32>,
    /// Topic: general (default) or news
    pub topic: Option<String>,
    /// Include domains
    pub include_domains: Option<Vec<String>>,
    /// Exclude domains
    pub exclude_domains: Option<Vec<String>>,
    /// Lookback period in days (news topic only)
    pub days: Option<i32>,
    /// Include AI answer summary
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
    /// URL to scrape (http/https)
    pub url: String,
    /// Provider: default (HTTP) or lightpanda (JS rendering)
    pub provider: Option<String>,
    /// Formats: markdown/html/text/screenshot (Lightpanda)
    pub formats: Option<Vec<String>>,
    /// Wait for CSS selector (Lightpanda)
    pub wait_selector: Option<String>,
    /// Strip mode: js/css/ui/full (Lightpanda)
    pub strip_mode: Option<String>,
    /// Extract structured data (Lightpanda)
    pub structured_data: Option<bool>,
    /// Include iframes (Lightpanda)
    pub include_frames: Option<bool>,
    /// Wait event: load/domcontentloaded/networkidle/done (Lightpanda)
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
    /// Starting URL (http/https)
    pub url: String,
    /// Provider: default or lightpanda (renders JS)
    pub provider: Option<String>,
    /// Max BFS depth (default: 2)
    pub max_depth: Option<i32>,
    /// Max pages (default: 20)
    pub max_pages: Option<i32>,
    /// Respect robots.txt (default: true)
    pub obey_robots: Option<bool>,
    /// Dump format: markdown/html/semantic_tree
    pub dump_format: Option<String>,
    /// Wait event: load/domcontentloaded/networkidle/done
    pub wait_until: Option<String>,
    /// Include iframes (Lightpanda)
    pub include_frames: Option<bool>,
    /// Wait for CSS selector (Lightpanda)
    pub wait_selector: Option<String>,
    /// Strip mode: js/css/ui/full (Lightpanda)
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
    /// Website URL (fetches /sitemap.xml)
    pub url: String,
    /// Provider: default or lightpanda
    pub provider: Option<String>,
    /// Max links (default: 100)
    pub limit: Option<i32>,
    /// Filter URLs by substring
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
    /// Entity name (omit for all cross-domain)
    pub entity: Option<String>,
    /// Min domains for connection (default: 2)
    pub min_domains: Option<i32>,
    /// Max results (default: 20, all-connections mode)
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
    /// Time window hours (default: 24)
    pub time_window_hours: Option<i64>,
    /// Min growth ratio (default: 2.0)
    pub min_growth: Option<f64>,
    /// Min current mentions (default: 3)
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
    /// Article ID
    pub id: String,
    /// Title
    pub title: String,
    /// Publication date (ISO 8601)
    pub pub_date: String,
    /// Source name
    pub source_name: String,
    /// Domains (omit for auto-detection)
    pub domains: Option<Vec<DomainInfo>>,
    /// Entities (omit for auto-extraction)
    pub entities: Option<Vec<EntityInfo>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightIndexInput {
    /// Articles to index (use enriched items for best results)
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

// ─── Security Types ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CveSearchInput {
    /// Search term (vulnerability name, CVE ID, product name)
    pub query: String,
    /// Severity filter: "LOW", "MEDIUM", "HIGH", "CRITICAL"
    pub severity: Option<String>,
    /// Days back to search (default: 30)
    pub days_back: Option<u32>,
    /// Max results (default: 20, max: 100)
    pub limit: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CveSearchOutput {
    pub query: String,
    pub total: usize,
    pub vulnerabilities: Vec<CveEntry>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CveEntry {
    pub id: String,
    pub source: String,
    pub published: String,
    pub description: String,
    pub severity: String,
    pub cvss_score: Option<f64>,
    pub affected_products: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SecurityAdvisoriesInput {
    /// Package ecosystem: "npm", "pip", "maven", "go", "rust"
    pub ecosystem: String,
    /// Severity filter: "low", "medium", "high", "critical"
    pub severity: Option<String>,
    /// Max results (default: 20, max: 100)
    pub limit: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SecurityAdvisoryOutput {
    pub ecosystem: String,
    pub total: usize,
    pub advisories: Vec<SecurityAdvisory>,
}

pub type SecurityAdvisoriesOutput = SecurityAdvisoryOutput;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SecurityAdvisory {
    pub ghsa_id: String,
    pub cve_id: Option<String>,
    pub summary: String,
    pub severity: String,
    pub published: String,
    pub updated: String,
    pub vulnerable_range: String,
    pub patched_versions: String,
    pub references: Vec<String>,
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

// ─── Weather Types ────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WeatherForecastInput {
    /// City name or "lat,lon"
    pub location: String,
    /// Forecast days (1-5, default 3)
    pub days: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WeatherForecastOutput {
    pub location: String,
    pub country: String,
    pub forecasts: Vec<WeatherDay>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WeatherDay {
    pub date: String,
    pub temp_high: f64,
    pub temp_low: f64,
    pub condition: String,
    pub description: String,
    pub humidity: u32,
    pub wind_speed: f64,
    pub precipitation_pct: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WeatherCurrentInput {
    pub location: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WeatherCurrentOutput {
    pub location: String,
    pub country: String,
    pub temp: f64,
    pub feels_like: f64,
    pub condition: String,
    pub description: String,
    pub humidity: u32,
    pub wind_speed: f64,
    pub visibility: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WeatherAlertsInput {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WeatherAlertsOutput {
    pub location: String,
    pub alerts: Vec<WeatherAlert>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WeatherAlert {
    pub sender: String,
    pub event: String,
    pub start: String,
    pub end: String,
    pub description: String,
}
