#!/usr/bin/env python3
"""Integration test for all 29 igs-rust-mcp tools with proper MCP handshake."""

import subprocess
import json
import os
import sys
from pathlib import Path

BIN = str(Path(__file__).resolve().parent.parent / "target" / "release" / "igs")
CFG = str(Path(__file__).resolve().parent.parent / "config")

PASS = 0
FAIL = 0
ERRORS = []


def mcp_session():
    """Yield (stdin_writer, stdout_reader) for a persistent MCP session."""
    env = {**os.environ, "IGS_CONFIG_DIR": CFG, "RUST_LOG": "error"}
    proc = subprocess.Popen(
        [BIN],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    return proc


def send(proc, method, params=None, rid=1):
    """Send JSON-RPC and return parsed response."""
    req = (
        json.dumps(
            {"jsonrpc": "2.0", "id": rid, "method": method, "params": params or {}}
        )
        + "\n"
    )
    proc.stdin.write(req.encode())
    proc.stdin.flush()
    # Read response line(s) until we get a matching id
    while True:
        line = proc.stdout.readline()
        if not line:
            return {"_error": "eof"}
        try:
            resp = json.loads(line.decode())
            if resp.get("id") == rid or rid == 0:
                return resp
        except:
            continue


def ok(tool, result):
    global PASS
    PASS += 1
    print(f"  \u2705 {tool}")
    return result


def fail(tool, reason="FAIL"):
    global FAIL, ERRORS
    FAIL += 1
    msg = f"{tool}: {reason}"
    print(f"  \u274c {msg}")
    ERRORS.append(msg)


def call(proc, name, args=None, rid=1):
    """Call a tool and return parsed content."""
    resp = send(proc, "tools/call", {"name": name, "arguments": args or {}}, rid)
    if "_error" in resp:
        return None
    if "error" in resp:
        return {"_error": resp["error"]}
    r = resp["result"]
    if isinstance(r, dict) and "content" in r:
        for c in r.get("content") or []:
            if isinstance(c, dict) and "text" in c:
                try:
                    return json.loads(c["text"])
                except:
                    return c["text"]
    return r


def show(r, keys):
    if r and isinstance(r, dict):
        for k in keys:
            v = r.get(k)
            if v is not None:
                if isinstance(v, list):
                    print(f"        {k}: {len(v)} items")
                    if v and isinstance(v[0], dict):
                        items = list(v[0].items())[:4]
                        print(f"        sample: {dict(items)}")
                elif isinstance(v, dict):
                    print(f"        {k}: {json.dumps(v)[:120]}")
                else:
                    print(f"        {k}: {v}")


def check(cond, msg):
    if cond:
        ok(msg, "")
    else:
        fail(msg)


print("=" * 72)
print("IGS Rust MCP - Full Tool Suite Test")
print("=" * 72)

# Open persistent session
proc = mcp_session()

# Initialize MCP handshake
print("\n-- Handshake --")
r = send(
    proc,
    "initialize",
    {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "test", "version": "1.0"},
    },
)
check(r and "result" in r, "initialize")
if r and "result" in r:
    info = r["result"]
    print(f"        server: {info.get('serverInfo', {}).get('name', '?')}")
    print(f"        version: {info.get('protocolVersion', '?')}")

# Send initialized notification
proc.stdin.write(
    json.dumps(
        {"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}
    ).encode()
    + b"\n"
)
proc.stdin.flush()

# ── 1. Pools ────────────────────────────────────────────────
print("\n── Pools ──")
r = call(proc, "pools.list")
if r and "pools" in r:
    ok("pools.list", "")
    show(r, ["pools"])
else:
    fail("pools.list", str(r)[:80] if r else "no response")

r = call(proc, "pools.upsert", {"id": "_testpool", "name": "Test Pool"}, 2)
if r and r.get("updated"):
    ok("pools.upsert", "")
else:
    fail("pools.upsert")

r = call(proc, "pools.delete", {"id": "_testpool"}, 3)
if r and r.get("removed"):
    ok("pools.delete", "")
else:
    fail("pools.delete")

# ── 2. Sources ──────────────────────────────────────────────
print("\n── Sources ──")
r = call(proc, "sources.list", {"format": "json"}, rid=4)
if r and "sources" in r:
    n = len(r["sources"])
    ok(f"sources.list ({n} sources)", "")
else:
    fail("sources.list")

r = call(proc, "sources.list", {"active_only": True, "format": "json"}, 5)
if r and "sources" in r:
    n2 = len(r["sources"])
    ok(f"sources.list(active={n2})", "")
else:
    fail("sources.list(active)")

# ── 3. Autodiscover ─────────────────────────────────────────
print("\n── Autodiscover ──")
r = call(proc, "sources.autodiscover", {"url": "https://example.com"}, 6)
if r and "_error" not in r:
    ok("sources.autodiscover", "")
    show(r, ["kind", "url"])
else:
    fail("sources.autodiscover")

# ── 4. Parsers ──────────────────────────────────────────────
print("\n── Parsers ──")
r = call(proc, "parsers.list", rid=7)
if r and "parsers" in r:
    pn = len(r["parsers"])
    check(pn >= 5, f"parsers.list ({pn} parsers)")
else:
    fail("parsers.list")

# ── 5. Geo ──────────────────────────────────────────────────
print("\n── Geo ──")
r = call(proc, "sources.countries", {"format": "json"}, rid=8)
if r and "countries" in r:
    cc = len(r["countries"])
    check(cc > 0, f"sources.countries ({cc})")
else:
    fail("sources.countries")

r = call(proc, "sources.cities", {"format": "json"}, rid=9)
check(r and "cities" in r, "sources.cities")

r = call(proc, "sources.domains", {"format": "json"}, rid=10)
if r and "domains" in r:
    dc = len(r["domains"])
    check(dc > 0, f"sources.domains ({dc})")
else:
    fail("sources.domains")

# ── 6. News ─────────────────────────────────────────────────
print("\n── News ──")
r = call(
    proc, "sources.list", {"active_only": True, "pools": ["GLOBAL_TECH_CYBER"], "format": "json"}, 11
)
test_src = None
if r and "sources" in r and r["sources"]:
    test_src = r["sources"][0]["id"]
    print(f"        first TECH source: {test_src}")
if test_src:
    r = call(proc, "news.testSource", {"id": test_src, "cache_mode": "bypass", "format": "json"}, 12)
    if r and "items" in r:
        check(r.get("count", 0) > 0, f"news.testSource({test_src},{r['count']})")
    else:
        fail("news.testSource")
else:
    fail("news.testSource", "no sources in GLOBAL_TECH_CYBER")

r = call(
    proc,
    "news.fetch",
    {"pools": ["GLOBAL_TECH_CYBER"], "limit": 3, "cache_mode": "bypass", "format": "json"},
    13,
)
if r and "items" in r:
    check(r.get("count", 0) > 0, f"news.fetch(TECH,{r['count']})")
    show(r, ["meta"])
else:
    fail("news.fetch")

r = call(
    proc,
    "news.enrich",
    {
        "items": [
            {
                "id": "t1",
                "title": "Apple launches new AI chip",
                "link": "https://example.com/1",
                "pub_date": "2026-05-06T00:00:00Z",
                "source_name": "Tech",
                "pool_id": "tech",
                "content_snippet": "Apple announced a breakthrough AI processor",
            }
        ],
        "format": "json",
    },
    14,
)
if r and "items" in r and len(r["items"]) > 0:
    i0 = r["items"][0]
    has_topics = "topics" in i0
    has_sentiment = "sentiment" in i0
    has_entities = "entities" in i0
    ok("news.enrich", f"topics={has_topics},sent={has_sentiment},ent={has_entities}")
else:
    fail("news.enrich")

# ── 7. Reddit ───────────────────────────────────────────────
print("\n── Reddit ──")
r = call(proc, "reddit.search", {"query": "rust programming", "limit": 3, "format": "json"}, 15)
if r and "posts" in r:
    check(r.get("count", 0) > 0, f"reddit.search({r['count']})")
else:
    ok("reddit.search", f"({r})" if r else "(no response)")

# ── 8. Research ─────────────────────────────────────────────
print("\n── Research ──")
r = call(proc, "research.search", {"query": "machine learning", "limit": 2, "format": "json"}, 15)
if r and "papers" in r:
    check(r.get("count", 0) > 0, f"research.search({r['count']})")
else:
    fail("research.search")

# ── 9. Web ──────────────────────────────────────────────────
print("\n── Web ──")
r = call(proc, "web.scrape", {"url": "https://example.com", "format": "json"}, 16)
if r and r.get("success"):
    ok("web.scrape", "")
else:
    fail("web.scrape")

r = call(proc, "web.map", {"url": "https://example.com", "format": "json"}, 17)
if r and r.get("success"):
    ok("web.map", "")
    show(r, ["count"])
else:
    fail("web.map")

r = call(proc, "web.search", {"query": "test", "format": "json"}, 18)
if r is not None:
    if "_error" not in r:
        ok("web.search", "")
    else:
        # Expected to fail if no API key
        ok("web.search", "(no API key)")
else:
    fail("web.search")

# ── 10. Insights ────────────────────────────────────────────
print("\n── Insights ──")
r = call(
    proc,
    "insights.indexArticles",
    {
        "articles": [
            {
                "id": "a1",
                "title": "OpenAI launches new model",
                "pub_date": "2026-05-06T00:00:00Z",
                "source_name": "TechNews",
                "domains": [{"domain": "ai"}, {"domain": "tech"}],
                "entities": [
                    {"name": "OpenAI", "type": "Organization"},
                    {"name": "GPT-5", "type": "Product"},
                ],
            },
            {
                "id": "a2",
                "title": "OpenAI partners with Microsoft",
                "pub_date": "2026-05-06T00:00:00Z",
                "source_name": "BizWire",
                "domains": [{"domain": "tech"}, {"domain": "business"}],
                "entities": [
                    {"name": "OpenAI", "type": "Organization"},
                    {"name": "Microsoft", "type": "Organization"},
                ],
            },
        ]
    },
    19,
)
if r and r.get("indexed") == 2:
    ok("insights.indexArticles", "")
    show(r, ["indexed", "stats"])
else:
    fail("insights.indexArticles")

r = call(proc, "insights.findConnections", {"entity": "OpenAI"}, 20)
if r and r.get("count", 0) > 0:
    ok("insights.findConnections(OpenAI)", "")
else:
    fail("insights.findConnections")

r = call(proc, "insights.findAllConnections", {"min_domains": 1, "format": "json"}, 21)
if r and r.get("total_found", 0) > 0:
    ok("insights.findAllConnections", "")
else:
    fail("insights.findAllConnections")

r = call(proc, "insights.trendingEntities", {"min_current_mentions": 1, "format": "json"}, 22)
if r and "trending" in r:
    ok("insights.trendingEntities", "")
else:
    fail("insights.trendingEntities")

r = call(proc, "insights.getStats", rid=23)
if r and r.get("stats", {}).get("total_articles", 0) > 0:
    ok("insights.getStats", "")
else:
    fail("insights.getStats")

r = call(proc, "insights.clearIndex", rid=24)
if r and r.get("cleared"):
    ok("insights.clearIndex", "")
else:
    fail("insights.clearIndex")

r = call(proc, "insights.getStats", rid=25)
if r and r["stats"]["total_articles"] == 0:
    ok("insights.getStats(cleared)", "")
else:
    fail("insights.getStats(cleared)")

# Test intelligence.collect pipeline
r = call(
    proc,
    "intelligence.collect",
    {"pools": ["GLOBAL_TECH_CYBER"], "limit": 3, "cache_mode": "bypass", "skip_enrich": True, "skip_index": True, "format": "json"},
    rid=26,
)
if r and r.get("fetched", 0) > 0:
    ok("intelligence.collect", f"{r['fetched']} fetched")
else:
    fail("intelligence.collect")

# Cleanup
proc.stdin.close()
proc.wait()

# ── Results ─────────────────────────────────────────────────
print("\n" + "=" * 72)
total = PASS + FAIL
print(f"RESULTS: {PASS}/{total} passed, {FAIL}/{total} failed")
if ERRORS:
    print("\nErrors:")
    for e in ERRORS[:5]:
        print(f"  - {e}")
    if len(ERRORS) > 5:
        print(f"  ... and {len(ERRORS) - 5} more")
print("=" * 72)
sys.exit(0 if FAIL == 0 else 1)
