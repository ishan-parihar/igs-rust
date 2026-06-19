use super::types::*;

fn built_in_chains() -> Vec<SopChain> {
    vec![
        SopChain {
            name: "deep-threat-intel".into(),
            description: "Full threat intelligence pipeline: search CVEs → fetch news → search web → enrich → index".into(),
            steps: vec![
                SopStep { tool: "security.cve".into(), params: serde_json::json!({"query": "$QUERY", "days_back": 7}), depends_on: None },
                SopStep { tool: "news.fetch".into(), params: serde_json::json!({"pools": ["GLOBAL_TECH_CYBER"], "keywords": "$QUERY", "depth": "deep"}), depends_on: Some(0) },
                SopStep { tool: "web.search".into(), params: serde_json::json!({"query": "$QUERY threat analysis"}), depends_on: None },
                SopStep { tool: "insights.findConnections".into(), params: serde_json::json!({}), depends_on: Some(1) },
            ],
        },
        SopChain {
            name: "academic-deep-dive".into(),
            description: "Research a topic: search papers → get details → search web for context → search news".into(),
            steps: vec![
                SopStep { tool: "research.search".into(), params: serde_json::json!({"query": "$QUERY", "limit": 10}), depends_on: None },
                SopStep { tool: "web.search".into(), params: serde_json::json!({"query": "$QUERY recent developments"}), depends_on: None },
                SopStep { tool: "news.fetch".into(), params: serde_json::json!({"pools": ["GLOBAL_TECH_CYBER", "GLOBAL_SCIENCE"], "keywords": "$QUERY", "limit": 20}), depends_on: None },
                SopStep { tool: "insights.trendingEntities".into(), params: serde_json::json!({"time_window_hours": 48}), depends_on: Some(2) },
            ],
        },
        SopChain {
            name: "regulatory-monitor".into(),
            description: "Track regulatory changes: search bills → search regulations → fetch news → find connections".into(),
            steps: vec![
                SopStep { tool: "govt.bills".into(), params: serde_json::json!({"query": "$QUERY"}), depends_on: None },
                SopStep { tool: "govt.regulations".into(), params: serde_json::json!({"query": "$QUERY"}), depends_on: None },
                SopStep { tool: "news.fetch".into(), params: serde_json::json!({"pools": ["GLOBAL_LAW_REG"], "keywords": "$QUERY"}), depends_on: None },
                SopStep { tool: "insights.findConnections".into(), params: serde_json::json!({"min_domains": 2}), depends_on: Some(2) },
            ],
        },
        SopChain {
            name: "osint-recon".into(),
            description: "Open source reconnaissance: search web → scrape target → search news → find trending entities".into(),
            steps: vec![
                SopStep { tool: "web.search".into(), params: serde_json::json!({"query": "$QUERY", "max_results": 10}), depends_on: None },
                SopStep { tool: "web.scrape".into(), params: serde_json::json!({"url": "$TARGET_URL"}), depends_on: Some(0) },
                SopStep { tool: "news.fetch".into(), params: serde_json::json!({"pools": ["GLOBAL_TECH_CYBER"], "keywords": "$QUERY", "limit": 20}), depends_on: None },
                SopStep { tool: "insights.trendingEntities".into(), params: serde_json::json!({"time_window_hours": 24, "min_current_mentions": 2}), depends_on: Some(2) },
            ],
        },
        SopChain {
            name: "geo-intel-scan".into(),
            description: "Geographic intelligence: search news by country → search web → find connections across domains".into(),
            steps: vec![
                SopStep { tool: "news.fetch".into(), params: serde_json::json!({"countries": ["$COUNTRY"], "limit": 30}), depends_on: None },
                SopStep { tool: "web.search".into(), params: serde_json::json!({"query": "$QUERY $COUNTRY news analysis"}), depends_on: None },
                SopStep { tool: "insights.findConnections".into(), params: serde_json::json!({"min_domains": 2}), depends_on: Some(0) },
                SopStep { tool: "insights.trendingEntities".into(), params: serde_json::json!({"time_window_hours": 24}), depends_on: Some(0) },
            ],
        },
    ]
}

pub fn sop_list() -> SopListOutput {
    let chains = built_in_chains();
    SopListOutput {
        chains: chains.into_iter().map(|c| SopChainInfo {
            name: c.name,
            description: c.description,
            step_count: c.steps.len(),
        }).collect(),
    }
}

pub fn sop_execute(input: SopExecuteInput) -> Result<SopExecuteOutput, String> {
    let chains = built_in_chains();
    let chain = chains.iter()
        .find(|c| c.name == input.chain_name)
        .ok_or_else(|| format!("Unknown chain '{}'. Use sop.list to see available chains.", input.chain_name))?;

    let mut results = Vec::new();

    for (i, step) in chain.steps.iter().enumerate() {
        if let Some(dep) = step.depends_on {
            if dep >= i {
                results.push(SopStepResult {
                    step: i,
                    tool: step.tool.clone(),
                    status: "skipped".into(),
                    output: format!("Invalid dependency: step {} depends on step {}", i, dep),
                });
                continue;
            }
            if let Some(dep_result) = results.get(dep) {
                if dep_result.status != "completed" {
                    results.push(SopStepResult {
                        step: i,
                        tool: step.tool.clone(),
                        status: "skipped".into(),
                        output: format!("Dependency step {} did not complete ({})", dep, dep_result.status),
                    });
                    continue;
                }
            }
        }

        let params_str = serde_json::to_string(&step.params).unwrap_or_default();

        results.push(SopStepResult {
            step: i,
            tool: step.tool.clone(),
            status: "pending_dispatch".into(),
            output: format!("Ready to dispatch: {}({})", step.tool, params_str),
        });
    }

    Ok(SopExecuteOutput {
        chain_name: chain.name.clone(),
        steps_completed: results.iter().filter(|r| r.status == "completed").count(),
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::types_base::OutputOptions;

    #[test]
    fn test_sop_list_returns_chains() {
        let output = sop_list();
        assert!(!output.chains.is_empty());
        assert!(output.chains.iter().any(|c| c.name == "deep-threat-intel"));
    }

    #[test]
    fn test_sop_execute_valid_chain() {
        let input = SopExecuteInput {
            chain_name: "deep-threat-intel".into(),
            overrides: None,
            output: OutputOptions { format: None },
        };
        let result = sop_execute(input).unwrap();
        assert_eq!(result.chain_name, "deep-threat-intel");
        assert_eq!(result.results.len(), 4);
        assert_eq!(result.steps_completed, 0);
    }

    #[test]
    fn test_sop_execute_unknown_chain() {
        let input = SopExecuteInput {
            chain_name: "nonexistent".into(),
            overrides: None,
            output: OutputOptions { format: None },
        };
        let result = sop_execute(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_dependency_validation() {
        let chains = built_in_chains();
        let chain = chains.iter().find(|c| c.name == "deep-threat-intel").unwrap();
        for (i, step) in chain.steps.iter().enumerate() {
            if let Some(dep) = step.depends_on {
                assert!(dep < i, "Step {} depends on step {} which is not before it", i, dep);
            }
        }
    }
}
