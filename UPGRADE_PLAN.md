# IGS Upgrade Plan: Lessons from last30days-skill

**Date**: 2026-06-15
**Reference**: `last30days-skill/` (cloned from `mvanhorn/last30days-skill`)
**Goal**: Port proven patterns from last30days-skill to enhance IGS capabilities

---

## Executive Summary

last30days-skill is an agent-powered news research tool with a sophisticated multi-phase pipeline, 16+ source adapters, and advanced fusion/reranking. IGS is a broader MCP intelligence platform with 411 sources but simpler processing. This plan identifies **12 upgrade opportunities** ranked by impact and feasibility.

---

## Critical Gap: Date Extraction in generic_html Parser

**Severity**: HIGH | **Effort**: LOW | **Impact**: HIGH

The `parse_generic_html` parser (parsers.rs:271) **never extracts dates** even when `parserConfig.selectors.date` is configured in YAML. The `Selectors.date` type field exists (types.rs:38) but is completely ignored. ~15 sources get `Utc::now()` as their pub_date, making time-based filtering unreliable.

**last30days approach**: 3-level date filtering with confidence tracking:
1. Source-level: APIs receive `from_date`/`to_date`
2. Post-fetch normalization: items outside range dropped
3. Freshness scoring: 0-100 based on recency

### Fix 1.1: Implement generic_html date extraction
```rust
// parsers.rs - in parse_generic_html, after content extraction:
if let Some(date_selector) = &config.parser_config.selectors.date {
    if let Ok(date_el) = doc.select(Selector::parse(date_selector).unwrap()).next() {
        let date_text = date_el.text().collect::<String>();
        // Try multiple parse patterns
        if let Ok(dt) = DateTime::parse_from_rfc3339(&date_text) {
            item.pub_date = dt.to_rfc3339();
        } else if let Ok(dt) = NaiveDateTime::parse_from_str(&date_text, "%Y-%m-%d %H:%M") {
            item.pub_date = Utc.from_utc_datetime(&dt).to_rfc3339();
        } else if let Ok(dt) = NaiveDate::parse_from_str(&date_text, "%Y-%m-%d") {
            item.pub_date = Utc.from_utc_datetime(&dt.and_hms(0, 0, 0)).to_rfc3339();
        }
        // Also check datetime attribute (e.g., <time datetime="...">)
        if let Some(datetime_attr) = date_el.value().attr("datetime") {
            if let Ok(dt) = DateTime::parse_from_rfc3339(datetime_attr) {
                item.pub_date = dt.to_rfc3339();
            }
        }
    }
}
```

### Fix 1.2: Add date_confidence field to NewsItem
```rust
// types.rs - add to NewsItem:
pub date_confidence: Option<String>, // "high", "medium", "low"
```
- **high**: Extracted from source (RFC3339, datetime attribute)
- **medium**: Parsed from text patterns
- **low**: Fallback to Utc::now()

### Fix 1.3: Wire up time.timezone config
The `time.timezone` setting in settings.yml exists but is never used. Apply timezone conversion when parsing dates from sources that provide naive datetimes.

---

## Upgrade 1: Multi-Level Date Filtering with Confidence

**Priority**: P0 (Critical) | **Effort**: Medium | **Files**: parsers.rs, types.rs, tools/news.rs

### Current State
- `filter_by_time()` exists and works (parsers.rs:701-736)
- But generic_html sources have wrong dates, making filtering unreliable
- No freshness scoring

### Target State (from last30days)
1. **Source-level filtering**: Pass `from_date`/`to_date` to APIs that support it
2. **Post-fetch normalization**: Drop items outside requested range
3. **Freshness scoring**: 0-100 score based on recency (configurable decay curve)
4. **Date confidence**: Track extraction quality

### Implementation
```rust
// types.rs - add to NewsItem:
pub freshness_score: Option<f64>, // 0.0 - 100.0

// parsers.rs - add freshness calculation:
pub fn calculate_freshness(pub_date: &str, reference_date: DateTime<Utc>) -> f64 {
    // Exponential decay: score = 100 * e^(-lambda * hours_old)
    // lambda configurable (default: 0.01 for ~4.3 day half-life)
    let parsed = DateTime::parse_from_rfc3339(pub_date).ok()?;
    let hours_old = (reference_date - parsed.with_timezone(&reference_date.timezone())).num_hours() as f64;
    let lambda = 0.01; // configurable
    Some(100.0 * (-lambda * hours_old).exp())
}
```

---

## Upgrade 2: Query Planning Layer (Phase 0)

**Priority**: P1 (High) | **Effort**: High | **Files**: new src/query_planner.rs

### Current State
IGS accepts direct `keywords` and `pools` parameters. No intelligent query decomposition.

### last30days Approach
```
Phase 0 - Planning:
  - LLM generates QueryPlan with intent, freshness_mode, 3-5 subqueries
  - Each subquery has weight (importance) and source preferences
  - Deterministic fallback when LLM unavailable
```

### Target for IGS
Add optional AI-powered query planning:
```rust
pub struct QueryPlan {
    pub intent: String,
    pub freshness_mode: FreshnessMode, // breaking, recent, timeless
    pub subqueries: Vec<Subquery>,
    pub source_preferences: HashMap<String, Vec<String>>, // source_type -> preferred
}

pub struct Subquery {
    pub query: String,
    pub weight: f64, // 0.0 - 1.0
    pub source_types: Vec<String>, // which parsers to prefer
    pub exclude_keywords: Vec<String>,
}
```

**Benefit**: Instead of one broad search, IGS could run multiple targeted searches with different source preferences, then fuse results. This is especially valuable for complex topics.

---

## Upgrade 3: Weighted RRF Fusion

**Priority**: P1 (High) | **Effort**: Medium | **Files**: new src/fusion.rs

### Current State
Simple dedup via Jaccard similarity (parsers.rs). No ranking fusion across sources.

### last30days Approach
```python
# fusion.py - Weighted Reciprocal Rank Fusion
def weighted_rrf(items, weights):
    scores = {}
    for item in items:
        source_weight = weights.get(item.source_type, 1.0)
        rank_score = 1.0 / (k + rank)  # k=60 standard
        scores[item.id] += source_weight * rank_score
    return sorted(scores.items(), key=lambda x: -x[1])
```

### Target for IGS
```rust
// src/fusion.rs - new module
pub fn weighted_rrf(
    result_lists: Vec<(Vec<NewsItem>, f64)>, // (items, source_weight)
    k: usize, // rank constant, default 60
) -> Vec<NewsItem> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut item_map: HashMap<String, NewsItem> = HashMap::new();

    for (items, weight) in result_lists {
        for (rank, item) in items.iter().enumerate() {
            let key = dedup_key(item);
            let rrf_score = 1.0 / (k + rank + 1) as f64;
            *scores.entry(key.clone()).or_insert(0.0) += weight * rrf_score;
            item_map.insert(key, item.clone());
        }
    }

    let mut scored: Vec<_> = scores.into_iter().collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    scored.into_iter()
        .filter_map(|(key, _)| item_map.remove(&key))
        .collect()
}
```

**Benefit**: When fetching from multiple pools/domains, results are properly ranked by relevance across all sources, not just concatenation.

---

## Upgrade 4: Entity-Based Cross-Source Clustering

**Priority**: P2 (Medium) | **Effort**: High | **Files**: new src/clustering.rs, insights/

### Current State
Insights engine tracks entities per-article and finds cross-domain connections. But articles about the same event from different sources aren't merged.

### last30days Approach
- Extract entities from each article
- Build entity co-occurrence graph
- Cluster articles with overlapping entities
- Present as "clusters" with source diversity score

### Target for IGS
After enrichment (NLP), cluster articles about the same event:
```rust
pub struct ArticleCluster {
    pub event_id: String,
    pub headline: String, // best headline from cluster
    pub articles: Vec<NewsItem>,
    pub source_domains: Vec<String>,
    pub entity_overlap_score: f64,
    pub freshness: f64, // best freshness score in cluster
}
```

**Benefit**: Instead of seeing 5 articles about the same earthquake from 5 sources, user sees one cluster with source diversity indicator. Matches how humans consume news.

---

## Upgrade 5: New Source Adapters (Social Media)

**Priority**: P1 (High) | **Effort**: Medium | **Files**: src/sources/

### Sources to Add (from last30days)

| Source | last30days Adapter | IGS Priority | Difficulty |
|--------|-------------------|--------------|------------|
| **Hacker News** | Algolia API | P0 (easy win) | Easy |
| **YouTube** | yt-dlp RSS | P1 | Easy |
| **Bluesky** | AT Protocol | P1 | Medium |
| **Polymarket** | REST API | P2 | Medium |
| **X/Twitter** | Multiple adapters | P2 | Hard (auth) |
| **GitHub** | REST API | P1 | Easy |

### Quick Win: Hacker News
```rust
// src/sources/hackernews.rs - new parser
// Uses Algolia API: https://hn.algolia.com/api/v1/
pub struct HackerNewsParser {
    client: reqwest::Client,
}

impl HackerNewsParser {
    pub async fn fetch(&self, query: &str, tags: &str) -> Result<Vec<NewsItem>> {
        let url = format!(
            "https://hn.algolia.com/api/v1/search?query={}&tags={}&hitsPerPage=50",
            urlencoding::encode(query), tags
        );
        let resp = self.client.get(&url).send().await?;
        let data: HnSearchResult = resp.json().await?;
        // Convert to NewsItem...
    }
}
```

### Quick Win: YouTube (RSS feeds)
```rust
// YouTube provides RSS feeds for channels/playlists
// https://www.youtube.com/feeds/videos.xml?channel_id=CHANNEL_ID
// No API key needed, parser is just RSS with special fields
```

### Quick Win: GitHub (events/releases)
```rust
// https://api.github.com/repos/{owner}/{repo}/releases
// https://api.github.com/search/repositories?q={query}
// Public API, no auth for basic usage
```

---

## Upgrade 6: Date Confidence Tracking

**Priority**: P1 (High) | **Effort**: Low | **Files**: types.rs, parsers.rs

### Current State
No visibility into date extraction quality. Sources with `Utc::now()` pollute time filtering.

### last30days Approach
```python
@dataclass
class DateResult:
    iso: str
    confidence: str  # "high", "medium", "low"
    source_hint: str  # "api", "html_attr", "text_parse", "fallback"
```

### Target for IGS
```rust
// types.rs
pub struct DateConfidence {
    pub iso: String,
    pub confidence: String, // "high", "medium", "low"
    pub source_hint: String, // "api", "html_attr", "text_parse", "fallback"
}

// In NewsItem:
pub date_confidence: Option<DateConfidence>,
```

**Benefit**: Consumers can filter by confidence level. "Show me only high-confidence results" avoids pollution from fallback dates.

---

## Upgrade 7: Per-Source Weight Configuration

**Priority**: P2 (Medium) | **Effort**: Low | **Files**: config/sources.yml, types.rs

### Current State
All sources treated equally in results.

### last30days Approach
Each source has a weight in the fusion algorithm. Premium sources get higher weight.

### Target for IGS
```yaml
# sources.yml - add weight field
sources:
  - id: reuters_rss
    name: Reuters
    url: "https://www.reuters.com/rssFeed/worldNews"
    weight: 1.5  # Premium source
    trust_score: 0.95

  - id: some_blog
    name: Random Blog
    url: "https://example.com/feed"
    weight: 0.5  # Lower priority
    trust_score: 0.6
```

```rust
// types.rs - Source struct
pub weight: Option<f64>, // default 1.0
pub trust_score: Option<f64>, // default 1.0
```

---

## Upgrade 8: Fun Judge / Virality Scoring

**Priority**: P3 (Low) | **Effort**: Medium | **Files**: new src/scoring.rs

### last30days Approach
Phase 5 - Fun Judge: Parallel LLM scoring for virality, wit, and engagement potential. Scores 0-100.

### Target for IGS (Optional)
Add optional scoring endpoint:
```rust
pub struct ViralityScore {
    pub score: f64, // 0-100
    pub factors: Vec<String>, // "trending_on_social", "unusual_angle", etc.
}
```

**Benefit**: Users can sort by "most viral" or filter for high-engagement stories.

---

## Upgrade 9: Cross-Source Cluster Merging

**Priority**: P2 (Medium) | **Effort**: High | **Files**: new src/clustering.rs

(Detailed in Upgrade 4 above)

---

## Upgrade 10: Source Diversity Indicator

**Priority**: P2 (Medium) | **Effort**: Low | **Files**: types.rs, tools/news.rs

### Target for IGS
```rust
// In NewsItem or output:
pub source_diversity: Option<f64>, // 0.0 = single source, 1.0 = many sources covering same story
pub cross_references: Vec<String>, // other source_ids covering same event
```

**Benefit**: Quick visual indicator of story importance (widely covered = more important).

---

## Upgrade 11: Depth Settings (quick/default/deep)

**Priority**: P2 (Medium) | **Effort**: Low | **Files**: tools/types.rs, tools/news.rs

### last30days Approach
```python
DEPTH_CONFIGS = {
    "quick": {"max_streams": 2, "max_items_per_stream": 5},
    "default": {"max_streams": 5, "max_items_per_stream": 10},
    "deep": {"max_streams": 10, "max_items_per_stream": 20},
}
```

### Target for IGS
```rust
// In NewsFetchInput:
pub depth: Option<String>, // "quick", "default", "deep"

// Map to pool limits:
fn get_depth_config(depth: &str) -> DepthConfig {
    match depth {
        "quick" => DepthConfig { max_pools: 2, max_per_pool: 5 },
        "deep" => DepthConfig { max_pools: 20, max_per_pool: 50 },
        _ => DepthConfig { max_pools: 5, max_per_pool: 20 },
    }
}
```

---

## Upgrade 12: Author Dedup Cap

**Priority**: P3 (Low) | **Effort**: Low | **Files**: parsers.rs

### last30days Approach
Max 3 items per author to prevent single-source flooding.

### Target for IGS
```rust
// In filter_dedup:
pub fn per_author_cap(items: &mut Vec<NewsItem>, max_per_author: usize) {
    let mut author_counts: HashMap<String, usize> = HashMap::new();
    items.retain(|item| {
        let author = item.author.as_deref().unwrap_or("unknown");
        let count = author_counts.entry(author.to_string()).or_insert(0);
        *count += 1;
        *count <= max_per_author
    });
}
```

---

## Implementation Roadmap

### Phase 1: Foundation (Week 1-2)
- [ ] Fix generic_html date extraction (Fix 1.1)
- [ ] Add date_confidence to NewsItem (Fix 1.2)
- [ ] Wire up time.timezone config (Fix 1.3)
- [ ] Add freshness_score calculation
- [ ] Add depth parameter to news.fetch

**Impact**: Dates become reliable, time filtering works correctly for all sources.

### Phase 2: Source Expansion (Week 3-4)
- [ ] Add Hacker News adapter (Algolia API)
- [ ] Add YouTube RSS adapter
- [ ] Add GitHub releases/search adapter
- [ ] Add Bluesky adapter (AT Protocol)

**Impact**: 4 new source types, ~200+ new potential sources.

### Phase 3: Intelligence (Week 5-6)
- [ ] Implement weighted RRF fusion (Upgrade 3)
- [ ] Add per-source weight configuration (Upgrade 7)
- [ ] Add source diversity indicator (Upgrade 10)
- [ ] Add per-author cap (Upgrade 12)

**Impact**: Better ranking, more intelligent result presentation.

### Phase 4: Advanced (Week 7-8)
- [ ] Query planning layer (Upgrade 2)
- [ ] Entity-based clustering (Upgrade 4/9)
- [ ] Virality scoring (Upgrade 8, optional)

**Impact**: AI-powered intelligence, cross-source event detection.

---

## Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `src/fusion.rs` | Weighted RRF fusion algorithm |
| `src/clustering.rs` | Entity-based article clustering |
| `src/sources/hackernews.rs` | Hacker News adapter |
| `src/sources/youtube.rs` | YouTube RSS adapter |
| `src/sources/github.rs` | GitHub adapter |
| `src/sources/bluesky.rs` | Bluesky adapter |

### Modified Files
| File | Changes |
|------|---------|
| `src/types.rs` | Add DateConfidence, freshness_score, weight, depth |
| `src/parsers.rs` | Fix generic_html date extraction, add freshness calc |
| `src/tools/news.rs` | Wire depth parameter, source weights |
| `src/tools/types.rs` | Add depth field to NewsFetchInput |
| `config/sources.yml` | Add weight, trust_score fields |
| `config/settings.yml` | Document timezone usage |

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Breaking existing parsers | Add date extraction as fallback, never override existing behavior |
| Performance impact of clustering | Make optional via `cluster: true` parameter |
| New source API changes | Use established APIs (HN Algolia, GitHub REST) with fallbacks |
| Fusion algorithm complexity | Start with simple RRF, tune parameters over time |

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Sources with reliable dates | ~60% | 95%+ |
| Source types | 4 | 8+ |
| Cross-source clustering | None | 80% accuracy |
| Query planning | None | 3-5x more relevant results |
