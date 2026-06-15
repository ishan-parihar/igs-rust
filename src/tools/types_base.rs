//! Base input types for shared fields across IGS tool categories.
//! Eliminates ~300 tokens of duplicated `format: Option<String>` across 15 structs.

use serde::{Deserialize, Serialize};

/// Base fields shared by all tools that produce output.
/// Each tool embeds this as a single field instead of repeating these params.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputOptions {
    /// Output format: "toon" (default, token-efficient) or "json" (standard)
    #[serde(default)]
    pub format: Option<String>,
}

/// Base fields shared by all news/source discovery tools.
/// Covers filtering, date ranges, geographic scoping, and content matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryFilters {
    /// Pool IDs to search (e.g. ["GLOBAL_TECH_CYBER"]). If empty, searches all pools.
    #[serde(default)]
    pub pools: Option<Vec<String>>,
    /// Source IDs to search (e.g. ["techcrunch", "bbc"]). Overrides pools.
    #[serde(default)]
    pub sources: Option<Vec<String>>,
    /// ISO 3166-1 alpha-2 country codes (e.g. ["US", "IN"]). 47 countries supported.
    #[serde(default)]
    pub countries: Option<Vec<String>>,
    /// City names (e.g. ["Delhi", "London"]).
    #[serde(default)]
    pub cities: Option<Vec<String>>,
    /// Domains to filter by (e.g. ["example.com"]).
    #[serde(default)]
    pub domains: Option<Vec<String>>,
    /// Start date (ISO 8601: "2024-01-01T00:00:00Z"). For date range filtering.
    #[serde(default)]
    pub start: Option<String>,
    /// End date (ISO 8601: "2024-12-31T23:59:59Z"). For date range filtering.
    #[serde(default)]
    pub end: Option<String>,
    /// Keywords for content matching. Inclusion matches are applied after fetch.
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    /// Keywords to exclude. Items matching any exclusion keyword are dropped.
    #[serde(default)]
    pub exclude_keywords: Option<Vec<String>>,
    /// If true, all keywords must match (AND logic). Default: false (OR logic).
    #[serde(default)]
    pub match_all: Option<bool>,
    /// Maximum items to return. Default: 20. Range: 1-500.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Cache mode: "fresh" (new only), "all" (fresh + cached), "only" (cached only).
    /// Default: "all" for most tools, "fresh" for intelligence.collect.
    #[serde(default)]
    pub cache_mode: Option<String>,
}

/// Base fields shared by crawl/depth-aware tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthOptions {
    /// Crawl depth for web crawling: "shallow" (default, 1 level), "medium" (2 levels),
    /// "deep" (3 levels, full BFS). Used by news.fetch and web.crawl.
    #[serde(default)]
    pub depth: Option<String>,
}

/// Format helper: extract format from OutputOptions or return default "toon".
pub fn resolve_format(opts: &OutputOptions) -> String {
    opts.format.clone().unwrap_or_else(|| "toon".to_string())
}

/// Format helper: resolve format from an Option<String> directly.
pub fn resolve_format_opt(format: &Option<String>) -> String {
    format.clone().unwrap_or_else(|| "toon".to_string())
}
