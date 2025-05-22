// src/spawners/unified_search.rs
use crate::types::{AsyncResult, ActionResult};
use crate::usage_tracker::UsageStats;
use crate::spawners::apps::{scan_desktop_files, DesktopApp, fuzzy_match};
use std::collections::HashSet;
use std::path::Path;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::process::Command;
use crate::utils::LOG_TO_FILE;
use shellexpand;

pub fn spawn_unified_search(query: String, tx: tokio_mpsc::Sender<AsyncResult>) {
    tokio::spawn(async move {
        LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Unified search for: '{}'", query));
        
        let mut all_results: Vec<(ActionResult, i32)> = Vec::new(); // Keep scores!
        let _errors: Vec<String> = Vec::new();

        // 1. Search applications first (higher priority)
        let app_results = search_applications(&query).await;
        all_results.extend(app_results);

        // 2. Search directories (lower priority) 
        let dir_results = search_directories(&query).await;
        all_results.extend(dir_results);

        // 3. Sort by score (highest first), apps will naturally rank higher due to usage boosts
        all_results.sort_by(|a, b| {
            b.1.cmp(&a.1) // Sort by score descending
        });

        // 4. Extract just the ActionResults (drop scores)
        let final_results: Vec<ActionResult> = all_results.into_iter().map(|(action, _score)| action).collect();

        // Limit total results to prevent overwhelming UI
        let final_results = if final_results.len() > 50 {
            final_results[..50].to_vec()
        } else {
            final_results
        };

        LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Found {} total results", final_results.len()));

        let result_to_send = AsyncResult::PathSearchResult(final_results);

        if tx.send(result_to_send).await.is_err() {
            LOG_TO_FILE("[SPAWNER_UNIFIED] Failed to send unified results to main thread".to_string());
        }
    });
}

async fn search_applications(query: &str) -> Vec<(ActionResult, i32)> {
    LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Searching applications for: '{}'", query));
    
    let apps = scan_desktop_files();
    let query_lower = query.to_lowercase();
    let usage_stats = UsageStats::new();
    
    let mut matches: Vec<(DesktopApp, i32)> = apps
        .into_iter()
        .filter_map(|app| {
            let name_lower = app.name.to_lowercase();
            let comment_lower = app.comment.as_ref().map(|c| c.to_lowercase()).unwrap_or_default();
            
            // Scoring system for app relevance
            let mut score = 0;
            
            if query.is_empty() {
                // Empty query: show top 5 most-used apps only
                let usage_count = usage_stats.get_usage_count(&app.name);
                if usage_count > 0 {
                    score = usage_count as i32; // Use actual usage count as score for empty query
                    LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Empty query - {} has {} uses", app.name, usage_count));
                }
            } else {
                // Exact name match gets highest score
                if name_lower == query_lower {
                    score = 1000;
                }
                // Name starts with query
                else if name_lower.starts_with(&query_lower) {
                    score = 500;
                }
                // Name contains query
                else if name_lower.contains(&query_lower) {
                    score = 200;
                }
                // Comment contains query
                else if comment_lower.contains(&query_lower) {
                    score = 100;
                }
                // Categories contain query
                else if app.categories.iter().any(|c| c.to_lowercase().contains(&query_lower)) {
                    score = 50;
                }
                
                // Fuzzy matching bonus
                if score == 0 {
                    let name_chars: Vec<char> = name_lower.chars().collect();
                    let query_chars: Vec<char> = query_lower.chars().collect();
                    
                    if fuzzy_match(&name_chars, &query_chars) {
                        score = 25;
                    }
                }
            }
            
            if score > 0 {
                if !query.is_empty() {
                    // Add usage boost to the score for non-empty queries
                    let usage_boost = usage_stats.get_usage_boost(&app.name);
                    score += usage_boost;
                }
                
                Some((app, score))
            } else {
                None
            }
        })
        .collect();
    
    // Sort by score (highest first), then by name
    matches.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name))
    });
    
    // Special limit for empty query (top 5 used apps)
    if query.is_empty() {
        matches.truncate(5);
        LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Empty query - showing top {} most-used apps", matches.len()));
    } else {
        // Normal limit for search queries
        matches.truncate(20);
    }
    
    matches
        .into_iter()
        .map(|(app, score)| (ActionResult {
            spawner: "app".to_string(),
            action: "launch".to_string(),
            description: format!("{} - {}", 
                app.name, 
                app.comment.as_deref().unwrap_or("")
            ),
            data: format!("{}|{}", app.clean_exec_command(), app.terminal),
        }, score))
        .collect()
}

async fn search_directories(query: &str) -> Vec<(ActionResult, i32)> {
    LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Searching directories for: '{}'", query));
    
    // Don't show directories for empty queries - only show top apps
    if query.is_empty() {
        LOG_TO_FILE("[SPAWNER_UNIFIED] Empty query - skipping directory search".to_string());
        return Vec::new();
    }
    
    let mut potential_actions: Vec<ActionResult> = Vec::new();
    
    // 1. Try zoxide first
    match Command::new("zoxide")
        .arg("query")
        .arg("-s") // Add score for potential future sorting
        .arg(&query)
        .output()
        .await
    {
        Ok(output) => {
            if output.status.success() {
                let result_str = String::from_utf8_lossy(&output.stdout);
                for line in result_str.lines() {
                    // Take the path part (zoxide -s gives "score path")
                    let path = line.split_whitespace().last().unwrap_or("").trim().to_string();
                    if !path.is_empty() && Path::new(&path).is_dir() {
                        potential_actions.push(ActionResult {
                            spawner: "z".to_string(),
                            action: "cd".to_string(),
                            description: path.clone(),
                            data: path,
                        });
                    }
                }
            }
        }
        Err(e) => {
            LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Zoxide command failed: {}", e));
        }
    }

    // 2. Try direct path matching
    if !query.is_empty() {
        let expanded_query = shellexpand::tilde(&query).into_owned();
        let path = Path::new(&expanded_query);
        if path.is_dir() {
            let direct_path_str = path.to_string_lossy().into_owned();
            // Add if not already added by zoxide
            if !potential_actions.iter().any(|r| r.data == direct_path_str) {
                potential_actions.insert(
                    0, // Higher priority than zoxide results
                    ActionResult {
                        spawner: "fs".to_string(),
                        action: "cd".to_string(),
                        description: direct_path_str.clone(),
                        data: direct_path_str,
                    },
                );
            }
        }
    }
    
    // Deduplicate directories
    let mut final_actions = Vec::new();
    let mut seen_paths = HashSet::new();
    for action in potential_actions {
        if seen_paths.insert(action.data.clone()) {
            final_actions.push(action);
        }
    }

    // Limit directory results
    final_actions.truncate(15);
    
    LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Found {} directory results", final_actions.len()));
    
    // Convert to (ActionResult, score) tuples - directories get lower scores
    final_actions.into_iter().map(|action| (action, 10)).collect() // Give directories score of 10
}