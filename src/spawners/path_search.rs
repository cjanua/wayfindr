// src/spawners/path_search.rs
use crate::types::{AsyncResult, ActionResult};
use std::path::Path;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::process::Command;
use shellexpand;

pub fn spawn_path_search(query: String, tx: tokio_mpsc::Sender<AsyncResult>) {
    tokio::spawn(async move {
        let mut potential_actions: Vec<ActionResult> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

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
                        // Assuming zoxide -s might give "score path" or just "path"
                        // For now, let's take the whole line as path if simple, or parse if scored.
                        // Simple approach: take the path part.
                        let path = line.split_whitespace().last().unwrap_or("").trim().to_string();
                        if !path.is_empty() && Path::new(&path).is_dir() { // Verify it's a directory
                             potential_actions.push(ActionResult {
                                spawner: "z".to_string(),
                                action: "cd".to_string(),
                                description: path.clone(),
                                data: path,
                            });
                        }
                    }
                } else {
                    let err_msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if !err_msg.contains("no match found") && !err_msg.is_empty() {
                        errors.push(format!("Zoxide command failed: {}", err_msg));
                    }
                }
            }
            Err(e) => {
                errors.push(format!("Failed to execute zoxide: {}", e));
            }
        }

        let expanded_query = shellexpand::tilde(&query).into_owned();
        let path = Path::new(&expanded_query);
        if path.is_dir() {
            let direct_path_str = path.to_string_lossy().into_owned();
            // Add if not already added by zoxide with the exact same path
            if !potential_actions.iter().any(|r| r.data == direct_path_str) {
                potential_actions.insert( // Insert at beginning for higher priority
                    0,
                    ActionResult {
                        spawner: "fs".to_string(),
                        action: "cd".to_string(),
                        description: direct_path_str.clone(),
                        data: direct_path_str,
                    },
                );
            }
        }
        
        // Deduplicate, prioritizing entries that came first (fs, then zoxide)
        let mut final_actions = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();
        for action in potential_actions {
            if seen_paths.insert(action.data.clone()) {
                final_actions.push(action);
            }
        }


        let result_to_send = if !final_actions.is_empty() {
            AsyncResult::PathSearchResult(final_actions)
        } else if !errors.is_empty() {
            AsyncResult::Error(errors.join("; "))
        } else {
            AsyncResult::PathSearchResult(Vec::new()) // No results, no errors
        };

        if tx.send(result_to_send).await.is_err() {
            // Log error if sending fails, though not much can be done here
            // Use a generic log if LOG_TO_FILE is not accessible or appropriate here
            // eprintln!("[spawn_path_search] Failed to send result to main thread.");
        }
    });
}