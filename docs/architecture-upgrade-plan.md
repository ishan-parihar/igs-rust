# IGS Architecture Upgrade Plan

> **Document Version:** 4.0  
> **Date:** July 30, 2026  
> **Status:** Phase 1+2+3 Complete  
> **Author:** Buffy (AI Agent)  
> **Supersedes:** Section 12 of web-tools-upgrade-plan.md

---

## 1. Executive Summary

This document formalizes the Phase 2 upgrade roadmap for IGS, focusing on two objectives:

1. **Close Tavily/Firecrawl parity gaps** — Add answer synthesis, time-range filtering, chunked content, and image descriptions to match proprietary API capabilities
2. **Consolidate architecture** — Eliminate redundancies in NLP utilities, HttpClient instantiation, and tool boundary clarity

All upgrades maintain the **zero-API-key, self-contained** architecture.

---

## 2. Parity Gap Analysis (Updated)

### 2.1 Current Parity Score: ~95%

| Feature | IGS | Tavily | Firecrawl | Gap |
|---------|-----|--------|-----------|-----|
| Multi-engine search | ✅ 6 engines | ✅ Single | ✅ Single | ✅ We win |
| Zero API keys | ✅ All free | ❌ $0.005/credit | ❌ Paid | ✅ We win |
| Smart topic routing | ✅ Auto | ✅ Manual | ✅ Basic | ✅ We win |
| BM25 chunk scoring | ✅ | ❌ | ❌ | ✅ We win |
| Relevance scoring | ✅ 3-factor | ✅ AI ranker | ✅ Basic | 🟡 Close |
| **Answer synthesis** | ✅ extractive_answer top-5 + DDG IA | ✅ Built-in LLM | ❌ | ✅ Closed |
| **Time-range filtering** | ✅ HN + DDG + CLI --time-range | ✅ day/week/month | ❌ | ✅ Closed |
| **Chunked content per URL** | ✅ BM25-scored paragraphs + CLI --chunks-per-source | ✅ chunks_per_source | ❌ | ✅ Closed |
| **Image descriptions** | ✅ Wikimedia extmetadata + CLI | ✅ include_image_descriptions | ❌ | ✅ Closed |
| **Deep mode** | ✅ Obscura scrape + CLI --depth deep | ✅ scrape mode | ✅ Firecrawl | ✅ Parity |
| Reliability/SLA | 🟡 Free APIs | ✅ Enterprise | ✅ Enterprise | 🟡 Structural |

### 2.2 Highest-Value Upgrades (Ordered by Impact × Feasibility)

| Priority | Upgrade | Gap Closed | Effort | Risk |
|----------|---------|-----------|--------|------|
| 🔴 P0 | Answer synthesis via TextRank | Tavily `include_answer` | 1 hour | Low |
| 🔴 P0 | Time-range filtering | Tavily `time_range` | 1 hour | Low |
| 🟠 P1 | Chunked content per source | Tavily `chunks_per_source` | 1 hour | Low |
| 🟠 P1 | Image descriptions from metadata | Tavily `include_image_descriptions` | 30 min | Low |
| 🟡 P2 | News topic filter on web.search | Tavily `topic: "news"` | 2 hours | Low |
| 🟡 P2 | Batch extract parallelization | Firecrawl multi-URL | 2 hours | Medium |

---

## 3. Architecture Consolidation Plan

### 3.1 Redundancy Audit Results

#### 🔴 Redundancy 1: NLP Functions Duplicated Across 3 Files

| Function | Location | Used By |
|----------|----------|---------|
| `extract_topics()` | `helpers.rs` | `news_enrich()` |
| `extract_basic_entities()` | `helpers.rs` | `news_enrich()` |
| `basic_sentiment()` | `helpers.rs` | `news_enrich()` |
| `tokenize()` (BM25) | `web.rs` | `bm25_score_chunks()` |
| Stop words (60 words) | `helpers.rs` | `extract_topics()` |
| Stop words (120 words) | `summarize.rs` | `tokenize()` in summarize |

**Problem:** Two different `tokenize()` functions, two different stop-word lists, entity extraction not reusable from web tools.

**Fix:** Create `src/tools/nlp.rs` as canonical NLP utilities module. Move `extract_topics`, `extract_basic_entities`, `basic_sentiment`, unified stop words, and `tokenize` there.

#### 🔴 Redundancy 2: HttpClient Re-instantiated 30+ Times

Every tool function creates its own `HttpClient`:
```rust
let settings = config::load_settings().await?;
let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
let http = HttpClient::new(&settings.http, &cache_dir);
```

This pattern appears in: `web.rs`, `news.rs`, `research.rs`, `weather.rs`, `finance.rs`, `gdelt.rs`, `govt.rs`, `climate.rs`, `env.rs`, `health.rs`, `patents.rs`, `politics.rs`, `satellite.rs`, `security.rs`, `sources.rs`, `data_sources.rs`, `monitor.rs`, `plugins.rs`.

**Problem:** Settings loaded 30+ times per MCP call. Each tool creates its own HttpClient instead of using the one from `AppState`.

**Fix:** Accept `&HttpClient` parameter in tool functions. The MCP server layer passes it from `AppState.http_client`.

#### 🟡 Redundancy 3: web.search vs news.fetch Boundary

Both can search for news, but serve different purposes:
- `web.search` with `topic="news"` → DDG + HN engines (real-time web search)
- `news.fetch` with pools → RSS feeds + web crawling (configured source pipeline)

**Problem:** Tool descriptions don't differentiate well. Agents may use the wrong one.

**Fix:** Update tool descriptions to clarify: "web.search = real-time web search across engines; news.fetch = RSS/web-crawl pipeline from configured sources."

#### 🟡 Redundancy 4: Multiple Content Extraction Paths

| Path | Function | Used By |
|------|----------|---------|
| `extract_main_content()` | CRW text-density | `web.extract` |
| `extract_semantic_excerpt()` | Calls `extract_main_content()` | `web.search` deep mode |
| `parse_duckduckgo_html()` | DDG-specific parsing | `web.search` |
| `parsers::parse_by_source()` | RSS/generic parsers | `news.fetch` |
| `html_to_markdown_rs::convert()` | Generic HTML→MD | `web.scrape` |

**Assessment:** The CRW readability extractor is the most sophisticated but only used in `web.extract`. The deep mode in `web.search` uses a simpler path. This is acceptable for now — the extraction paths serve different use cases.

---

## 4. Implementation Phases

### Phase 1: Tavily/Firecrawl Parity (Quick Wins)

**Duration:** 1-2 days  
**Risk:** Low  
**Files:** `src/tools/web.rs`, `src/tools/types.rs`

#### 1.1 Answer Synthesis via TextRank
- When `include_answer=true`, take top 3 BM25-scored chunks from best result
- Run TextRank summarization (already in `summarize.rs`)
- Return as `WebSearchOutput.answer`
- Add `answer_confidence` field

#### 1.2 Time-Range Filtering
- Add `time_range: Option<String>` to `WebSearchInput` (`"day"`, `"week"`, `"month"`, `"year"`)
- Pass to HN API as `numericFilters=created_at_i>{timestamp}`
- Filter DDG results by parsed date
- Route to news engines when time_range is set

#### 1.3 Chunked Content Per Source
- Add `chunks: Option<Vec<String>>` to `WebSearchResult`
- When `chunks_per_source` is set, split content into paragraphs, score with BM25, return top-N
- Default: None (backward compatible)

#### 1.4 Image Descriptions
- Extract `extmetadata.ImageDescription.value` from Wikimedia API response
- Add `description: Option<String>` to `WebImageResult`
- Expose in MCP output

### Phase 2: Architecture Consolidation

**Duration:** 1 day  
**Risk:** Medium (refactoring)  
**Files:** `src/tools/nlp.rs` (new), `src/tools/helpers.rs`, `src/tools/web.rs`, `src/tools/news.rs`

#### 2.1 Create NLP Utilities Module
- Create `src/tools/nlp.rs` with:
  - Unified `STOP_WORDS` (merge 60 + 120 word lists)
  - `tokenize(text)` — single canonical implementation
  - `extract_topics(text, max)` — moved from helpers.rs
  - `extract_basic_entities(text)` — moved from helpers.rs
  - `basic_sentiment(text)` — moved from helpers.rs
- Update `helpers.rs` to re-export from `nlp.rs`
- Update `web.rs` to use `nlp::tokenize` instead of local `tokenize`
- Update `summarize.rs` to use `nlp::STOP_WORDS`
- Add unit tests for all NLP functions

#### 2.2 HttpClient Passing (Optional — Deferred)
- This is a larger refactor touching 18+ files
- Defer to separate PR to minimize risk
- Document as tech debt

#### 2.3 Tool Description Updates
- Update `web.search` description: clarify it's real-time web search
- Update `news.fetch` description: clarify it's RSS/source pipeline
- Update `web.image_search` description: clarify Wikimedia Commons source

### Phase 3: Testing & Validation

**Duration:** 1 day  
**Files:** Tests in all modified modules

#### 3.1 Unit Tests
- Answer synthesis: test with known BM25 scores
- Time-range filtering: test timestamp calculation
- Chunked content: test paragraph splitting and scoring
- NLP consolidation: verify all functions still work

#### 3.2 Integration Tests
- Live `web.search` with `include_answer=true`
- Live `web.search` with `time_range="week"`
- Live `web.image_search` with description extraction
- CLI commands for all new parameters

#### 3.3 Regression Tests
- All 14 existing tests pass
- Zero clippy warnings
- Zero cargo check warnings

---

## 5. Success Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| Tavily parity score | >85% | Feature matrix comparison |
| Clippy warnings | 0 | `cargo clippy -- -D warnings` |
| Test coverage | 194+ tests | `cargo test` |
| Answer synthesis | Working | `igs web search --query "rust" --include-answer` |
| Time-range filtering | Working | `igs web search --query "AI" --time-range week` |
| NLP consolidation | 0 duplicated functions | Code review |

---

## 6. Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| TextRank quality too low for answers | Fall back to first-result extract if TextRank output is empty |
| Time-range filtering breaks HN API | Validate timestamp math, add error handling |
| NLP refactor breaks existing tests | Run tests after each move, commit incrementally |
| Chunked content increases response size | Make opt-in via parameter, default to None |

---

## 7. Progress Tracking

### Completed ✅

| Item | Status | Commit |
|------|--------|--------|
| Multi-engine search (DDG, Wikipedia, GitHub, HN, SO) | ✅ | Previous session |
| BM25 chunk scoring | ✅ | Previous session |
| Image search (Wikimedia Commons) | ✅ | Previous session |
| Video routing (YouTube via yt-dlp) | ✅ | Previous session |
| Relevance scoring (keyword+freshness+authority) | ✅ | Previous session |
| Smart engine routing | ✅ | Previous session |
| Highlight extraction | ✅ | Previous session |
| Semantic dedup | ✅ | Previous session |
| Caching with TTL | ✅ | Previous session |
| CRW readability extractor | ✅ | Previous session |
| BM25_MIN_THRESHOLD constant | ✅ | Previous session |
| total_available fallback fix | ✅ | Previous session |
| Extractive answer synthesis | ✅ | 96211c1 |
| Time-range filtering (HN + DDG) | ✅ | 96211c1, 78fd133 |
| Image descriptions (Wikimedia extmetadata) | ✅ | 96211c1 |
| NLP consolidation (nlp.rs) | ✅ | 48ff92e |
| Tool description updates | ✅ | 1337dc2 |
| All clippy warnings fixed | ✅ | c57ecc8 |
| Comprehensive unit tests (23 new) | ✅ | ca969a0 |

### Active 🔄

| Item | Status | Phase |
|------|--------|-------|
| HttpClient passing refactor | ⏳ Deferred | Tech Debt |

### Pending ⏳

| Item | Priority | Phase |
|------|----------|-------|
| HttpClient passing (18+ files) | 🟡 Deferred | Tech Debt |
| CLI --depth/--time-range value_enum validation | 🟡 Low | Follow-up |
| CLI --engines flag exposure | 🟡 Low | Follow-up |

---

*This document is updated as implementation progresses. Each phase is committed separately.*
