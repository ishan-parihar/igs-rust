# IGS MCP Server — Upgrade Plan v2

**Version**: 2.0  
**Date**: 2026-06-19  
**Status**: Active  
**Previous**: IGS_UPGRADE_PLAN.md (v1.0 — 15/18 items completed)

---

## Executive Summary

This document outlines the next phase of IGS upgrades. Building on the completed v1 work (56 tools, 13 domains), we now focus on:

1. **Fixing existing issues** — patents URL encoding, Federal Register bot detection
2. **Adding high-value intelligence domains** — Politics (FEC), Health (CDC), Satellite (NASA FIRMS), Research (PubMed)
3. **Extending existing domains** — Climate (NOAA), Legal (CourtListener), Environment (EPA), Supply Chain (UN Comtrade)

**Design Principles:**
- No complex dependencies — all APIs are free, JSON-based, no OAuth2
- Web-scraping fallback for bot-protected APIs
- Consistent integration patterns across all domains
- Production-ready with proper error handling and rate limiting

**Target state:** 68+ tools across 17 intelligence domains.

---

## 1. Current State (v1 Completed)

| Metric | Value |
|--------|-------|
| Total tools | 56 |
| Tool groups | 13 |
| Intelligence domains | 13 |
| Tests | 68 (all passing) |
| Build status | Clean (1 dead_code warning) |

### 1.1 Tools by Domain

| Domain | Tools | API | Status |
|--------|-------|-----|--------|
| Discovery | 13 | Meta-tool | ✅ |
| News | 3 | RSS/Atom | ✅ |
| Research | 3 | arXiv, Semantic Scholar | ✅ |
| Web | 4 | Tavily, Lightpanda | ✅ |
| Insights | 5 | In-memory | ✅ |
| Social | 2 | Reddit JSON | ✅ |
| Weather | 3 | OpenWeatherMap | ✅ |
| Finance | 3 | Yahoo Finance, CoinGecko | ✅ |
| Security | 2 | NVD, GitHub Advisory | ✅ |
| Patents | 2 | PatentsView | ⚠️ URL encoding issue |
| Government | 2 | Congress.gov, Federal Register | ⚠️ FR bot detection |
| SOP | 2 | Built-in chains | ✅ |
| Browser | 12 | Lightpanda | ✅ |

### 1.2 Known Issues

| Issue | Severity | File | Fix |
|-------|----------|------|-----|
| Patents URL encoding | Medium | `patents.rs` | Replace custom `urlencoding()` with `url::form_urlencoded` |
| Patents `per_page` misuse | Low | `patents.rs` | Separate page size from year range |
| Federal Register bot detection | Low | `govt.rs` | Add User-Agent rotation or document limitation |
| Finance HTTP client inconsistency | Low | `finance.rs` | Standardize to use IGS HttpClient |
| sop.rs unused import | Low | `sop.rs` | Move import inside `#[cfg(test)]` |

---

## 2. Upgrade Plan

### Phase 1: Fix Existing Issues (1 day)

**Goal:** Resolve all known issues before adding new features.

#### 1.1 Fix Patents URL Encoding

**File:** `src/tools/patents.rs`

**Problem:** The `q` parameter contains JSON like `{"patent_number":"12345"}` but `urlencoding()` only handles spaces and commas — not `{`, `"`, `}` characters.

**Solution:** Use `url::form_urlencoded::Serializer` for proper percent-encoding.

```rust
// Before (broken)
let url = format!(
    "https://api.patentsview.org/patents/query?q={}&f={}&o={}",
    query, fields, opts
);

// After (fixed)
let mut params = url::form_urlencoded::Serializer::new(String::new());
params.append_pair("q", &serde_json::to_string(&json!({"_contains": input.query})).unwrap());
params.append_pair("f", fields);
params.append_pair("o", &opts);
let url = format!("https://api.patentsview.org/patents/query?{}", params.finish());
```

**Also:** Fix `per_page` misuse — separate page size from year range:
```rust
let per_page = input.limit.unwrap_or(20).clamp(1, 100);
let opts = format!(r#"{{"per_page":{}}}"#, per_page);
```

#### 1.2 Fix Finance HTTP Client Inconsistency

**File:** `src/tools/finance.rs`

**Problem:** Uses `reqwest::Client::new()` directly instead of IGS HttpClient.

**Solution:** Refactor to use shared HttpClient from server state:
```rust
pub async fn finance_market(input: FinanceMarketInput) -> Result<FinanceMarketOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    
    for symbol in &input.symbols {
        let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d", symbol);
        let outcome = http.fetch(&url, None, "bypass").await
            .map_err(|e| format!("Yahoo Finance error: {}", e))?;
        // ... parse response
    }
}
```

#### 1.3 Fix sop.rs Unused Import

**File:** `src/tools/sop.rs`

**Problem:** `OutputOptions` imported at module level but only used in test module.

**Solution:** Move import inside `#[cfg(test)]` block:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::types_base::OutputOptions;
    // ...
}
```

#### 1.4 Update README.md

Update tool count from 56 to reflect any additions.

**Deliverables:**
- [ ] Patents URL encoding fixed
- [ ] Finance HTTP client standardized
- [ ] sop.rs unused import removed
- [ ] All 68 tests pass
- [ ] README updated

---

### Phase 2: New Intelligence Domains (3 days)

**Goal:** Add 4 high-value, easy-integration domains.

#### 2.1 Politics Domain (FEC API)

**API:** FEC OpenFEC (`https://api.open.fec.gov/v1/`)  
**Auth:** Optional API key (free via data.gov)  
**Rate limit:** 1,000 calls/hour  
**Free tier:** Yes

**New tools:**
- `politics.fec_candidates` — Search candidates by name, party, office
- `politics.fec_committees` — Search committees (PACs, parties)

**Implementation:**
```rust
// src/tools/politics.rs
pub async fn politics_fec_candidates(input: PoliticsFecInput) -> Result<PoliticsFecOutput, String> {
    let client = reqwest::Client::new();
    let name = urlencoding(&input.name);
    let office = input.office.as_deref().unwrap_or("");
    
    let mut url = format!(
        "https://api.open.fec.gov/v1/candidates/?name={}&per_page={}",
        name, input.limit.unwrap_or(20)
    );
    
    if !office.is_empty() {
        url = format!("{}&office={}", url, office);
    }
    
    let resp = client.get(&url)
        .header("User-Agent", "IGS-MCP/0.5")
        .send().await.map_err(|e| format!("HTTP error: {}", e))?;
    
    // Parse JSON response into PoliticsFecOutput
}
```

**Types to add:**
```rust
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoliticsFecInput {
    pub name: String,
    pub office: Option<String>,  // "P", "S", "H" (President, Senate, House)
    pub party: Option<String>,   // "DEM", "REP", etc.
    #[serde(flatten)]
    pub output: OutputOptions,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PoliticsFecOutput {
    pub name: String,
    pub total_results: usize,
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
```

#### 2.2 Health Domain (CDC Open Data)

**API:** CDC SODA (`https://data.cdc.gov/resource/`)  
**Auth:** None required  
**Rate limit:** 1,000 req/hour  
**Free tier:** Yes

**New tools:**
- `health.cdc_leading_causes` — Leading causes of death by state/year
- `health.cdc_covid` — COVID-19 data by state/county

**Implementation:**
```rust
pub async fn health_cdc_leading_causes(input: HealthCdcInput) -> Result<HealthCdcOutput, String> {
    let client = reqwest::Client::new();
    let state = input.state.as_deref().unwrap_or("");
    let year = input.year.unwrap_or(2021);
    
    let mut url = format!(
        "https://data.cdc.gov/resource/3y38-azbh.json?$where=year='{}'&$order=deaths DESC&$limit={}",
        year, input.limit.unwrap_or(20)
    );
    
    if !state.is_empty() {
        url = format!("{}&$where=state='{}'", url, state);
    }
    
    let resp = client.get(&url)
        .header("User-Agent", "IGS-MCP/0.5")
        .send().await.map_err(|e| format!("HTTP error: {}", e))?;
    
    // Parse JSON response
}
```

#### 2.3 Satellite Domain (NASA FIRMS)

**API:** NASA FIRMS (`https://firms.modaps.eosdis.nasa.gov/api/area/`)  
**Auth:** MAP_KEY (free registration)  
**Rate limit:** 10-minute rolling window  
**Free tier:** Yes

**New tools:**
- `satellite.firms_fires` — Active fire hotspots by region/date

**Implementation:**
```rust
pub async fn satellite_firms_fires(input: SatelliteFirmsInput) -> Result<SatelliteFirmsOutput, String> {
    let client = reqwest::Client::new();
    let bbox = format!("{},{},{},{}", input.west, input.south, input.east, input.north);
    let date = input.date.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    
    let url = format!(
        "https://firms.modaps.eosdis.nasa.gov/api/area/csv/DEMO_KEY/VIIRS_SNPP_NRT/{}/1",
        bbox
    );
    
    let resp = client.get(&url)
        .header("User-Agent", "IGS-MCP/0.5")
        .send().await.map_err(|e| format!("HTTP error: {}", e))?;
    
    // Parse CSV response into SatelliteFirmsOutput
}
```

#### 2.4 Research Extension (PubMed)

**API:** PubMed E-utilities (`https://eutils.ncbi.nlm.nih.gov/entrez/eutils/`)  
**Auth:** API key optional (free, higher rate limits)  
**Rate limit:** 3 req/sec without key, 10/sec with key  
**Free tier:** Yes

**New tools:**
- `research.pubmed_search` — Search PubMed for medical research papers

**Implementation:**
```rust
pub async fn research_pubmed_search(input: ResearchPubMedInput) -> Result<ResearchPubMedOutput, String> {
    let client = reqwest::Client::new();
    let query = urlencoding(&input.query);
    let limit = input.limit.unwrap_or(20);
    
    // Step 1: Search for PMIDs
    let search_url = format!(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&term={}&retmax={}&retmode=json",
        query, limit
    );
    
    let search_resp = client.get(&search_url)
        .header("User-Agent", "IGS-MCP/0.5")
        .send().await.map_err(|e| format!("HTTP error: {}", e))?;
    
    let search_data: serde_json::Value = search_resp.json().await
        .map_err(|e| format!("JSON error: {}", e))?;
    
    let pmids: Vec<String> = search_data["esearchresult"]["idlist"]
        .as_array()
        .map(|ids| ids.iter().filter_map(|id| id.as_str().map(String::from)).collect())
        .unwrap_or_default();
    
    if pmids.is_empty() {
        return Ok(ResearchPubMedOutput { query: input.query, total: 0, papers: vec![] });
    }
    
    // Step 2: Fetch details for PMIDs
    let ids = pmids.join(",");
    let detail_url = format!(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pubmed&id={}&retmode=json",
        ids
    );
    
    let detail_resp = client.get(&detail_url)
        .header("User-Agent", "IGS-MCP/0.5")
        .send().await.map_err(|e| format!("HTTP error: {}", e))?;
    
    // Parse response into ResearchPubMedOutput
}
```

**Deliverables:**
- [ ] `src/tools/politics.rs` created with FEC tools
- [ ] `src/tools/health.rs` created with CDC tools
- [ ] `src/tools/satellite.rs` created with FIRMS tools
- [ ] `research.pubmed_search` added to existing research module
- [ ] All types added to `src/tools/types.rs`
- [ ] All modules declared in `src/tools/mod.rs`
- [ ] All tools registered in `src/tools/registry.rs`
- [ ] All tools registered in `src/server.rs`
- [ ] Registry tests updated
- [ ] All tests pass

---

### Phase 3: Extended Domains (4 days)

**Goal:** Add 4 medium-effort domains that extend existing capabilities.

#### 3.1 Climate Domain (NOAA CDO)

**API:** NOAA Climate Data Online (`https://www.ncei.noaa.gov/cdo-web/api/v2/`)  
**Auth:** API token (free registration)  
**Rate limit:** 10,000 requests/day  
**Free tier:** Yes

**New tools:**
- `climate.noaa_observations` — Historical weather observations
- `climate.noaa_stations` — Find weather stations

#### 3.2 Legal Domain (CourtListener)

**API:** CourtListener (`https://www.courtlistener.com/api/rest/v4/`)  
**Auth:** Token (free registration)  
**Rate limit:** 125 req/day  
**Free tier:** Yes

**New tools:**
- `legal.search_cases` — Search case law
- `legal.case_details` — Get case details with opinions

#### 3.3 Environment Domain (EPA Envirofacts)

**API:** EPA Envirofacts (`https://data.epa.gov/dmapservice/`)  
**Auth:** None required  
**Rate limit:** Generous  
**Free tier:** Yes (fully open)

**New tools:**
- `env.epa_facilities` — Search EPA-regulated facilities
- `env.epa_emissions` — Toxic release inventory data

#### 3.4 Supply Chain Domain (UN Comtrade)

**API:** UN Comtrade (`https://comtradeapi.un.org/data/v1/get/`)  
**Auth:** API key (free registration)  
**Rate limit:** 500 calls/day  
**Free tier:** Yes

**New tools:**
- `supply_chain.trade_flows` — International trade statistics

**Deliverables:**
- [ ] `src/tools/climate.rs` created
- [ ] `src/tools/legal.rs` created
- [ ] `src/tools/env.rs` created
- [ ] `src/tools/supply_chain.rs` created
- [ ] All types, modules, registry, server entries
- [ ] All tests pass

---

## 3. Integration Patterns

### 3.1 Standard Tool Pattern

Every new tool follows this exact pattern:

```rust
// src/tools/<domain>.rs
use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::types::*;
use crate::tools::helpers::urlencoding;

pub async fn <domain>_<tool>(input: <Domain>Input) -> Result<Domain>Output, String> {
    // 1. Load settings and create HTTP client
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);
    
    // 2. Build URL with proper encoding
    let query = urlencoding(&input.query);
    let url = format!("https://api.example.com/endpoint?q={}", query);
    
    // 3. Make request with error handling
    let outcome = http.fetch(&url, None, "bypass").await
        .map_err(|e| format!("API error: {}", e))?;
    
    // 4. Extract response
    let resp = match outcome {
        http_mod::FetchOutcome::Response(r, _, _) => r,
        _ => return Err("API returned cached response".into()),
    };
    
    // 5. Parse JSON
    let json: serde_json::Value = serde_json::from_str(&resp.body_text)
        .map_err(|e| format!("JSON parse error: {}", e))?;
    
    // 6. Check for API errors
    if json["error"].as_str().is_some() {
        return Err(format!("API error: {}", json["error"].as_str().unwrap_or("unknown")));
    }
    
    // 7. Extract and transform data
    let items = json["results"].as_array()
        .map(|arr| arr.iter().map(|item| {
            // Transform to output type
        }).collect())
        .unwrap_or_default();
    
    Ok(DomainOutput { query: input.query, total: items.len(), items })
}
```

### 3.2 Anti-Patterns to Avoid

| Anti-Pattern | Why | Correct Approach |
|--------------|-----|------------------|
| Using `reqwest::Client::new()` directly | Inconsistent, no caching/retry | Use IGS `HttpClient` |
| Custom URL encoding | Incomplete, misses special chars | Use `url::form_urlencoded` |
| Swallowing errors silently | Hard to debug | Return descriptive error messages |
| No rate limiting | Gets API banned | Respect API rate limits |
| Hardcoding API keys | Security risk | Use `${ENV_VAR}` pattern in settings.yml |

### 3.3 Configuration Pattern

All API keys are stored in `settings.yml` with env var expansion:

```yaml
# settings.yml
fec:
  enabled: true
  api_key: ${FEC_API_KEY}  # Optional, works without key

cdc:
  enabled: true
  # No API key needed

nasa_firms:
  enabled: true
  api_key: ${NASA_FIRMS_KEY}  # Free registration

noaa:
  enabled: true
  api_key: ${NOAA_TOKEN}  # Free registration
```

---

## 4. Success Metrics

### 4.1 Tool Count

| Metric | Before | After Phase 1 | After Phase 2 | After Phase 3 |
|--------|--------|---------------|---------------|---------------|
| Total tools | 56 | 56 | 62 | 68 |
| Tool groups | 13 | 13 | 17 | 17 |
| Intelligence domains | 13 | 13 | 17 | 17 |

### 4.2 Quality Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Compilation warnings | 0 | `cargo build 2>&1 | grep warning` |
| Test pass rate | 100% | `cargo test` |
| API endpoint verification | 100% | Manual testing of each endpoint |
| Error handling coverage | 100% | All API errors return descriptive messages |
| Rate limit compliance | 100% | No API bans |

### 4.3 Domain Coverage

| Category | Before | After |
|----------|--------|-------|
| News/Media | ✅ | ✅ |
| Research/Academic | ✅ | ✅ |
| Web Intelligence | ✅ | ✅ |
| Social Media | ✅ | ✅ |
| Weather | ✅ | ✅ |
| Finance | ✅ | ✅ |
| Security | ✅ | ✅ |
| Patents | ⚠️ | ✅ |
| Government | ⚠️ | ✅ |
| **Politics** | ❌ | ✅ |
| **Health** | ❌ | ✅ |
| **Satellite/Environmental** | ❌ | ✅ |
| **Climate** | ❌ | ✅ |
| **Legal** | ❌ | ✅ |
| **Environment** | ❌ | ✅ |
| **Supply Chain** | ❌ | ✅ |

---

## 5. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| API deprecation | Low | Medium | Monitor API changelogs, maintain fallbacks |
| Rate limiting | Medium | Low | Implement per-domain rate limiting, cache aggressively |
| Bot detection | Medium | Low | Use IGS HttpClient with proper User-Agent, fallback to web scraping |
| Data format changes | Low | Medium | Use flexible JSON parsing with fallbacks |
| Authentication changes | Low | High | Document all API key requirements, provide setup guide |

---

## 6. Implementation Order

### Week 1: Phase 1 (Fix Existing)
- Day 1: Fix patents URL encoding + finance HTTP client + sop import
- Day 2: Verify all fixes, update README

### Week 2: Phase 2 (New Domains)
- Day 3: Politics domain (FEC API)
- Day 4: Health domain (CDC Open Data)
- Day 5: Satellite domain (NASA FIRMS)
- Day 6: Research extension (PubMed)
- Day 7: Integration testing

### Week 3: Phase 3 (Extended Domains)
- Day 8: Climate domain (NOAA CDO)
- Day 9: Legal domain (CourtListener)
- Day 10: Environment domain (EPA Envirofacts)
- Day 11: Supply Chain domain (UN Comtrade)
- Day 12: Integration testing

### Week 4: Polish
- Day 13: Update tool_guide with new domains
- Day 14: Update AGENTS.md and README.md
- Day 15: Final testing and documentation

---

## 7. Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `src/tools/politics.rs` | FEC API tools |
| `src/tools/health.rs` | CDC Open Data tools |
| `src/tools/satellite.rs` | NASA FIRMS tools |
| `src/tools/climate.rs` | NOAA CDO tools |
| `src/tools/legal.rs` | CourtListener tools |
| `src/tools/env.rs` | EPA Envirofacts tools |
| `src/tools/supply_chain.rs` | UN Comtrade tools |

### Modified Files
| File | Changes |
|------|---------|
| `src/tools/types.rs` | Add all new types |
| `src/tools/mod.rs` | Add new module declarations |
| `src/tools/registry.rs` | Add new tool groups + update tests |
| `src/server.rs` | Add new tool handlers + impl_has_format entries |
| `src/tools/patents.rs` | Fix URL encoding |
| `src/tools/finance.rs` | Standardize HTTP client |
| `src/tools/sop.rs` | Remove unused import |
| `README.md` | Update tool count |
| `AGENTS.md` | Update tool documentation |

---

## 8. Appendix: API Reference

### 8.1 FEC OpenFEC
- **Base URL:** `https://api.open.fec.gov/v1/`
- **Endpoints:** `/candidates/`, `/committees/`
- **Auth:** Optional API key (free)
- **Docs:** https://docs.open.fec.us/

### 8.2 CDC SODA
- **Base URL:** `https://data.cdc.gov/resource/`
- **Dataset IDs:**
  - Leading causes: `3y38-azbh`
  - COVID-19: `9mfq-cb36`
- **Auth:** None
- **Docs:** https://dev.socrata.com/data/

### 8.3 NASA FIRMS
- **Base URL:** `https://firms.modaps.eosdis.nasa.gov/api/area/`
- **Format:** `{base}/{map_key}/{source}/{bbox}/{date}`
- **Auth:** MAP_KEY (free registration)
- **Docs:** https://firms.modaps.eosdis.nasa.gov/api/area/

### 8.4 PubMed E-utilities
- **Base URL:** `https://eutils.ncbi.nlm.nih.gov/entrez/eutils/`
- **Endpoints:** `esearch.fcgi`, `esummary.fcgi`, `efetch.fcgi`
- **Auth:** API key optional (free)
- **Docs:** https://www.ncbi.nlm.nih.gov/books/NBK25501/

### 8.5 NOAA CDO
- **Base URL:** `https://www.ncei.noaa.gov/cdo-web/api/v2/`
- **Endpoints:** `/data`, `/datasets`, `/stations`
- **Auth:** API token (free registration)
- **Docs:** https://www.ncdc.noaa.gov/cdo-web/webservices/v2

### 8.6 CourtListener
- **Base URL:** `https://www.courtlistener.com/api/rest/v4/`
- **Endpoints:** `/search/`, `/dockets/`, `/opinions/`
- **Auth:** Token (free registration)
- **Docs:** https://www.courtlistener.com/api/rest-info/

### 8.7 EPA Envirofacts
- **Base URL:** `https://data.epa.gov/dmapservice/`
- **Endpoints:** `/efservice/`, `/vocabulary/`
- **Auth:** None
- **Docs:** https://www.epa.gov/enviro/web-services

### 8.8 UN Comtrade
- **Base URL:** `https://comtradeapi.un.org/data/v1/get/`
- **Format:** `{base}/{type}/{freq}/{clCode}`
- **Auth:** API key (free registration)
- **Docs:** https://comtradeapi.un.org/docs
