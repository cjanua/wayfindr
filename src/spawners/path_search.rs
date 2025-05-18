// src/spawners/path_search.rs
use crate::types::AsyncResult;
use std::path::Path;
use tokio::sync::mpsc as tokio_mpsc; // Using tokio's mpsc
use tokio::process::Command;
use shellexpand;

pub fn spawn_path_search(query: String, tx: tokio_mpsc::Sender<AsyncResult>) {
    tokio::spawn(async move { // This whole block is an async task
        let mut potential_paths: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        // -- Zoxide fuzzy finder check --
        match Command::new("zoxide")
            .arg("query")
            .arg(&query)
            .output()
            .await // This await is for the command output
        {
            Ok(output) => {
                if output.status.success() {
                    let result_str = String::from_utf8_lossy(&output.stdout);
                    let path = result_str.trim().to_string();
                    if !path.is_empty() {
                        potential_paths.push(path);
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

        // -- Direct FS check --
        let expanded_query = shellexpand::tilde(&query).into_owned();
        let path = Path::new(&expanded_query);
        if path.is_dir() {
            let direct_path_str = path.to_string_lossy().into_owned();
            if !potential_paths.contains(&direct_path_str) {
                potential_paths.insert(0, direct_path_str);
            }
        }

        // Determine the result to send
        let result_to_send = if !potential_paths.is_empty() {
            AsyncResult::ZoxideResult(potential_paths)
        } else if !errors.is_empty() {
            AsyncResult::Error(errors.join("; "))
        } else {
            AsyncResult::ZoxideResult(Vec::new())
        };

        // Send the result - THIS IS THE CRITICAL PART - ensure .await is used
        tx.send(result_to_send)
            .await
            .expect("[spawn_path_search] Failed to send result");

    });
}