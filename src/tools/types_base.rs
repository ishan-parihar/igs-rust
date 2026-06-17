//! Base input types for shared fields across IGS tool categories.
//! Eliminates ~300 tokens of duplicated `format: Option<String>` across 15 structs.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Flexible keyword filter that supports string, array, or nested array formats.
/// Use Single("AI safety") for one keyword, Multiple(["AI", "safety"]) for AND-style flat list,
/// or Nested([["AI","safety"], ["ML"]]) for OR-group clusters.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum KeywordFilter {
    /// Single keyword string
    Single(String),
    /// Flat array of keywords (AND logic within the group)
    Multiple(Vec<String>),
    /// Nested arrays for OR-group logic (AND within inner, OR across outer)
    Nested(Vec<Vec<String>>),
}

/// Base fields shared by all tools that produce output.
/// Each tool embeds this as a single field instead of repeating these params.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OutputOptions {
    /// Output format: "toon" (default, token-efficient) or "json" (standard)
    pub format: Option<String>,
}

/// Base fields shared by all news/source discovery tools.
/// Covers filtering, date ranges, geographic scoping, and content matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiscoveryFilters {
    /// Pool IDs to search (e.g. ["GLOBAL_TECH_CYBER"]). If empty, searches all pools.
    pub pools: Option<Vec<String>>,
    /// Source IDs to search (e.g. ["techcrunch", "bbc"]). Overrides pools.
    pub sources: Option<Vec<String>>,
    /// ISO 3166-1 alpha-2 country codes (e.g. ["US", "IN"]). 47 countries supported.
    pub countries: Option<Vec<String>>,
    /// City names (e.g. ["Delhi", "London"]).
    pub cities: Option<Vec<String>>,
    /// Domains to filter by (e.g. ["example.com"]).
    pub domains: Option<Vec<String>>,
    /// Start date (ISO 8601: "2024-01-01T00:00:00Z"). For date range filtering.
    pub start: Option<String>,
    /// End date (ISO 8601: "2024-12-31T23:59:59Z"). For date range filtering.
    pub end: Option<String>,
    /// Keywords for content matching. Accepts string, array, or array-of-arrays for clusters.
    /// See [`KeywordFilter`] for accepted formats.
    pub keywords: Option<KeywordFilter>,
    /// Keywords to exclude. Items matching any exclusion keyword are dropped.
    pub exclude_keywords: Option<Vec<String>>,
    /// If true, all keywords must match (AND logic). Default: false (OR logic).
    pub match_all: Option<bool>,
    /// Maximum items to return. Default: 20. Range: 1-500.
    pub limit: Option<i32>,
    /// Cache mode: "fresh" (new only), "all" (fresh + cached), "only" (cached only).
    /// Default: "all" for most tools.
    pub cache_mode: Option<String>,
}

/// Base fields shared by crawl/depth-aware tools.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DepthOptions {
    /// Crawl depth for web crawling: "shallow" (default, 1 level), "medium" (2 levels),
    /// "deep" (3 levels, full BFS). Used by news.fetch and web.crawl.
    pub depth: Option<String>,
}

/// Format helper: extract format from OutputOptions or return default "toon".
pub fn resolve_format(opts: &OutputOptions) -> String {
    opts.format.as_deref().unwrap_or("toon").to_string()
}

/// Format helper: resolve format from an Option<String> directly.
pub fn resolve_format_opt(format: &Option<String>) -> String {
    format.as_deref().unwrap_or("toon").to_string()
}
