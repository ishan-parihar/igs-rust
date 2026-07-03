use crate::config;
use crate::http::{self as http_mod, HttpClient};
use crate::tools::helpers::urlencoding;
use crate::tools::types::*;
use crate::types::*;
use chrono::Datelike;
use unpdf;

/// Search academic papers across arXiv and Semantic Scholar
pub async fn research_search(input: ResearchSearchInput) -> Result<ResearchSearchOutput, String> {
    let sources = input
        .sources
        .unwrap_or_else(|| vec!["arxiv".into(), "semanticscholar".into()]);
    let limit = input.limit.unwrap_or(25).clamp(1, 100);
    let query_enc = urlencoding(&input.query);

    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let mut all_papers: Vec<ResearchPaper> = Vec::new();
    let mut total = 0usize;

    // Search arXiv
    if sources.contains(&"arxiv".to_string()) {
        let cat_filter = input
            .categories
            .as_ref()
            .map(|cats| {
                cats.iter()
                    .map(|c| format!("cat:{}", c))
                    .collect::<Vec<_>>()
                    .join("+OR+")
            })
            .unwrap_or_default();
        let arxiv_query = if cat_filter.is_empty() {
            format!(
                "search_query=all:{}&start=0&max_results={}",
                query_enc, limit
            )
        } else {
            format!(
                "search_query=(all:{})+AND+({})&start=0&max_results={}",
                query_enc, cat_filter, limit
            )
        };
        let arxiv_url = format!("https://export.arxiv.org/api/query?{}", arxiv_query);

        match http.fetch(&arxiv_url, None, "bypass").await {
            Ok(outcome) => {
                if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                    let body = resp.body_text;
                    if let Ok(feed) = feed_rs::parser::parse(body.as_bytes()) {
                        for entry in &feed.entries {
                            let arxiv_id = entry
                                .id
                                .trim_start_matches("https://arxiv.org/abs/")
                                .to_string();
                            let pdf_url = format!("https://arxiv.org/pdf/{}.pdf", arxiv_id);
                            let title = entry
                                .title
                                .as_ref()
                                .map(|t| t.content.clone())
                                .unwrap_or_default();
                            let abstract_text = entry
                                .summary
                                .as_ref()
                                .map(|s| s.content.clone())
                                .unwrap_or_default();
                            let authors: Vec<String> =
                                entry.authors.iter().map(|a| a.name.clone()).collect();
                            let year = entry.published.map(|d| d.year());

                            all_papers.push(ResearchPaper {
                                id: format!("arxiv:{}", arxiv_id),
                                title: title.clone(),
                                authors: authors.clone(),
                                abstract_text: abstract_text.clone(),
                                year,
                                citation_count: None,
                                pdf_url: Some(pdf_url),
                                source: "arXiv".into(),
                                link: Some(
                                    entry
                                        .links
                                        .first()
                                        .map(|l| l.href.clone())
                                        .unwrap_or_default(),
                                ),
                            });
                        }
                    }

                    if let Some(total_str) = body
                        .split("<opensearch:totalResults")
                        .nth(1)
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
                                let pdf_url = paper["openAccessPdf"]["url"]
                                    .as_str()
                                    .map(|s| s.to_string());
                                let authors: Vec<String> = paper["authors"]
                                    .as_array()
                                    .map(|a| {
                                        a.iter()
                                            .filter_map(|author| {
                                                author["name"].as_str().map(|n| n.to_string())
                                            })
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
                                    link: Some(format!(
                                        "https://api.semanticscholar.org/{}/{}",
                                        paper_id, "CorpusId"
                                    )),
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
    all_papers.sort_by_key(|p| std::cmp::Reverse(p.year.unwrap_or(0)));
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

/// Fetch a paginated list of related papers (citations or references) from Semantic Scholar.
async fn fetch_s2_related_papers(
    paper_id: &str,
    endpoint: &str,
    nested_key: &str,
    http: &HttpClient,
) -> Vec<PaperCitationEntry> {
    let mut all_entries = Vec::new();
    let limit = 1000;
    let max_offset = 10_000i32;
    let mut offset = 0i32;

    loop {
        let url = format!(
            "https://api.semanticscholar.org/graph/v1/paper/{paper_id}/{endpoint}?fields=title,authors,year&limit={limit}&offset={offset}",
        );

        match http.fetch(&url, None, "bypass").await {
            Ok(outcome) => {
                if let http_mod::FetchOutcome::Response(resp, _, _) = outcome {
                    match serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                        Ok(json) => {
                            if let Some(data) = json["data"].as_array() {
                                for item in data {
                                    if let Some(paper) = item[nested_key].as_object() {
                                        let pid = paper
                                            .get("paperId")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let title = paper
                                            .get("title")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let authors: Vec<String> = paper
                                            .get("authors")
                                            .and_then(|a| a.as_array())
                                            .map(|a| {
                                                a.iter()
                                                    .filter_map(|author| {
                                                        author["name"]
                                                            .as_str()
                                                            .map(|n| n.to_string())
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default();
                                        let year = paper
                                            .get("year")
                                            .and_then(|v| v.as_i64())
                                            .map(|y| y as i32);

                                        all_entries.push(PaperCitationEntry {
                                            paper_id: pid,
                                            title,
                                            authors,
                                            year,
                                        });
                                    }
                                }
                            }

                            match json.get("next").and_then(|n| n.as_i64()).map(|n| n as i32) {
                                Some(next_offset)
                                    if next_offset > offset && next_offset < max_offset =>
                                {
                                    offset = next_offset;
                                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                    continue;
                                }
                                _ => break,
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "[igs] Warning: failed to parse S2 {} response: {}",
                                endpoint, e
                            );
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
            Err(e) => {
                eprintln!("[igs] Warning: failed to fetch S2 {}: {}", endpoint, e);
                break;
            }
        }
    }

    all_entries
}

/// Convert PDF bytes to markdown text using unpdf
fn pdf_to_markdown(bytes: &[u8]) -> Result<String, String> {
    let doc = unpdf::parse_reader(bytes).map_err(|e| format!("Failed to parse PDF: {}", e))?;
    let options = unpdf::render::RenderOptions::default();
    let markdown = unpdf::render::to_markdown(&doc, &options)
        .map_err(|e| format!("Failed to render PDF as markdown: {}", e))?;
    Ok(markdown)
}

type PaperFetchResult = (
    String,
    Vec<String>,
    String,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<String>,
    Option<String>,
);

/// Get detailed information about a specific paper by ID
pub async fn research_paper(input: ResearchPaperInput) -> Result<ResearchPaperOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let paper_id = &input.paper_id;
    let (title, authors, abstract_text, year, citations, references, pdf_url, _content): PaperFetchResult =
        if paper_id.starts_with("arxiv:") || !paper_id.contains(':') {
            let id = paper_id.trim_start_matches("arxiv:");
            let url = format!("https://export.arxiv.org/api/query?id_list={}", id);
            match http.fetch(&url, None, "bypass").await {
                Ok(outcome) => {
                    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
                        else { unreachable!("bypass cache mode never returns Cached") };
                    if let Ok(feed) = feed_rs::parser::parse(resp.body_text.as_bytes()) {
                        if let Some(entry) = feed.entries.first() {
                            let t = entry.title.as_ref().map(|t| t.content.clone()).unwrap_or_default();
                            let abs = entry.summary.as_ref().map(|s| s.content.clone()).unwrap_or_default();
                            let auths: Vec<String> = entry.authors.iter().map(|a| a.name.clone()).collect();
                            let yr = entry.published.map(|d| d.year());
                            (t, auths, abs, yr, None::<i32>, None::<i32>, Some(format!("https://arxiv.org/pdf/{}.pdf", id)), None::<String>)
                        } else {
                            return Err("Paper not found".into());
                        }
                    } else {
                        return Err("Failed to parse arXiv response".into());
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
                    let http_mod::FetchOutcome::Response(resp, _, _) = outcome
                        else { unreachable!("bypass cache mode never returns Cached") };
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
                }
                Err(e) => return Err(format!("Semantic Scholar fetch failed: {}", e)),
            }
        } else {
            return Err("Unknown paper ID format. Use arxiv:XXXX.XXXXX or semanticscholar:XXXX".into());
        };

    // Optionally extract PDF content as markdown
    let content = if input.extract_pdf.unwrap_or(false) {
        if let Some(pdf_url_val) = &pdf_url {
            let client = reqwest::Client::builder()
                .user_agent(&settings.http.user_agent)
                .timeout(std::time::Duration::from_millis(settings.http.timeout_ms))
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
            match client.get(pdf_url_val).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.bytes().await {
                            Ok(bytes) => match pdf_to_markdown(&bytes) {
                                Ok(md) => Some(md),
                                Err(e) => {
                                    tracing::warn!("PDF to markdown failed: {}", e);
                                    None
                                }
                            },
                            Err(_) => None,
                        }
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        }
    } else {
        None
    };

    // Determine S2-compatible ID for citation/reference fetching
    let s2_lookup_id = if paper_id.starts_with("arxiv:") {
        format!("arXiv:{}", paper_id.trim_start_matches("arxiv:"))
    } else if paper_id.starts_with("semanticscholar:") {
        paper_id.trim_start_matches("semanticscholar:").to_string()
    } else {
        paper_id.clone()
    };

    // Fetch citations list if requested
    let citations_list = if input.include_citations == Some(true) {
        Some(fetch_s2_related_papers(&s2_lookup_id, "citations", "citingPaper", &http).await)
    } else {
        None
    };

    // Fetch references list if requested (add delay if both were requested to avoid rate limiting)
    let references_list = if input.include_references == Some(true) {
        if input.include_citations == Some(true) {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        Some(fetch_s2_related_papers(&s2_lookup_id, "references", "citedPaper", &http).await)
    } else {
        None
    };

    Ok(ResearchPaperOutput {
        paper: PaperDetail {
            id: paper_id.clone(),
            title,
            authors,
            abstract_text,
            year,
            citations,
            references,
            citations_list,
            references_list,
            pdf_url,
            content,
        },
    })
}

/// Search PubMed for biomedical and life sciences research papers
pub async fn research_pubmed_search(
    input: ResearchPubMedInput,
) -> Result<ResearchPubMedOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
    let cache_dir = http_mod::resolve_cache_dir(&settings, &config::user_config_dir());
    let http = HttpClient::new(&settings.http, &cache_dir);

    let query = urlencoding(&input.query);
    let limit = input.limits.limit.unwrap_or(20).clamp(1, 100);

    let search_url = format!(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&term={}&retmax={}&retmode=json",
        query, limit
    );

    let search_outcome = http
        .fetch(&search_url, None, "bypass")
        .await
        .map_err(|e| format!("PubMed search error: {}", e))?;

    let http_mod::FetchOutcome::Response(search_resp, _, _) = search_outcome
        else { unreachable!("bypass cache mode never returns Cached") };

    let search_data: serde_json::Value = serde_json::from_str(&search_resp.body_text)
        .map_err(|e| format!("JSON parse error: ${e}"))?;

    let pmids: Vec<String> = search_data["esearchresult"]["idlist"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(|id| id.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if pmids.is_empty() {
        return Ok(ResearchPubMedOutput {
            query: input.query,
            total: 0,
            papers: vec![],
        });
    }

    let ids = pmids.join(",");
    let detail_url = format!(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pubmed&id={}&retmode=json",
        ids
    );

    let detail_outcome = http
        .fetch(&detail_url, None, "bypass")
        .await
        .map_err(|e| format!("PubMed detail error: {}", e))?;

    let http_mod::FetchOutcome::Response(detail_resp, _, _) = detail_outcome
        else { unreachable!("bypass cache mode never returns Cached") };

    let detail_data: serde_json::Value = serde_json::from_str(&detail_resp.body_text)
        .map_err(|e| format!("JSON parse error: ${e}"))?;

    let mut papers = Vec::new();
    if let Some(result) = detail_data["result"].as_object() {
        for pmid in &pmids {
            if let Some(paper) = result.get(pmid) {
                let authors = paper["authors"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|a| a["name"].as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                papers.push(ResearchPubMedPaper {
                    pmid: pmid.clone(),
                    title: paper["title"].as_str().unwrap_or("").to_string(),
                    authors,
                    journal: paper["fulljournalname"].as_str().unwrap_or("").to_string(),
                    pub_date: paper["pubdate"].as_str().unwrap_or("").to_string(),
                    url: format!("https://pubmed.ncbi.nlm.nih.gov/{}", pmid),
                });
            }
        }
    }

    Ok(ResearchPubMedOutput {
        query: input.query,
        total: papers.len(),
        papers,
    })
}

/// Download a research paper PDF
pub async fn research_download(
    input: ResearchDownloadInput,
) -> Result<ResearchDownloadOutput, String> {
    let settings = config::load_settings()
        .await
        .map_err(|e| format!("Settings: {}", e))?;
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
                let http_mod::FetchOutcome::Response(resp, _, _) = outcome
                    else { unreachable!("bypass cache mode never returns Cached") };
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.body_text) {
                    json["openAccessPdf"]["url"]
                        .as_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| "No PDF available for this paper".to_string())?
                } else {
                    return Err("Failed to parse Semantic Scholar response".into());
                }
            }
            Err(e) => return Err(format!("Failed to fetch paper details: {}", e)),
        }
    } else {
        return Err("Unknown paper ID format. Use arxiv:XXXX.XXXXX or semanticscholar:XXXX".into());
    };

    // Download the PDF using settings-configured client
    let client = reqwest::Client::builder()
        .user_agent(&settings.http.user_agent)
        .timeout(std::time::Duration::from_millis(settings.http.timeout_ms))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    let resp = client
        .get(&pdf_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download PDF: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "PDF download failed with status: {}",
            resp.status()
        ));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read PDF content: {}", e))?;

    // Determine output path
    let output_path = input
        .output_path
        .unwrap_or_else(|| format!("{}.pdf", input.paper_id.replace(":", "_")));

    // Write PDF to file
    std::fs::write(&output_path, &bytes).map_err(|e| format!("Failed to write PDF file: {}", e))?;

    // Optionally convert to markdown sidecar
    let markdown_path = if input.convert_to_markdown.unwrap_or(false) {
        let md_path = output_path.replace(".pdf", ".md");
        match pdf_to_markdown(&bytes) {
            Ok(md) => {
                if let Err(e) = std::fs::write(&md_path, &md) {
                    tracing::warn!("Failed to write markdown file {}: {}", md_path, e);
                    None
                } else {
                    Some(md_path)
                }
            }
            Err(e) => {
                tracing::warn!("PDF to markdown conversion failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Create metadata
    let metadata = PaperMetadata {
        title: input.paper_id.clone(),
        id: input.paper_id.clone(),
        year: None,
        pages: None,
        file_size: bytes.len() as u64,
        file_path: output_path.clone(),
    };

    Ok(ResearchDownloadOutput {
        pdf_path: Some(output_path),
        markdown_path,
        file_size: bytes.len() as u64,
        metadata,
    })
}
