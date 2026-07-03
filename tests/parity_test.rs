/// Parity test: verifies that every MCP tool registered in TOOL_GROUPS
/// has a corresponding CLI subcommand accessible via `igs --help`.
///
/// This test prevents the MCP/CLI parity gap from re-opening: if a new
/// MCP tool is added to the registry without a matching CLI subcommand,
/// this test will fail.
///
/// The test works by:
/// 1. Collecting all tool names from `registry::TOOL_GROUPS`
/// 2. Running `igs --help` and `igs <domain> --help` to enumerate CLI subcommands
/// 3. Converting MCP tool names (e.g., "weather.forecast") to CLI command paths
///    (e.g., "weather forecast")
/// 4. Asserting every MCP tool has a CLI equivalent

use igs_rust_mcp::tools::registry::TOOL_GROUPS;
use std::process::Command;

/// Map an MCP tool name (e.g., "weather.forecast") to the CLI command path
/// that should expose it (e.g., "weather forecast"). Some tool names use
/// underscores or different separators in the CLI — this function handles
/// the known mappings.
fn mcp_tool_to_cli_path(tool_name: &str) -> Option<(&str, &str)> {
    // Special cases where CLI subcommand name differs from MCP tool name
    match tool_name {
        "sources.enable_generic_scraper" => return Some(("sources", "enable-scraper")),
        "sources.autodiscover" => return Some(("sources", "discover")),
        "news.test_source" => return Some(("news", "test")),
        "research.pubmed_search" => return Some(("research", "pub-med-search")),
        "research.paper" => return Some(("research", "paper")),
        "research.download" => return Some(("research", "download")),
        "satellite.firms_fires" => return Some(("satellite", "firms-fires")),
        "env.epa_facilities" => return Some(("env", "epa-facilities")),
        "env.epa_emissions" => return Some(("env", "epa-emissions")),
        "legal.search_cases" => return Some(("legal", "search-cases")),
        "legal.case_details" => return Some(("legal", "case-details")),
        "health.cdc_leading_causes" => return Some(("health", "cdc-leading-causes")),
        "health.who_gho" => return Some(("health", "who-gho")),
        "climate.noaa_observations" => return Some(("climate", "noaa-observations")),
        "climate.noaa_stations" => return Some(("climate", "noaa-stations")),
        "politics.fec_candidates" => return Some(("politics", "fec-candidates")),
        "politics.fec_committees" => return Some(("politics", "fec-committees")),
        "insights.find_connections" => return Some(("insights", "find-connections")),
        "insights.trending_entities" => return Some(("insights", "trending-entities")),
        "insights.index_articles" => return Some(("insights", "index-articles")),
        "insights.get_stats" => return Some(("insights", "stats")),
        "insights.clear_index" => return Some(("insights", "clear-index")),
        "browser.wait_for_selector" => return Some(("browser", "wait-for-selector")),
        "youtube.search" => return Some(("youtube", "search")),
        "youtube.metadata" => return Some(("youtube", "metadata")),
        "youtube.subtitles" => return Some(("youtube", "subtitles")),
        "parsers.list" => return Some(("parsers", "")), // top-level command, no subcommand
        // Intelligence tools (P1)
        "news.summarize" => return Some(("intelligence", "summarize")),
        "entities.resolve" => return Some(("intelligence", "resolve-entities")),
        "search.gdelt" => return Some(("intelligence", "gdelt")),
        // Monitor tools — handled via direct manager calls, not #[tool] macro
        "monitor.create" => return Some(("monitor", "create")),
        "monitor.list" => return Some(("monitor", "list")),
        "monitor.delete" => return Some(("monitor", "delete")),
        "monitor.pause" => return Some(("monitor", "pause")),
        "monitor.resume" => return Some(("monitor", "resume")),
        _ => {}
    }

    // Default: split on "." → (domain, subcommand)
    let parts: Vec<&str> = tool_name.splitn(2, '.').collect();
    if parts.len() == 2 {
        Some((parts[0], parts[1]))
    } else {
        None
    }
}

/// Get the list of CLI subcommands for a given domain by running `igs <domain> --help`
fn get_cli_subcommands(domain: &str) -> Vec<String> {
    let bin = std::env::current_exe()
        .expect("failed to get current exe")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("igs");

    let output = Command::new(&bin)
        .arg(domain)
        .arg("--help")
        .output()
        .expect("failed to run `igs <domain> --help`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut subcommands = Vec::new();

    // Parse clap's help output: subcommands are listed as indented entries
    // e.g., "  forecast        Get weather forecast for a location"
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("Usage:") || trimmed.starts_with("Options:")
            || trimmed.starts_with("Commands:") || trimmed.starts_with("--")
            || trimmed.starts_with("help")
        {
            continue;
        }
        // The subcommand name is the first word
        if let Some(name) = trimmed.split_whitespace().next() {
            subcommands.push(name.to_string());
        }
    }

    subcommands
}

#[test]
fn test_every_mcp_tool_has_cli_equivalent() {
    let mut missing: Vec<String> = Vec::new();

    for group in TOOL_GROUPS {
        for tool in group.tools {
            match mcp_tool_to_cli_path(tool) {
                Some((domain, "")) => {
                    // Top-level command with no subcommand (e.g., `igs parsers`)
                    // Just verify the domain command exists by checking `igs --help`
                    let bin = std::env::current_exe()
                        .expect("failed to get current exe")
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .join("igs");
                    let output = Command::new(&bin)
                        .arg("--help")
                        .output()
                        .expect("failed to run `igs --help`");
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if !stdout.lines().any(|line| {
                        let t = line.trim_start();
                        t.starts_with(domain) && (t.len() == domain.len() || t.as_bytes()[domain.len()] == b' ')
                    }) {
                        missing.push(format!("{} → `igs {}` (top-level command not found)", tool, domain));
                    }
                }
                Some((domain, subcmd)) => {
                    let cli_subcommands = get_cli_subcommands(domain);
                    if !cli_subcommands.iter().any(|s| s == subcmd) {
                        missing.push(format!(
                            "{} → `igs {} {}` (available: [{}])",
                            tool,
                            domain,
                            subcmd,
                            cli_subcommands.join(", ")
                        ));
                    }
                }
                None => {
                    missing.push(format!("{} → no CLI mapping defined", tool));
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "MCP/CLI parity gap — {} MCP tools missing CLI equivalents:\n{}\n\
         \nTo fix: add the missing CLI subcommands in src/cli.rs.\n\
         To update mappings: edit mcp_tool_to_cli_path() in this test file.",
        missing.len(),
        missing.join("\n")
    );
}
