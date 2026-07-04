use crate::types::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types_base::{DepthOptions, DiscoveryFilters, OutputOptions};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaginatedOutput<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub total: usize,
}

pub fn paginate<T: Clone>(
    items: &[T],
    cursor: Option<String>,
    page_size: u32,
) -> (Vec<T>, Option<String>) {
    let page_size = page_size.min(100) as usize;
    let start = cursor.and_then(|c| c.parse::<usize>().ok()).unwrap_or(0);
    let end = (start + page_size).min(items.len());
    let page = items[start..end].to_vec();
    let next_cursor = if end < items.len() {
        Some(end.to_string())
    } else {
        None
    };
    (page, next_cursor)
}

// ─── Limit Types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LimitInput {
    /// Max results (default: 20, max: 100)
    #[serde(default)]
    pub limit: Option<u32>,
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
    /// Pool ID
    pub id: String,
    /// Pool name
    pub name: String,
    /// Pool description
    pub description: Option<String>,
    /// Active (default: true)
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolUpsertOutput {
    pub updated: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoolDeleteInput {
    /// Pool ID to delete
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
    /// Active only (default: all)
    pub active_only: Option<bool>,
    /// Cursor for next page
    pub cursor: Option<String>,
    /// Items per page (default: 50, max: 100)
    pub page_size: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceListOutput {
    pub sources: Vec<Source>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceUpsertInput {
    /// Source ID (auto from name)
    pub id: Option<String>,
    /// Source name
    pub name: String,
    /// Source type
    #[serde(rename = "type")]
    pub source_type: String,
    /// Feed URL
    pub url: String,
    /// Custom headers
    pub headers: Option<HashMap<String, String>>,
    /// Parser key
    pub parser: Option<String>,
    /// Pool IDs for source
    pub pools: Option<Vec<String>>,
    /// Country codes
    pub countries: Option<Vec<String>>,
    /// City names
    pub cities: Option<Vec<String>>,
    /// Domain tags
    pub domains: Option<Vec<String>>,
    /// Active (default: true)
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceUpsertOutput {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceDeleteInput {
    /// Source ID to delete
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
    /// Cursor for next page
    pub cursor: Option<String>,
    /// Items per page (default: 50, max: 100)
    pub page_size: Option<u32>,
}

// ─── Autodiscover Types ───────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AutodiscoverInput {
    /// Homepage URL
    pub url: String,
    /// Pool IDs for source
    pub pools: Option<Vec<String>>,
    /// Name override
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
    /// Source ID
    pub id: String,
    /// Listing page URL
    pub list_url: Option<String>,
    /// CSS selectors
    pub selectors: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnableScraperOutput {
    pub updated: bool,
}

// ─── Country/City/Domain Types ────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GeoListInput {
    /// Cursor for next page
    pub cursor: Option<String>,
    /// Items per page (default: 50, max: 100)
    pub page_size: Option<u32>,
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
    /// Discovery mode
    pub discovery_mode: Option<bool>,
    /// Urgency filter
    pub urgency: Option<String>,
    /// Skip enrichment
    pub skip_enrich: Option<bool>,
    /// Skip indexing
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
    /// Number of sources that were skipped because their `platform` is
    /// "reddit" or "twitter" — these are fetched via dedicated `reddit.*`
    /// and `twitter.*` tools, not via `news.fetch`. Previously counted as
    /// "succeeded", which inflated the success metric.
    pub sources_short_circuited: usize,
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
    /// Source ID
    pub id: String,
    /// Cache mode
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
    /// Pub date
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
    /// Items to enrich
    pub items: Vec<EnrichItemInput>,
    /// NLP features
    pub extract: Option<Vec<String>>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EnrichedItem {
    /// Original item data
    #[serde(flatten)]
    pub item: serde_json::Value,
    /// Topics
    #[serde(default)]
    pub topics: Vec<String>,
    /// Entities
    #[serde(default)]
    pub entities: Vec<EntityInfo>,
    /// Sentiment
    #[serde(default)]
    pub sentiment: Option<SentimentResult>,
    /// Summary
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EnrichmentMeta {
    /// Enriched count
    pub enriched_count: usize,
    /// NLP features applied
    pub features: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NewsEnrichOutput {
    /// Enriched items
    pub items: Vec<EnrichedItem>,
    /// Enrichment metadata
    pub meta: EnrichmentMeta,
}

// ─── Reddit Search Types ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RedditSearchInput {
    /// Search query
    pub query: String,
    /// Subreddits (omit for all)
    pub subreddits: Option<Vec<String>>,
    /// Sort order
    pub sort: Option<String>,
    /// Time filter
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
    /// Subreddits (no r/)
    pub subreddits: Vec<String>,
    /// Per-sub limit (25-100)
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
    /// Engines (default: both)
    pub sources: Option<Vec<String>>,
    /// arXiv categories
    pub categories: Option<Vec<String>>,
    /// Earliest year
    pub year_from: Option<i32>,
    /// Latest year
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
    /// Paper ID
    pub paper_id: String,
    /// Include citations
    pub include_citations: Option<bool>,
    /// Include references
    pub include_references: Option<bool>,
    /// Extract PDF
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
    /// Paper ID
    pub paper_id: String,
    /// Output file path
    pub output_path: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
    /// Generate markdown
    pub convert_to_markdown: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaperMetadata {
    /// Paper title
    pub title: String,
    /// Paper ID (e.g., "arxiv:2301.00001")
    pub id: String,
    /// Year
    pub year: Option<u32>,
    /// Pages
    pub pages: Option<u32>,
    /// File size
    pub file_size: u64,
    /// File path
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
    /// Search query
    pub query: String,
    /// Provider (default: tavily)
    pub provider: Option<String>,
    /// Max results (10-20)
    pub max_results: Option<i32>,
    /// Topic (general|news)
    pub topic: Option<String>,
    /// Include domains
    pub include_domains: Option<Vec<String>>,
    /// Exclude domains
    pub exclude_domains: Option<Vec<String>>,
    /// Days back (news only)
    pub days: Option<i32>,
    /// Include answer
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
    /// URL to scrape
    pub url: String,
    /// Provider
    pub provider: Option<String>,
    /// Output formats
    pub formats: Option<Vec<String>>,
    /// Wait for CSS selector (Lightpanda)
    pub wait_selector: Option<String>,
    /// Strip mode
    pub strip_mode: Option<String>,
    /// Extract structured data (Lightpanda)
    pub structured_data: Option<bool>,
    /// Include iframes (Lightpanda)
    pub include_frames: Option<bool>,
    /// Wait event
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScrapeStructuredData {
    pub description: Option<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub links_count: usize,
    pub headings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScrapeMeta {
    /// Final URL
    pub url: String,
    /// Status code
    pub status: u16,
    /// Content type
    pub content_type: Option<String>,
    /// Elapsed ms
    pub elapsed_ms: u64,
    /// JS rendered
    pub js_rendered: bool,
}

// ─── Web Crawl Types ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WebCrawlInput {
    /// Starting URL
    pub url: String,
    /// Provider
    pub provider: Option<String>,
    /// Max BFS depth (default: 2)
    pub max_depth: Option<i32>,
    /// Max pages (default: 20)
    pub max_pages: Option<i32>,
    /// Respect robots.txt
    pub obey_robots: Option<bool>,
    /// Dump format
    pub dump_format: Option<String>,
    /// Wait event
    pub wait_until: Option<String>,
    /// Include iframes (Lightpanda)
    pub include_frames: Option<bool>,
    /// Wait for CSS selector (Lightpanda)
    pub wait_selector: Option<String>,
    /// Strip mode
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
    /// Website URL
    pub url: String,
    /// Provider: default or obscura
    pub provider: Option<String>,
    /// Max links (default: 100)
    pub limit: Option<i32>,
    /// Filter by substring
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
    /// Entity name
    pub entity: Option<String>,
    /// Min domains (default: 2)
    pub min_domains: Option<i32>,
    /// Max results (default: 20)
    pub limit: Option<i32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightFindConnectionsOutput {
    pub connections: Vec<EntityConnection>,
    pub count: usize,
    /// All-connections only
    pub total_found: Option<usize>,
    /// All-connections only
    pub stats: Option<InsightStats>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightTrendingInput {
    /// Time window (default: 24h)
    pub time_window_hours: Option<i64>,
    /// Min growth (default: 2.0)
    pub min_growth: Option<f64>,
    /// Min mentions (default: 3)
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
    /// Pub date
    pub pub_date: String,
    /// Source name
    pub source_name: String,
    /// Domains (auto-detect)
    pub domains: Option<Vec<DomainInfo>>,
    /// Entities (auto-extract)
    pub entities: Option<Vec<EntityInfo>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct InsightIndexInput {
    /// Articles to index
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
    /// Search term
    pub query: String,
    /// Severity filter
    pub severity: Option<String>,
    /// Days back (default: 30)
    pub days_back: Option<u32>,
    #[serde(flatten)]
    pub limits: LimitInput,
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
    /// Package ecosystem
    pub ecosystem: String,
    /// Severity filter
    pub severity: Option<String>,
    #[serde(flatten)]
    pub limits: LimitInput,
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
    /// URL
    pub url: String,
    /// Wait event
    pub wait_until: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpMarkdownInput {
    /// Strip mode
    pub strip_mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpLinksInput {
    /// Selector
    pub selector: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpEvaluateInput {
    /// Expression
    pub expression: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpClickInput {
    /// Selector
    pub selector: String,
    /// Wait for nav
    pub wait_for_navigation: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpFillInput {
    /// Selector
    pub selector: String,
    /// Value
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpScrollInput {
    /// Direction
    pub direction: Option<String>,
    /// Pixels
    pub pixels: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpWaitForSelectorInput {
    /// Selector
    pub selector: String,
    /// Timeout (ms)
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BrowserMeta {
    /// Current URL
    pub url: String,
    /// Page title
    pub title: Option<String>,
    /// Operation type
    pub operation: String,
    /// Elapsed ms
    pub elapsed_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LpToolOutput {
    pub success: bool,
    pub content: String,
    pub meta: BrowserMeta,
}

// ─── Weather Types ────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WeatherForecastInput {
    /// Location
    pub location: String,
    /// Forecast days (1-5)
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

// ─── Finance Types ─────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FinanceMarketInput {
    /// Stock symbols
    pub symbols: Vec<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FinanceMarketOutput {
    pub quotes: Vec<MarketQuote>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MarketQuote {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub volume: u64,
    pub market_cap: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FinanceCryptoInput {
    /// CoinGecko IDs
    pub symbols: Vec<String>,
    /// CoinGecko IDs (override)
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FinanceCryptoOutput {
    pub prices: Vec<CryptoPrice>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CryptoPrice {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub price_usd: f64,
    pub change_24h_pct: f64,
    pub market_cap: u64,
    pub volume_24h: u64,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FinanceTrendingInput {
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FinanceTrendingOutput {
    pub trending: Vec<TrendingCoin>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TrendingCoin {
    pub name: String,
    pub symbol: String,
    pub market_cap_rank: u32,
    pub score: f64,
}

// ─── Patent Types ─────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PatentSearchInput {
    /// Search query
    pub query: String,
    /// Patent office
    pub office: Option<String>,
    /// Years back (default: 5)
    pub years_back: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PatentSearchOutput {
    pub query: String,
    pub office: String,
    pub total: usize,
    pub patents: Vec<PatentEntry>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PatentEntry {
    pub id: String,
    pub title: String,
    pub date: String,
    pub abstract_text: String,
    pub office: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PatentDetailsInput {
    /// Patent ID
    pub patent_id: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PatentDetailsOutput {
    pub id: String,
    pub title: String,
    pub date: String,
    pub abstract_text: String,
    pub claims: u32,
    pub url: String,
}

// ─── Government Types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GovtBillsInput {
    /// Search query
    pub query: String,
    /// Congress number
    pub congress: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GovtBillsOutput {
    pub query: String,
    pub congress: u32,
    pub total: usize,
    pub bills: Vec<BillEntry>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct BillEntry {
    pub number: u32,
    pub title: String,
    pub sponsor: String,
    pub introduced_date: String,
    pub latest_action: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GovtRegulationsInput {
    /// Search query
    pub query: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GovtRegulationsOutput {
    pub query: String,
    pub total: usize,
    pub regulations: Vec<RegulationEntry>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RegulationEntry {
    pub document_number: String,
    pub title: String,
    pub abstract_text: String,
    pub publication_date: String,
    pub agency: String,
    pub url: String,
}

// ─── SOP Types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SopStep {
    /// Tool name
    pub tool: String,
    /// Tool params
    pub params: serde_json::Value,
    /// Depends on step
    pub depends_on: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SopChain {
    /// Chain name
    pub name: String,
    /// Description
    pub description: String,
    /// Steps
    pub steps: Vec<SopStep>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SopListInput {
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SopListOutput {
    pub chains: Vec<SopChainInfo>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SopChainInfo {
    pub name: String,
    pub description: String,
    pub step_count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SopExecuteInput {
    /// Chain name
    pub chain_name: String,
    /// Query to substitute for $QUERY placeholder in chain steps
    pub query: Option<String>,
    /// Target URL to substitute for $TARGET_URL placeholder
    pub target_url: Option<String>,
    /// Country code to substitute for $COUNTRY placeholder
    pub country: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SopExecuteOutput {
    pub chain_name: String,
    pub steps_completed: usize,
    pub results: Vec<SopStepResult>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SopStepResult {
    pub step: usize,
    pub tool: String,
    pub status: String,
    pub output: String,
}

// ─── PubMed Types ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchPubMedInput {
    /// Search query
    pub query: String,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchPubMedOutput {
    pub query: String,
    pub total: usize,
    pub papers: Vec<ResearchPubMedPaper>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResearchPubMedPaper {
    pub pmid: String,
    pub title: String,
    pub authors: Vec<String>,
    pub journal: String,
    pub pub_date: String,
    pub url: String,
}

// ─── Health Types ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HealthCdcInput {
    /// State name
    pub state: Option<String>,
    /// Year (default: 2021)
    pub year: Option<u32>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HealthCdcOutput {
    pub query: String,
    pub total: usize,
    pub causes: Vec<HealthCause>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HealthCause {
    pub cause: String,
    pub state: String,
    pub year: String,
    pub deaths: u64,
    pub age_adjusted_rate: String,
}

// ─── Politics Types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoliticsFecInput {
    /// Candidate name
    pub name: String,
    /// Office filter
    pub office: Option<String>,
    /// Party filter
    pub party: Option<String>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoliticsFecOutput {
    pub query: String,
    pub total: usize,
    pub candidates: Vec<FecCandidate>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FecCandidate {
    pub id: String,
    pub name: String,
    pub party: String,
    pub office: String,
    pub state: String,
    pub total_receipts: f64,
    pub total_disbursements: f64,
    pub cash_on_hand: f64,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoliticsFecCommitteesInput {
    /// Committee name
    pub name: String,
    /// Committee type
    pub committee_type: Option<String>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoliticsFecCommitteesOutput {
    pub query: String,
    pub total: usize,
    pub committees: Vec<FecCommittee>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FecCommittee {
    pub id: String,
    pub name: String,
    pub committee_type: String,
    pub party: String,
    pub state: String,
    pub total_receipts: f64,
    pub total_disbursements: f64,
}

// ─── Satellite Types ────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SatelliteFirmsInput {
    /// West longitude
    pub west: f64,
    /// South latitude
    pub south: f64,
    /// East longitude
    pub east: f64,
    /// North latitude
    pub north: f64,
    /// Data source
    pub source: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SatelliteFirmsOutput {
    pub query: String,
    pub source: String,
    pub total: usize,
    pub hotspots: Vec<FireHotspot>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct FireHotspot {
    pub latitude: f64,
    pub longitude: f64,
    pub bright_ti4: f64,
    pub scan: f64,
    pub track: f64,
    pub acq_date: String,
    pub acq_time: String,
    pub satellite: String,
    pub confidence: String,
    pub frp: f64,
    pub daynight: String,
}

// ─── Environment Types ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnvEpaFacilitiesInput {
    /// State code
    pub state: Option<String>,
    /// Facility name
    pub name: Option<String>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnvEpaFacilitiesOutput {
    pub query: String,
    pub total: usize,
    pub facilities: Vec<EpaFacility>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EpaFacility {
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub zip: String,
    pub county: String,
    pub latitude: f64,
    pub longitude: f64,
    pub registry_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnvEpaEmissionsInput {
    /// State code
    pub state: Option<String>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EnvEpaEmissionsOutput {
    pub query: String,
    pub total: usize,
    pub facilities: Vec<EpaEmissionsFacility>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EpaEmissionsFacility {
    pub name: String,
    pub state: String,
    pub county: String,
    pub latitude: f64,
    pub longitude: f64,
    pub registry_id: String,
}

// ─── Legal Types ────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LegalSearchInput {
    /// Search query
    pub query: String,
    /// Court filter
    pub court: Option<String>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LegalSearchOutput {
    pub query: String,
    pub total: usize,
    pub cases: Vec<LegalCase>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LegalCase {
    pub id: u32,
    pub case_name: String,
    pub court: String,
    pub date_filed: String,
    pub citation: u64,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LegalCaseDetailsInput {
    /// Case ID
    pub case_id: u32,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LegalCaseDetailsOutput {
    pub id: u32,
    pub case_name: String,
    pub court: String,
    pub date_filed: String,
    pub date_terminated: String,
    pub judges: Vec<String>,
    pub nature_of_suit: String,
    pub url: String,
}

// ─── Climate Types ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClimateNoaaInput {
    /// Dataset
    pub dataset: Option<String>,
    /// Location ID
    pub location: Option<String>,
    /// Station ID
    pub station: Option<String>,
    /// Start date
    pub start_date: Option<String>,
    /// End date
    pub end_date: Option<String>,
    /// Max results (20-1000)
    pub limit: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClimateNoaaOutput {
    pub query: String,
    pub total: usize,
    pub observations: Vec<NoaaObservation>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NoaaObservation {
    pub date: String,
    pub station: String,
    pub datatype: String,
    pub value: f64,
    pub attributes: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClimateNoaaStationsInput {
    /// Location ID
    pub location: Option<String>,
    /// Max results (20-1000)
    pub limit: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClimateNoaaStationsOutput {
    pub query: String,
    pub total: usize,
    pub stations: Vec<NoaaStation>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct NoaaStation {
    pub id: String,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: f64,
    pub mindate: String,
    pub maxdate: String,
    pub datacoverage: f64,
}

// ─── WHO Types ──────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HealthWhoInput {
    /// WHO indicator
    pub indicator: Option<String>,
    /// Country code
    pub country: Option<String>,
    /// Year filter
    pub year: Option<u32>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct HealthWhoOutput {
    pub query: String,
    pub total: usize,
    pub observations: Vec<WhoObservation>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WhoObservation {
    pub indicator: String,
    pub country: String,
    pub year: u32,
    pub value: f64,
    pub low: f64,
    pub high: f64,
}

// ─── YouTube Types ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct YoutubeSearchInput {
    /// Search query
    pub query: String,
    /// Max results (default: 10, max: 50)
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct YoutubeVideo {
    pub id: String,
    pub title: String,
    pub url: String,
    pub channel: String,
    pub duration: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct YoutubeSearchOutput {
    pub videos: Vec<YoutubeVideo>,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct YoutubeMetadataInput {
    /// Video URL
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct YoutubeMetadataOutput {
    pub title: String,
    pub description: String,
    pub channel: String,
    pub duration: Option<String>,
    pub views: Option<u64>,
    pub likes: Option<u64>,
    pub upload_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct YoutubeSubtitlesInput {
    /// Video URL
    pub url: String,
    /// Subtitle language (default: en)
    pub language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct YoutubeSubtitlesOutput {
    pub subtitles: String,
    pub language: String,
}

// ─── Twitter Types ─────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TwitterSearchInput {
    /// Search query
    pub query: String,
    /// Max results (default: 10)
    pub limit: Option<u32>,
    /// Search mode: Top, Latest, Photos, Videos, Users (default: Latest)
    pub search_mode: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TwitterSearchOutput {
    pub tweets: Vec<TwitterTweet>,
    pub count: usize,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TwitterTweet {
    pub id: String,
    pub text: String,
    pub author: String,
    pub username: String,
    pub created_at: String,
    pub url: String,
    pub likes: Option<i32>,
    pub retweets: Option<i32>,
    pub replies: Option<i32>,
    pub views: Option<i32>,
    pub is_retweet: bool,
    pub is_reply: bool,
    pub hashtags: Vec<String>,
    pub urls: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TwitterReadInput {
    /// Tweet URL or ID
    pub url: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TwitterReadOutput {
    pub tweet: TwitterTweet,
}

// ─── Monitor Tool Types ───────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MonitorCreateInput {
    /// Unique monitor ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Pool IDs to monitor
    pub pools: Vec<String>,
    /// Keywords to watch for
    pub keywords: Vec<String>,
    /// Poll interval in seconds (default: 300 = 5 min, minimum: 30)
    pub interval_secs: Option<u64>,
    /// Alert threshold: min keyword matches (default: 1)
    pub threshold: Option<u32>,
    /// Webhook URL for alerts (Slack, Discord, or custom HTTP endpoint)
    pub webhook_url: Option<String>,
    /// Webhook format: "slack" (default), "discord", "teams", "raw"
    pub webhook_format: Option<String>,
    /// File path to append alerts to
    pub alert_file: Option<String>,
    /// Telegram bot token (from @BotFather)
    pub telegram_bot_token: Option<String>,
    /// Telegram chat ID to send alerts to
    pub telegram_chat_id: Option<String>,
    /// Email webhook URL (HTTP endpoint that sends emails)
    pub email_webhook_url: Option<String>,
    /// Email recipients
    pub email_recipients: Option<Vec<String>>,
    /// Cooldown in seconds between alerts (default: 300 = 5 min)
    pub cooldown_secs: Option<u64>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MonitorCreateOutput {
    pub created: bool,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MonitorListInput {
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MonitorInfo {
    pub id: String,
    pub name: String,
    pub pools: Vec<String>,
    pub keywords: Vec<String>,
    pub interval_secs: u64,
    pub threshold: u32,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MonitorListOutput {
    pub monitors: Vec<MonitorInfo>,
    pub count: usize,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MonitorDeleteInput {
    pub id: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MonitorDeleteOutput {
    pub removed: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MonitorPauseInput {
    pub id: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct MonitorPauseOutput {
    pub paused: bool,
}

// ─── Summarize Tool Types ─────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SummarizeInput {
    /// Text to summarize
    pub text: String,
    /// Number of sentences in the summary (default: 3)
    pub num_sentences: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SummarizeOutput {
    pub summary: String,
    pub sentence_count: usize,
    pub original_count: usize,
    pub top_sentences: Vec<String>,
}

// ─── Semantic Search Tool Types ───────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SemanticSearchInput {
    /// Search query
    pub query: String,
    /// Max results (default: 20)
    pub limit: Option<u32>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SemanticSearchResultItem {
    pub article_id: String,
    pub title: String,
    pub link: String,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SemanticSearchResultOutput {
    pub query: String,
    pub results: Vec<SemanticSearchResultItem>,
    pub count: usize,
}

// ─── Entity Resolution Tool Types ─────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EntityResolveInput {
    /// Entity names to resolve (JSON array of strings)
    pub names: Vec<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EntityResolveOutput {
    pub entities: Vec<crate::tools::entity_resolution::ResolvedEntity>,
    pub count: usize,
}

// ─── GDELT Tool Types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GdeltInput {
    /// Search query
    pub query: String,
    /// Max results (default: 50, max: 250)
    pub limit: Option<u32>,
    /// Start date (YYYYMMDD format)
    pub start_date: Option<String>,
    /// End date (YYYYMMDD format)
    pub end_date: Option<String>,
    #[serde(flatten)]
    pub limits: LimitInput,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GdeltOutput {
    pub query: String,
    pub total: usize,
    pub events: Vec<crate::tools::gdelt::GdeltEvent>,
}

// ─── Advanced Intelligence Tool Types (P2) ────────────────────

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TemporalAnalysisInput {
    /// Entity name to analyze
    pub entity: String,
    /// Time-series points as JSON: [{"timestamp":"2026-01-01","count":10}, ...]
    pub points_json: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GeoExtractionInput {
    /// Text to extract locations from
    pub text: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LanguageDetectionInput {
    /// Text to detect language of
    pub text: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SourceQualityInput {
    /// Sources as JSON: [{"name":"Reuters","domain":"reuters.com"}, ...]
    pub sources_json: String,
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ReportGenerateInput {
    /// Report title
    pub title: String,
    /// Articles as JSON array
    pub articles_json: String,
    /// Summary style: "brief", "detailed", or "bullet"
    pub style: Option<String>,
    #[serde(flatten)]
    pub output: OutputOptions,
}
