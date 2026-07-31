# IGS-Rust Production Audit Report

**Date:** 2026-07-31
**Repository:** igs-rust (MCP Server + CLI)
**Version:** 0.5.5 (Cargo.toml) — README/settings show stale 0.5.2/0.5.0
**Scope:** Full codebase — 80+ tool handlers, HTTP layer, MCP boundary, concurrency, security, CLI/UX

---

## Executive Summary

| Severity | Count | Categories |
|----------|-------|------------|
| **CRITICAL** | 11 | Production panics, SSRF, unbounded memory, poisoned mutexes |
| **HIGH** | 18 | Error-chain loss, input validation gaps, concurrency races, version skew |
| **MEDIUM** | 23 | Unused error variants, O(N²) rebuilds, TOCTOU, schema defaults |
| **LOW** | 13 | Install/UX polish, test naming, doc coverage |

**Key takeaway:** The codebase is functional and well-structured for its domain, but has **zero SSRF protection**, **unbounded response sizes**, and **several production panic paths** that can crash the MCP server on a single malformed request. The monitor subsystem is the largest single risk surface.

---

## CRITICAL Findings (Fix Immediately)

### 1. `load_settings_sync().expect("Failed to load settings")` — Process crash on startup
**File:** `src/server.rs:500`
```rust
let settings = load_settings_sync().expect("Failed to load settings");
```
**Impact:** Fresh install with missing/corrupt `settings.yml` panics the MCP server. Violates project rule #1979 (AppError everywhere internal).
**Fix:** `?` with `AppError::config(...)`.

### 2. SSRF — Zero URL validation on ANY fetch path
**Files:** `src/tools/web/extractors.rs`, `src/tools/sources.rs`, `src/obscura.rs`, `src/lightpanda.rs`
**Attack:** `{"url": "http://169.254.169.254/latest/meta-data/iam/security-credentials/"}` — reaches AWS metadata via default reqwest redirects.
**Fix:** Single `validate_public_url(url)` helper at every entry point: reject bad schemes, resolve DNS, deny-loopback/link-local/private/CGNAT.

### 3. Unbounded response body — Memory DoS / RSS bomb
**File:** `src/http.rs:212` (`res.text().await?`) — no `body_limit`, no `Content-Length` check.
**Impact:** 10 GB feed → OOM. Decompression bomb via gzip/brotli (reqwest defaults to unbounded).
**Fix:** `Client::builder().body_limit(50*1024*1024)` + streaming for large payloads.

### 4. Obscura binary update — Zip-slip + decompression bomb
**File:** `src/obscura.rs:166-215` — `archive.unpack()` without path validation or size cap.
**Impact:** Compromised GitHub Release → arbitrary file write + disk fill.
**Fix:** Stream with `.take(200MB)`, validate each entry path under `binary_dir`, verify SHA-256 against pinned manifest.

### 5. Cookie/secrets in `settings.yml` — No file permission enforcement
**Files:** `src/tools/twitter.rs`, `src/tools/reddit.rs`
**Impact:** `auth_token`/`ct0` = account takeover. World-readable `~/.config/igs-mcp/settings.yml` by default.
**Fix:** `chmod 600` on load, redact in `Debug`, document in README.

### 6. Monitor dispatcher — 3 interlocking CRITICAL issues
**File:** `src/tools/monitor.rs:385-456`

| Issue | Line | Risk |
|-------|------|------|
| Orphaned `tokio::spawn` — no `JoinHandle` retention | 391, 421 | Panic in any task kills process |
| `loop {}` with no shutdown signal | 395 | Cannot gracefully stop; SIGKILL mid-alert |
| Unbounded fan-out — no per-monitor in-flight tracking | 402-451 | Hundreds of tasks accumulate under slow upstream |

**Fix:** `CancellationToken` + `HashMap<String, JoinHandle>` + skip if `!handle.is_finished()`.

### 7. Twitter/Reddit cookie `.expect()` panics
**Files:** `src/tools/twitter.rs:87-90`, `src/tools/reddit.rs:75`
```rust
HeaderValue::from_str(cookie_str).expect("cookie is ASCII")
```
**Impact:** Malformed cookie (newline from paste) → MCP server crash.
**Fix:** Return `AppError::validation("invalid cookie: control characters")`.

### 8. `std::sync::Mutex` poisoning — Cache + scoring
**Files:** `src/cache.rs:63,100,112,119`, `src/tools/web/scoring.rs:51,73,85`
```rust
self.lru_order.lock().unwrap()
```
**Impact:** One panic in any task holding the lock → every subsequent cache access panics → process death.
**Fix:** `lock().unwrap_or_else(|p| p.into_inner())` or switch to `parking_lot::Mutex` (no poisoning).

### 9. MCP Server version skew — `0.2.0` vs `0.5.5`
**File:** `src/server.rs:1937`
```rust
.with_server_info(Implementation::new("igs-rust", "0.2.0"))
```
**Impact:** AI agents record wrong version; diagnostics confusion.
**Fix:** `env!("CARGO_PKG_VERSION")`.

### 10. `unchecked_transaction()` invariant violation risk
**File:** `src/server.rs:127-156`, `src/persistence.rs:84-89`
**Impact:** Safe *today* only because `InsightStorage` is always behind tokio `Mutex`. One refactor → UB/SQLite corruption.
**Fix:** Use safe `conn.transaction()`.

### 11. `unreachable!()` coupling 24 tool files to one HTTP invariant
**Files:** `src/tools/web/extractors.rs:36`, `src/tools/research.rs:305/331/451/486/545`, etc. (24 sites)
**Impact:** If `HttpClient::fetch` ever returns `Cached` for `bypass` mode, every tool panics.
**Fix:** Match all arms → `AppError::other("unexpected cached response")`.

---

## HIGH Findings

### 12. Systemic `.map_err(|e| format!("X error: {}", e))?` — 53+ sites lose error chain
**Pattern:** Every tool file (finance, gdelt, govt, politics, health, env, data_sources, weather, legal, patents, climate, plugins, research, news, monitor, sources, pools, reddit, youtube, lp_mcp, web/extractors, web/engines, server, satellite).
**Impact:** `reqwest::Error` with URL/status/body → flat string. Bypasses `AppError::Http(#[from])` and `AppError::Json(#[from])`. `$` prefix typos at health.rs:98, research.rs:455, climate.rs:48, env.rs:37 produce literal `$` in errors.
**Fix:** Use `?` directly — `#[from]` impls already exist in `error.rs`.

### 13. `news.fetch` `limit` unclamped — DoS vector
**Files:** `src/cli.rs:296`, `src/tools/news.rs:64`, `src/tools/types.rs:272-287`
**Impact:** `--limit 999999` accepted; other tools clamp at 500.
**Fix:** Clamp at top of handler; change `i32` → `u32`.

### 14. `std::sync::Mutex` in async context — `scoring.rs`
**File:** `src/tools/web/scoring.rs:51-93`
```rust
static SEARCH_CACHE: LazyLock<std::sync::Mutex<...>> = ...;
pub fn cache_get(...) { SEARCH_CACHE.lock().ok()?; }
```
**Risk:** Called from async `web_search`; blocks worker thread. `clone()` of MB-scale `WebSearchOutput` under lock.
**Fix:** `tokio::sync::Mutex` or `DashMap`.

### 15. `HostSemaphoreMap` unbounded memory growth
**File:** `src/http.rs:24-47`
**Impact:** New host = new `HashMap` entry + `Arc<Semaphore>` never removed. Weeks of varied news sources = slow leak.
**Fix:** Track `last_seen: Instant` per host; evict >1h idle. Or `DashMap` with cleanup task.

### 16. `IgsMcpServer::new_with_groups` calls `monitor.start_all()` before `Ok(Self)`
**File:** `src/server.rs:499-518`
**Risk:** Dispatcher runs with half-built server if later init fails. No shutdown hook stored.
**Fix:** Make `start_all` async returning `AbortHandle`; store in server; join on `Drop`.

### 17. Obscura/Lightpanda binary download — blocking `std::fs` in async
**Files:** `src/obscura.rs:183-212`
**Impact:** Multi-MB tar extraction on tokio worker starves runtime for seconds.
**Fix:** Wrap in `tokio::task::spawn_blocking`.

### 18. `web_crawl` BFS follows links without SSRF check
**Files:** `src/tools/web/extractors.rs:264-340`, `:411-489`
**Impact:** `base_host` filter can be bypassed by DNS resolving to private IP; `javascript:`/`data:` URLs passed to browser engine.
**Fix:** Scheme allowlist (`http`/`https` only) + IP deny-list on every queued URL.

### 19. `research.download` path traversal
**File:** `src/tools/research.rs:586-592`
```rust
let output_path = input.output_path.unwrap_or(...);
std::fs::write(&output_path, &bytes)
```
**Impact:** User writes arbitrary files (`~/.ssh/authorized_keys`).
**Fix:** Resolve against fixed download dir; reject `..`/absolute.

### 20. YouTube subtitles `/tmp` symlink race
**File:** `src/tools/youtube.rs:150-167`
**Impact:** Predictable prefix `igs_sub_` → symlink attack on multi-user systems.
**Fix:** `tempfile::tempdir()`.

### 21. `ObscuraManager::screenshot` port collision
**File:** `src/tools/obscura.rs:276-280`
```rust
let port = 9222 + (millis % 1000) as u16;
```
**Impact:** Two concurrent calls same ms → bind failure.
**Fix:** `AtomicU16` counter or `TcpListener::bind("127.0.0.1:0")`.

### 22. Twitter bearer token hardcoded without comment
**File:** `src/tools/twitter.rs:15`
**Impact:** Misleads auditors; cookie is the real secret.
**Fix:** Add comment: `// Public Twitter API bearer token (not a secret)`.

### 23. `web_search` engine fan-out unbounded
**File:** `src/tools/web/mod.rs:50-118`
**Impact:** 7 engines = 7 concurrent Obscura subprocesses.
**Fix:** `Semaphore::new(4)` per engine batch.

### 24. `web_extract` batch discards N-1 successes
**File:** `src/tools/web/extractors.rs:691-696`
**Impact:** Spawns 5 Obscura procs, uses 1. 10-15s wasted.
**Fix:** `try_join_all` + return first success with others aborted.

### 25. `CURRENT_URL: OnceLock<Mutex<String>>` singleton race
**File:** `src/tools/lp_mcp.rs:7-21`
**Impact:** Process-wide URL overwritten by concurrent `lp_goto` → `lp_markdown` reads wrong URL.
**Fix:** Remove global; pass URL explicitly per call.

### 26. `settings.example.yml` version `0.5.2` vs `Cargo.toml` `0.5.5`
**Files:** `README.md` (4 places), `config/settings.example.yml:7`, `Cargo.toml:3`
**Impact:** Install script downloads wrong version; confusion.
**Fix:** Single source of truth; CI check for drift.

### 27. `default_format: toon` ignored by CLI and MCP
**Files:** `src/cli.rs:22`, `src/server.rs:466`, `config/settings.example.yml:43`
**Impact:** `output.default_format: json` in settings does nothing.
**Fix:** Read `settings.output.default_format` in both `cli.rs` and `server.rs::resolve_format`.

### 28. `tool_groups` default = ALL groups (90 tools) — most fail without API keys
**Files:** `src/cli.rs:1238`, `src/server.rs:499`
```rust
let tool_groups = settings.tool_groups.unwrap_or_default();
```
**Impact:** Fresh `igs mcp` shows 90 tools; `tavily`, `firecrawl`, `twitter`, `reddit` fail immediately.
**Fix:** Default to `["discovery", "news", "research", "web", "insights"]` (~30 safe tools).

### 29. No `deny(missing_docs)` + zero doc tests
**Files:** `src/lib.rs`, `Cargo.toml`
**Impact:** Library crate (`igs_rust_mcp`) has public API with no forced docs; `cargo test --doc` does nothing.
**Fix:** `#![warn(missing_docs)]` on `lib.rs`; add 1-2 `///` examples on top types.

---

## MEDIUM Findings

| # | Finding | File(s) | Fix Sketch |
|---|---------|---------|------------|
| 30 | `AppError::Validation` never used | `src/error.rs:29-30` | Migrate input checks (reddit empty query, twitter cookie, bad lat/lon) |
| 31 | `AppError::Config` only in type def | `src/error.rs:64-66` | Use in `load_settings_sync().map_err(AppError::config(...))` |
| 32 | `config.rs` returns `anyhow::Result` not `AppResult` | `src/config.rs` | Migrate `read_yaml`, `load_*` to `AppResult` |
| 33 | `latitude.parse().unwrap_or(0.0)` — Gulf of Guinea = valid 0,0 | `src/tools/satellite.rs:65-74` | `try_into` with `Validation` error |
| 34 | `SemanticIndex::add()` triggers O(N²) full rebuild | `src/tools/semantic.rs:64-89` | Batch inserts via existing `add_batch` |
| 35 | `InsightStorage` lock held during long alias sweep | `src/server.rs:240-291`, `src/tools/insights.rs:13-29` | `Arc<RwLock>` for read-heavy paths |
| 36 | `FeedCache::evict_if_needed` TOCTOU + blocking `remove_file` | `src/cache.rs:109-126` | Single lock scope; `spawn_blocking` for deletes |
| 37 | `parsers.rs` selector parse failures silent | `src/parsers.rs:298,391,403` | `tracing::warn!` on failure |
| 38 | `web_crawl` Lightpanda/Obscura `obey_robots` asymmetry | `src/obscura.rs:222` (unused `_`) | Implement or rename |
| 39 | `parsers.list` ignores `format` param | `src/server.rs:708-721` | Use `Self::resolve_format` |
| 40 | `web.screenshot` hardcodes `json` | `src/server.rs:1316-1326` | Honor `params.0.output.format` |
| 41 | `news.test_source` unknown id → silent 0 items | `src/tools/news.rs` | Return `Validation` error |
| 42 | Subcommand order buries `status`/`tool-groups` | `src/cli.rs:32-181` | Reorder: mcp, status, tool-groups, news, web, research... |
| 43 | `TwitterAction`/`YoutubeAction` missing variant docs | `src/cli.rs:563-595` | Add `///` like `WeatherAction` |
| 44 | `limit` fields `i32` not `u32` across CLI | `src/cli.rs`, `src/tools/types.rs` | Standardize to `u32` |
| 45 | `r()` helper loses error chain for exit codes | `src/cli.rs:1089-1092` | Map `AppError` variants to distinct exit codes |
| 46 | `--full` truncates strings invisibly | `src/cli.rs:1100-1109` | Show `...` marker or document |
| 47 | MCP tool descriptions >300 chars → truncated by clients | `src/server.rs:1240,727` | Trim to ~120 chars |
| 48 | `output: OutputOptions` flatten may break older MCP clients | `src/tools/types.rs:106-118` | Verify schema flattens; fallback to top-level |
| 49 | `parity_test.rs` brittle clap parsing | `tests/parity_test.rs:91-128` | `igs --help --format=json` |
| 50 | Version string lacks git SHA | `Cargo.toml:3`, `src/cli.rs:18` | `vergen` build script |
| 51 | `decision_tree` in tool_guide has 3× duplicates | `src/tools/tool_guide.rs:78-93,122-137` | Canonicalize one entry per tool |
| 52 | `uninstall.sh` leaves Claude/Codex/OpenCode hooks | `scripts/uninstall.sh` | Surgically remove hook blocks with `jq` |

---

## LOW Findings

| # | Finding | File(s) |
|---|---------|---------|
| 53 | README install `curl \| bash` no SHA256 | `README.md:25-27`, `scripts/install.sh:60` |
| 54 | `~/.local/bin` not in default PATH on macOS | `scripts/install.sh:91-96` |
| 55 | `configure-mcp.sh` doesn't handle multiple `igs` on PATH | `scripts/configure-mcp.sh:13-25` |
| 56 | `Environment` group in tool_guide but `satellite` CLI separate | `src/tools/tool_guide.rs:570-587`, `src/tools/registry.rs:110-115` |
| 57 | `web_scrape`/`web_crawl` no robots.txt enforcement in Obscura | `src/obscura.rs:222` |
| 58 | Debug log dumps full Twitter GraphQL response | `src/tools/twitter.rs:322-325` |
| 59 | `pub_date` parse failure → epoch (0) | `src/parsers.rs:1275-1278` |
| 60 | `yt-dlp` arg injection via `--` prefix in query | `src/tools/youtube.rs:9-15` |
| 61 | Cache key race / tmp file race in `cache.write` | `src/http.rs:281-289` (verify `cache.rs:49-54`) |
| 62 | `web_map` uses `base_url/sitemap.xml` without SSRF check | `src/tools/web/extractors.rs:1149` |
| 63 | Twitter cookie sanitization (strip control chars) | `src/tools/twitter.rs` |
| 64 | `tool_guide` MCP resource not documented in README | `README.md` |
| 65 | `lightpanda.rs` no self-update; Obscura has it | `src/lightpanda.rs` |

---

## Recommended Fix Order (Priority Queue)

| Phase | Tasks | Rationale |
|-------|-------|-----------|
| **P0 — Process Survival** | 1, 4, 5, 6, 7, 8 | Eliminate crashes on bad input, poisoned locks, unbounded tasks |
| **P1 — Security Boundary** | 2, 3, 12, 13, 18, 19, 20 | SSRF, memory DoS, path traversal, error chain loss |
| **P2 — Correctness** | 9, 10, 11, 14, 15, 16, 17 | Version skew, invariant coupling, blocking in async, semaphore leak |
| **P3 — Ergonomics** | 21, 22, 23, 24, 25, 26, 27, 28, 29 | UX polish, defaults, docs, test coverage |
| **P4 — Architecture** | 30-52 | Variant usage, O(N²), lock contention, schema hygiene |
| **P5 — Polish** | 53-65 | Install, hooks, docs, minor asymmetry |

---

## Quick Wins (One-Liners / Tiny Patches)

| # | Change | Effort |
|---|--------|--------|
| 1 | `load_settings_sync().expect(...) → ?` | 1 line |
| 7 | `HeaderValue::from_str(...).expect(...) → .map_err(AppError::validation(...))?` | 4 lines × 4 sites |
| 9 | `"0.2.0" → env!("CARGO_PKG_VERSION")` | 1 line |
| 27 | Read `settings.output.default_format` in `resolve_format` | 3 lines |
| 28 | Default `tool_groups` to minimal safe set | 1 line + config |
| 37 | `Selector::parse(...).ok()? → match + warn` | 2 lines × 9 sites |
| 41 | Validate `NewsTestInput.id` non-empty & known | 5 lines |

---

## Appendix: Verified Clean Areas

| Area | Status |
|------|--------|
| SQL injection | ✅ All `rusqlite` calls use `?` parameterization; no `format!` in queries |
| FFI / unsafe | ✅ Zero `unsafe` blocks in source; deps are safe Rust |
| Thread safety | ✅ All shared state `Arc<Mutex<...>>` or `Arc<RwLock<...>>`; `Send+Sync` derives clean |
| TLS | ✅ `openssl-sys` vendored; `reqwest` default-features=false |
| Dependency audit | ⚠️ Run `cargo audit` in CI (not yet integrated) |

---

## Next Steps

1. **Apply P0 patches** (11 fixes, ~50 LOC) — restores crash-free baseline
2. **Implement `validate_public_url` helper** + wire at 7 entry points — closes SSRF
3. **Add `body_limit` + redirect policy** to `HttpClient::new` — closes memory DoS
4. **Run `cargo audit` and add to CI** — ongoing supply-chain hygiene
5. **Fix `tool_groups` default + version skew** — improves fresh-user experience

All findings are from read-only analysis. No files were modified.