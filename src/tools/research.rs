use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;
use crate::tools::types::*;
use crate::types::*;
use chrono::Datelike;

/// Search academic papers across arXiv and Semantic Scholar
pub async fn research_search(input: ResearchSearchInput) -> Result<ResearchSearchOutput, String> {
    let sources = input.sources.unwrap_or_else(|| vec!["arxiv".into(), "semanticscholar".into()]);
    let limit = input.limit.unwrap_or(25).min(100).max(1);
    let query_enc = urlencoding(&input.query);

    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let mut all_papers: Vec<ResearchPaper> = Vec::new();
    let mut total = 0usize;

    // Search arXiv
    if sources.contains(&"arxiv".to_string()) {
        let cat_filter = input.categories.as_ref()
            .map(|cats| cats.iter().map(|c| format!("cat:{}", c)).collect::<Vec<_>>().join("+OR+"))
            .unwrap_or_default();
        let arxiv_query = if cat_filter.is_empty() {
            format!("search_query=all:{}&start=0&max_results={}", query_enc, limit)
        } else {
            format!("search_query=(all:{})+AND+({})&start=0&max_results={}", query_enc, cat_filter, limit)
        };
        let arxiv_url = format!("http://export.arxiv.org/api/query?{}", arxiv_query);

        match http.fetch(&arxiv_url, None, "bypass").await {
            Ok(outcome) => {
                if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                    let body = resp.body_text;
                    if let Ok(feed) = feed_rs::parser::parse(body.as_bytes()) {
                        for entry in &feed.entries {
                            let arxiv_id = entry.id.trim_start_matches("http://arxiv.org/abs/").to_string();
                            let pdf_url = format!("https://arxiv.org/pdf/{}.pdf", arxiv_id);
                            let title = entry.title.as_ref().map(|t| t.content.clone()).unwrap_or_default();
                            let abstract_text = entry.summary.as_ref().map(|s| s.content.clone()).unwrap_or_default();
                            let authors: Vec<String> = entry.authors.iter()
                                .map(|a| a.name.clone())
                                .collect();
                            let year = entry.published.map(|d| d.year() as i32);

                            all_papers.push(ResearchPaper {
                                id: format!("arxiv:{}", arxiv_id),
                                title: title.clone(),
                                authors: authors.clone(),
                                abstract_text: abstract_text.clone(),
                                year,
                                citation_count: None,
                                pdf_url: Some(pdf_url),
                                source: "arXiv".into(),
                                link: Some(entry.links.first().map(|l| l.href.clone()).unwrap_or_default()),
                            });
                        }
                    }

                    if let Some(total_str) = body.split("<opensearch:totalResults").nth(1)
                        .and_then(|s| s.split('>').nth(1))
                        .and_then(|s| s.split('<').next())
                    {
                        total += total_str.parse::<usize>().unwrap_or(0);
                    } else {
                        total += all_papers.len();
                    }
                }
            }
            Err(e) => tracing::warn!("arXiv search failed: {}", e),
        }
    }

    // Search Semantic Scholar
    if sources.contains(&"semanticscholar".to_string()) {
        let ss_query = format!(
            "https://api.semanticscholar.org/graph/v1/paper/search?query={}&limit={}&fields=title,authors,abstract,year,citationCount,openAccessPdf,externalIds",
            query_enc, limit.min(100)
        );
        match http.fetch(&ss_query, None, "bypass").await {
            Ok(outcome) => {
                if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                        if let Some(papers) = json["data"].as_array() {
                            for paper in papers {
                                let paper_id = paper["paperId"].as_str().unwrap_or("");
                                let title = paper["title"].as_str().unwrap_or("");
                                let abstract_text = paper["abstract"].as_str().unwrap_or("");
                                let year = paper["year"].as_i64();
                                let citations = paper["citationCount"].as_i64();
                                let pdf_url = paper["openAccessPdf"]["url"].as_str().map(|s| s.to_string());
                                let authors: Vec<String> = paper["authors"]
                                    .as_array()
                                    .map(|a| {
                                        a.iter()
                                            .filter_map(|author| author["name"].as_str().map(|n| n.to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                all_papers.push(ResearchPaper {
                                    id: format!("semanticscholar:{}", paper_id),
                                    title: title.to_string(),
                                    authors,
                                    abstract_text: abstract_text.to_string(),
                                    year: year.map(|y| y as i32),
                                    citation_count: citations.map(|c| c as i32),
                                    pdf_url,
                                    source: "Semantic Scholar".into(),
                                    link: Some(format!("https://api.semanticscholar.org/{}/{}", paper_id, "CorpusId")),
                                });
                            }
                        }
                        total += json["total"].as_i64().unwrap_or(0) as usize;
                    }
                }
            }
            Err(e) => tracing::warn!("Semantic Scholar search failed: {}", e),
        }
    }

    // Sort by year descending, limit
    all_papers.sort_by(|a, b| b.year.unwrap_or(0).cmp(&a.year.unwrap_or(0)));
    all_papers.truncate(limit as usize);

    let count = all_papers.len();
    Ok(ResearchSearchOutput {
        papers: all_papers,
        count,
        total,
        meta: ResearchSearchMeta {
            query: input.query,
            sources,
        },
    })
}

/// Get detailed information about a specific paper by ID
pub async fn research_paper(input: ResearchPaperInput) -> Result<ResearchPaperOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let paper_id = &input.paper_id;
    let (title, authors, abstract_text, year, citations, references, pdf_url, _content): (String, Vec<String>, String, Option<i32>, Option<i32>, Option<i32>, Option<String>, Option<String>) =
        if paper_id.starts_with("arxiv:") || !paper_id.contains(':') {
            let id = paper_id.trim_start_matches("arxiv:");
            let url = format!("http://export.arxiv.org/api/query?id_list={}", id);
            match http.fetch(&url, None, "bypass").await {
                Ok(outcome) => {
                    if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                        if let Ok(feed) = feed_rs::parser::parse(resp.body_text.as_bytes()) {
                            if let Some(entry) = feed.entries.first() {
                                let t = entry.title.as_ref().map(|t| t.content.clone()).unwrap_or_default();
                                let abs = entry.summary.as_ref().map(|s| s.content.clone()).unwrap_or_default();
                                let auths: Vec<String> = entry.authors.iter().map(|a| a.name.clone()).collect();
                                let yr = entry.published.map(|d| d.year() as i32);
                                (t, auths, abs, yr, None::<i32>, None::<i32>, Some(format!("https://arxiv.org/pdf/{}.pdf", id)), None::<String>)
                            } else {
                                return Err("Paper not found".into());
                            }
                        } else {
                            return Err("Failed to parse arXiv response".into());
                        }
                    } else {
                        return Err("Cached response for paper fetch".into());
                    }
                }
                Err(e) => return Err(format!("arXiv fetch failed: {}", e)),
            }
        } else if paper_id.starts_with("semanticscholar:") {
            let id = paper_id.trim_start_matches("semanticscholar:");
            let url = format!(
                "https://api.semanticscholar.org/graph/v1/paper/{}?fields=title,authors,abstract,year,citationCount,referenceCount,openAccessPdf",
                id
            );
            match http.fetch(&url, None, "bypass").await {
                Ok(outcome) => {
                    if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                            let t = json["title"].as_str().unwrap_or("").to_string();
                            let abs = json["abstract"].as_str().unwrap_or("").to_string();
                            let auths: Vec<String> = json["authors"]
                                .as_array()
                                .map(|a| a.iter().filter_map(|author| author["name"].as_str().map(|n| n.to_string())).collect())
                                .unwrap_or_default();
                            let yr = json["year"].as_i64().map(|y| y as i32);
                            let cites = json["citationCount"].as_i64().map(|c| c as i32);
                            let refs = json["referenceCount"].as_i64().map(|r| r as i32);
                            let pdf = json["openAccessPdf"]["url"].as_str().map(|s| s.to_string());
                            (t, auths, abs, yr, cites, refs, pdf, None)
                        } else {
                            return Err("Failed to parse Semantic Scholar response".into());
                        }
                    } else {
                        return Err("Cached response for paper fetch".into());
                    }
                }
                Err(e) => return Err(format!("Semantic Scholar fetch failed: {}", e)),
            }
        } else {
            return Err("Unknown paper ID format. Use arxiv:XXXX.XXXXX or semanticscholar:XXXX".into());
        };

    // Optionally extract PDF content
    let content = if input.extract_pdf.unwrap_or(false) {
        if let Some(pdf_url_val) = &pdf_url {
            match http.fetch(pdf_url_val, None, "bypass").await {
                Ok(outcome) => {
                    if let http_mod::FetchOutcome::Response(_resp, _, _) = outcome {
                        Some(format!("PDF available at {}. Direct content extraction requires pdf-extractor crate.", pdf_url_val))
                    } else { None }
                }
                Err(_) => None,
            }
        } else { None }
    } else { None };

    Ok(ResearchPaperOutput {
        paper: PaperDetail {
            id: paper_id.clone(),
            title,
            authors,
            abstract_text,
            year,
            citations,
            references,
            pdf_url,
            content,
        },
    })
}

/// Download a research paper PDF
pub async fn research_download(input: ResearchDownloadInput) -> Result<ResearchDownloadOutput, String> {
    let settings = config::load_settings().await.map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    // Determine the PDF URL based on the paper ID
    let pdf_url = if input.paper_id.starts_with("arxiv:") {
        let id = input.paper_id.trim_start_matches("arxiv:");
        format!("https://arxiv.org/pdf/{}.pdf", id)
    } else if input.paper_id.starts_with("semanticscholar:") {
        // For Semantic Scholar, we need to fetch the paper details first to get the PDF URL
        let id = input.paper_id.trim_start_matches("semanticscholar:");
        let url = format!(
            "https://api.semanticscholar.org/graph/v1/paper/{}?fields=openAccessPdf",
            id
        );
        match http.fetch(&url, None, "bypass").await {
            Ok(outcome) => {
                if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                        json["openAccessPdf"]["url"]
                            .as_str()
                            .map(|s| s.to_string())
                            .ok_or_else(|| "No PDF available for this paper".to_string())?
                    } else {
                        return Err("Failed to parse Semantic Scholar response".into());
                    }
                } else {
                    return Err("Cached response for paper details".into());
                }
            }
            Err(e) => return Err(format!("Failed to fetch paper details: {}", e)),
        }
    } else {
        return Err("Unknown paper ID format. Use arxiv:XXXX.XXXXX or semanticscholar:XXXX".into());
    };

    // Download the PDF
    let client = reqwest::Client::new();
    let resp = client
        .get(&pdf_url)
        .header("User-Agent", "igs-mcp/0.1")
        .send()
        .await
        .map_err(|e| format!("Failed to download PDF: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("PDF download failed with status: {}", resp.status()));
    }

    let bytes = resp.bytes().await
        .map_err(|e| format!("Failed to read PDF content: {}", e))?;

    // Determine output path
    let output_path = input.output_path.unwrap_or_else(|| {
        format!("{}.pdf", input.paper_id.replace(":", "_"))
    });

    // Write to file
    std::fs::write(&output_path, &bytes)
        .map_err(|e| format!("Failed to write PDF file: {}", e))?;

    // Create metadata
    let metadata = serde_json::json!({
        "paper_id": input.paper_id,
        "pdf_url": pdf_url,
        "file_size": bytes.len(),
        "format": input.format.unwrap_or_else(|| "pdf".to_string()),
    });

    Ok(ResearchDownloadOutput {
        pdf_path: Some(output_path),
        markdown_path: None,
        file_size: bytes.len() as u64,
        metadata,
    })
}
