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
        
        let mut all_results: Vec<ActionResult> = Vec::new();
        let errors: Vec<String> = Vec::new();

        // 1. Search applications first (higher priority)
        let app_results = search_applications(&query).await;
        all_results.extend(app_results);

        // 2. Search directories (lower priority)
        let dir_results = search_directories(&query).await;
        all_results.extend(dir_results);

        // 3. Sort results: Apps first (by usage + relevance), then directories (by relevance)
        all_results.sort_by(|a, b| {
            // First, separate by type (app vs directory)
            let a_is_app = a.spawner == "app";
            let b_is_app = b.spawner == "app";
            
            match (a_is_app, b_is_app) {
                (true, false) => std::cmp::Ordering::Less,   // Apps come first
                (false, true) => std::cmp::Ordering::Greater, // Directories come second
                _ => {
                    // Same type, sort by description (which contains relevance info)
                    a.description.cmp(&b.description)
                }
            }
        });

        // Limit total results to prevent overwhelming UI
        all_results.truncate(50);

        LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Found {} total results ({} apps, {} directories)", 
            all_results.len(),
            all_results.iter().filter(|r| r.spawner == "app").count(),
            all_results.iter().filter(|r| r.spawner != "app").count()
        ));

        let result_to_send = if !all_results.is_empty() {
            AsyncResult::PathSearchResult(all_results)
        } else if !errors.is_empty() {
            AsyncResult::Error(errors.join("; "))
        } else {
            AsyncResult::PathSearchResult(Vec::new())
        };

        if tx.send(result_to_send).await.is_err() {
            LOG_TO_FILE("[SPAWNER_UNIFIED] Failed to send unified results to main thread".to_string());
        }
    });
}

async fn search_applications(query: &str) -> Vec<ActionResult> {
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
                let usage_count = usage_stats.get_usage_count(&app.name);
                if usage_count > 0 {
                    score = usage_count as i32; // Use usage count as score
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
                // Add usage boost to the score
                let usage_boost = usage_stats.get_usage_boost(&app.name);
                score += usage_boost;
                
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
    
    // Limit app results (save space for directories)
    if query.is_empty() {
        matches.truncate(5);
    } else {
        matches.truncate(20);
    }
    
    matches
        .into_iter()
        .map(|(app, _score)| ActionResult {
            spawner: "app".to_string(),
            action: "launch".to_string(),
            description: format!("{} - {}", 
                app.name, 
                app.comment.as_deref().unwrap_or("")
            ),
            data: format!("{}|{}", app.clean_exec_command(), app.terminal),
        })
        .collect()
}

async fn search_directories(query: &str) -> Vec<ActionResult> {
    LOG_TO_FILE(format!("[SPAWNER_UNIFIED] Searching directories for: '{}'", query));
    
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
    final_actions
}