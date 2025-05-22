// src/providers/directories.rs
use crate::{
    providers::{ScoredResult, SearchProvider},
    types::{ActionData, ActionMetadata, ActionResult, ActionType, ProviderError, ProviderResult},
    utils,
};
use async_trait::async_trait;
use std::path::Path;
use tokio::process::Command;

pub struct DirectoryProvider;

impl DirectoryProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SearchProvider for DirectoryProvider {
    fn id(&self) -> &'static str {
        "directories"
    }

    fn name(&self) -> &str {
        "Directories"
    }

    fn can_handle(&self, query: &str) -> bool {
        // Don't handle empty queries (leave those to applications)
        // Don't handle AI queries
        !query.is_empty()
            && !query.starts_with("ai:")
            && !query.starts_with("ask:")
            && !query.starts_with("app:")
    }

    fn priority(&self) -> u8 {
        40 // Lower priority than applications
    }

    async fn search(&self, query: &str) -> ProviderResult<Vec<ScoredResult>> {
        let mut results = Vec::new();

        // Try zoxide first
        match self.search_with_zoxide(query).await {
            Ok(mut zoxide_results) => results.append(&mut zoxide_results),
            Err(e) => utils::log_warn(&format!("Zoxide search failed: {}", e)),
        }

        // Try direct path matching
        if let Ok(mut direct_results) = self.search_direct_path(query).await {
            results.append(&mut direct_results);
        }

        // Deduplicate by path
        let mut seen_paths = std::collections::HashSet::new();
        results.retain(|result| {
            if let ActionData::Path(path) = &result.result.data {
                seen_paths.insert(path.clone())
            } else {
                true
            }
        });

        // Sort by score and limit
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(15);

        Ok(results)
    }

    fn configure(&mut self, _config: &crate::config::Config) {
        // Could configure zoxide options, etc.
    }
}

impl DirectoryProvider {
    async fn search_with_zoxide(&self, query: &str) -> ProviderResult<Vec<ScoredResult>> {
        let output = Command::new("zoxide")
            .arg("query")
            .arg("-s")
            .arg(query)
            .output()
            .await
            .map_err(|e| ProviderError::Command(format!("Failed to execute zoxide: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("no match found") {
                return Ok(Vec::new());
            }
            return Err(ProviderError::Command(format!("Zoxide failed: {}", stderr)));
        }

        let result_str = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in result_str.lines() {
            let path = line.split_whitespace().last().unwrap_or("").trim();
            if !path.is_empty() && Path::new(path).is_dir() {
                let action_id = utils::generate_id("dir", path);
                let result = ActionResult {
                    id: action_id,
                    provider: self.id().to_string(),
                    action: ActionType::Navigate {
                        path: path.to_string(),
                    },
                    title: path.to_string(),
                    description: format!("Navigate to {}", path),
                    data: ActionData::Path(path.to_string()),
                    metadata: ActionMetadata {
                        icon: Some("folder".to_string()),
                        category: Some("directory".to_string()),
                        tags: vec!["directory".to_string(), "zoxide".to_string()],
                        usage_count: 0,
                        last_used: None,
                    },
                };

                // Score based on zoxide ranking (higher is better)
                let score = 100; // Base score for zoxide matches

                results.push(ScoredResult::new(result, score, self.id().to_string()));
            }
        }

        Ok(results)
    }

    async fn search_direct_path(&self, query: &str) -> ProviderResult<Vec<ScoredResult>> {
        let expanded_query = shellexpand::tilde(query).into_owned();
        let path = Path::new(&expanded_query);

        if path.is_dir() {
            let action_id = utils::generate_id("dir", &expanded_query);
            let result = ActionResult {
                id: action_id,
                provider: self.id().to_string(),
                action: ActionType::Navigate {
                    path: expanded_query.clone(),
                },
                title: expanded_query.clone(),
                description: format!("Navigate to {}", expanded_query),
                data: ActionData::Path(expanded_query),
                metadata: ActionMetadata {
                    icon: Some("folder".to_string()),
                    category: Some("directory".to_string()),
                    tags: vec!["directory".to_string(), "direct".to_string()],
                    usage_count: 0,
                    last_used: None,
                },
            };

            // Direct path matches get higher priority
            let score = 150;

            Ok(vec![ScoredResult::new(
                result,
                score,
                self.id().to_string(),
            )])
        } else {
            Ok(Vec::new())
        }
    }
}

impl Default for DirectoryProvider {
    fn default() -> Self {
        Self::new()
    }
}
