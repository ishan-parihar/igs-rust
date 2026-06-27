use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;
use crate::tools::types::*;
use chrono::{Duration, Utc};

pub async fn security_cve_search(input: CveSearchInput) -> Result<CveSearchOutput, String> {
    let limit = input.limits.limit.unwrap_or(20).clamp(1, 100);
    let days_back = input.days_back.unwrap_or(30);
    let query_enc = urlencoding(&input.query);

    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let now = Utc::now();
    let start_date = (now - Duration::days(days_back as i64))
        .format("%Y-%m-%dT%H:%M:%S.000")
        .to_string();
    let end_date = now.format("%Y-%m-%dT%H:%M:%S.000").to_string();

    let mut url = format!(
        "https://services.nvd.nist.gov/rest/json/cves/2.0?keywordSearch={}&pubStartDate={}&pubEndDate={}&resultsPerPage={}",
        query_enc, start_date, end_date, limit
    );

    if let Some(ref severity) = input.severity {
        // NVD v2.0 uses cvssV3Severity for CVSS v3 severity filtering
        url = format!("{}&cvssV3Severity={}", url, severity.to_uppercase());
    }

    let mut vulnerabilities: Vec<CveEntry> = Vec::new();
    let mut total = 0usize;

    match http.fetch(&url, None, "bypass").await {
        Ok(outcome) => {
            if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                    total = json["totalResults"].as_i64().unwrap_or(0) as usize;

                    if let Some(vulns) = json["vulnerabilities"].as_array() {
                        for vuln in vulns {
                            let cve = &vuln["cve"];

                            let id = cve["id"].as_str().unwrap_or("").to_string();
                            let published = cve["published"].as_str().unwrap_or("").to_string();
                            let source = cve["sourceIdentifier"].as_str().unwrap_or("").to_string();

                            let description = cve["descriptions"]
                                .as_array()
                                .and_then(|descs| {
                                    descs
                                        .iter()
                                        .find(|d| d["lang"].as_str() == Some("en"))
                                        .and_then(|d| d["value"].as_str())
                                        .map(|s| s.to_string())
                                })
                                .unwrap_or_default();

                            let (cvss_score, severity_str) = extract_cvss_metrics(cve);
                            let affected_products = extract_affected_products(cve);

                            let references = cve["references"]
                                .as_array()
                                .map(|refs| {
                                    refs.iter()
                                        .filter_map(|r| r["url"].as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default();

                            vulnerabilities.push(CveEntry {
                                id,
                                source,
                                published,
                                description,
                                severity: severity_str,
                                cvss_score,
                                affected_products,
                                references,
                            });
                        }
                    }
                }
            }
        }
        Err(e) => return Err(format!("NVD API request failed: {}", e)),
    }

    vulnerabilities.sort_by(|a, b| b.published.cmp(&a.published));

    Ok(CveSearchOutput {
        query: input.query,
        total,
        vulnerabilities,
    })
}

// CVSS v3.1 > v3.0 > v2.0 fallback — NVD stores metrics by version
fn extract_cvss_metrics(cve: &serde_json::Value) -> (Option<f64>, String) {
    if let Some(metrics) = cve["metrics"]["cvssMetricV31"].as_array() {
        if let Some(metric) = metrics.first() {
            let score = metric["cvssData"]["baseScore"].as_f64();
            let severity = metric["cvssData"]["baseSeverity"]
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_string();
            return (score, severity);
        }
    }

    if let Some(metrics) = cve["metrics"]["cvssMetricV30"].as_array() {
        if let Some(metric) = metrics.first() {
            let score = metric["cvssData"]["baseScore"].as_f64();
            let severity = metric["cvssData"]["baseSeverity"]
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_string();
            return (score, severity);
        }
    }

    if let Some(metrics) = cve["metrics"]["cvssMetricV20"].as_array() {
        if let Some(metric) = metrics.first() {
            let score = metric["cvssData"]["baseScore"].as_f64();
            let severity = metric["baseSeverity"]
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_string();
            return (score, severity);
        }
    }

    (None, "UNKNOWN".to_string())
}

fn extract_affected_products(cve: &serde_json::Value) -> Vec<String> {
    let mut products = Vec::new();

    if let Some(nodes) = cve["configurations"].as_array() {
        for node in nodes {
            if let Some(cpe_match) = node["nodes"].as_array() {
                for n in cpe_match {
                    if let Some(matches) = n["cpeMatch"].as_array() {
                        for m in matches {
                            if let Some(cpe) = m["criteria"].as_str() {
                                // CPE: cpe:2.3:part:vendor:product:version:...
                                let parts: Vec<&str> = cpe.split(':').collect();
                                if parts.len() >= 5 {
                                    let vendor = parts[3];
                                    let product = parts[4];
                                    if vendor != "*" && product != "*" {
                                        let entry = format!("{} {}", vendor, product);
                                        if !products.contains(&entry) {
                                            products.push(entry);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    products
}

pub async fn security_advisories(
    input: SecurityAdvisoriesInput,
) -> Result<SecurityAdvisoriesOutput, String> {
    let limit = input.limits.limit.unwrap_or(20).clamp(1, 100);

    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let mut url = format!(
        "https://api.github.com/advisories?affects={}&type=reviewed&per_page={}",
        input.ecosystem, limit
    );

    if let Some(ref severity) = input.severity {
        url = format!("{}&severity={}", url, severity.to_lowercase());
    }

    let mut headers = std::collections::HashMap::new();
    headers.insert(
        "Accept".to_string(),
        "application/vnd.github+json".to_string(),
    );

    let mut advisories: Vec<SecurityAdvisory> = Vec::new();

    match http.fetch(&url, Some(&headers), "bypass").await {
        Ok(outcome) => {
            if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                    if let Some(items) = json.as_array() {
                        for item in items {
                            let ghsa_id = item["ghsa_id"].as_str().unwrap_or("").to_string();
                            let cve_id = item["cve_id"].as_str().map(|s| s.to_string());
                            let summary = item["summary"].as_str().unwrap_or("").to_string();
                            let severity = item["severity"].as_str().unwrap_or("").to_string();
                            let published = item["published_at"].as_str().unwrap_or("").to_string();
                            let updated = item["updated_at"].as_str().unwrap_or("").to_string();

                            let vulnerable_range = item["vulnerabilities"]
                                .as_array()
                                .and_then(|vulns| {
                                    vulns.first().and_then(|v| {
                                        v["vulnerable_version_range"]
                                            .as_str()
                                            .map(|s| s.to_string())
                                    })
                                })
                                .unwrap_or_default();

                            let patched_versions = item["vulnerabilities"]
                                .as_array()
                                .and_then(|vulns| {
                                    vulns.first().and_then(|v| {
                                        let patches = v["patched_versions"].as_array()?;
                                        let versions: Vec<&str> = patches
                                            .iter()
                                            .filter_map(|p| p["identifier"].as_str())
                                            .collect();
                                        if versions.is_empty() {
                                            v["first_patched_version"]["identifier"]
                                                .as_str()
                                                .map(|s| s.to_string())
                                        } else {
                                            Some(versions.join(", "))
                                        }
                                    })
                                })
                                .unwrap_or_default();

                            let references = item["references"]
                                .as_array()
                                .map(|refs| {
                                    refs.iter()
                                        .filter_map(|r| r["url"].as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default();

                            advisories.push(SecurityAdvisory {
                                ghsa_id,
                                cve_id,
                                summary,
                                severity,
                                published,
                                updated,
                                vulnerable_range,
                                patched_versions,
                                references,
                            });
                        }
                    }
                }
            }
        }
        Err(e) => return Err(format!("GitHub Advisory API request failed: {}", e)),
    }

    Ok(SecurityAdvisoryOutput {
        ecosystem: input.ecosystem,
        total: advisories.len(),
        advisories,
    })
}
