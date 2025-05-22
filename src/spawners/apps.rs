// src/spawners/apps.rs
use crate::types::{AsyncResult, ActionResult};
use crate::usage_tracker::UsageStats;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tokio::sync::mpsc as tokio_mpsc;
use crate::utils::LOG_TO_FILE;
use shellexpand;

#[derive(Debug, Clone)]
pub struct DesktopApp {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub comment: Option<String>,
    pub categories: Vec<String>,
    pub no_display: bool,
    pub terminal: bool,
}

impl DesktopApp {
    fn from_desktop_file(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        let mut app = DesktopApp {
            name: String::new(),
            exec: String::new(),
            icon: None,
            comment: None,
            categories: Vec::new(),
            no_display: false,
            terminal: false,
        };

        let mut in_desktop_entry = false;
        for line in content.lines() {
            let line = line.trim();
            
            if line == "[Desktop Entry]" {
                in_desktop_entry = true;
                continue;
            } else if line.starts_with('[') && line.ends_with(']') {
                in_desktop_entry = false;
                continue;
            }
            
            if !in_desktop_entry || line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "Name" => app.name = value.to_string(),
                    "Exec" => app.exec = value.to_string(),
                    "Icon" => app.icon = Some(value.to_string()),
                    "Comment" => app.comment = Some(value.to_string()),
                    "Categories" => {
                        app.categories = value.split(';')
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .collect();
                    },
                    "NoDisplay" => app.no_display = value.to_lowercase() == "true",
                    "Terminal" => app.terminal = value.to_lowercase() == "true",
                    _ => {}
                }
            }
        }

        // Skip if no name or exec, or if NoDisplay is true
        if app.name.is_empty() || app.exec.is_empty() || app.no_display {
            return None;
        }

        Some(app)
    }

    pub fn clean_exec_command(&self) -> String {
        // Remove desktop file field codes like %f, %F, %u, %U, etc.
        let mut cleaned = self.exec.clone();
        
        // Remove common field codes
        let field_codes = ["%f", "%F", "%u", "%U", "%d", "%D", "%n", "%N", "%i", "%c", "%k", "%v", "%m"];
        for code in &field_codes {
            cleaned = cleaned.replace(code, "");
        }
        
        // Clean up extra whitespace
        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

pub fn scan_desktop_files() -> Vec<DesktopApp> {
    let mut apps = Vec::new();
    let mut seen_names = HashMap::new();
    
    // Standard XDG application directories
    let search_paths = [
        "/usr/share/applications",
        "/usr/local/share/applications",
        "~/.local/share/applications",
    ];

    for path_str in &search_paths {
        let expanded_path = shellexpand::tilde(path_str);
        let path = Path::new(expanded_path.as_ref());
        
        if !path.exists() {
            continue;
        }

        LOG_TO_FILE(format!("[APP_SCAN] Scanning directory: {}", path.display()));
        
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                
                if file_path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Some(app) = DesktopApp::from_desktop_file(&file_path) {
                        // Prefer apps from user directories over system directories
                        // (later entries with same name will override earlier ones)
                        if let Some(existing_index) = seen_names.get(&app.name) {
                            apps[*existing_index] = app;
                        } else {
                            seen_names.insert(app.name.clone(), apps.len());
                            apps.push(app);
                        }
                    }
                }
            }
        }
    }

    LOG_TO_FILE(format!("[APP_SCAN] Found {} desktop applications", apps.len()));
    apps
}

pub fn spawn_app_search(query: String, tx: tokio_mpsc::Sender<AsyncResult>) {
    tokio::spawn(async move {
        LOG_TO_FILE(format!("[SPAWNER_APP] Searching apps for: '{}'", query));
        
        let apps = scan_desktop_files();
        let query_lower = query.to_lowercase();
        let usage_stats = UsageStats::new();
        
        let mut matches: Vec<(DesktopApp, i32)> = apps
            .into_iter()
            .filter_map(|app| {
                let name_lower = app.name.to_lowercase();
                let comment_lower = app.comment.as_ref().map(|c| c.to_lowercase()).unwrap_or_default();
                
                // Base scoring system for relevance
                let mut score = 0;
                
                if query.is_empty() {
                    // If no query, show all apps with base score
                    score = 1;
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
                    
                    // Fuzzy matching bonus (simple substring matching)
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
                    
                    LOG_TO_FILE(format!("[SPAWNER_APP] {} -> base_score: {}, usage_boost: {}, final_score: {}", 
                        app.name, score - usage_boost, usage_boost, score));
                    
                    Some((app, score))
                } else {
                    None
                }
            })
            .collect();
        
        // Sort by final score (highest first), then by name for tie-breaking
        matches.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name))
        });
        
        // Limit results to prevent overwhelming UI
        matches.truncate(50);
        
        let results: Vec<ActionResult> = matches
            .into_iter()
            .map(|(app, _score)| ActionResult {
                spawner: "app".to_string(),
                action: "launch".to_string(),
                description: format!("{} - {}", 
                    app.name, 
                    app.comment.as_deref().unwrap_or("")
                ),
                data: format!("{}|{}", app.clean_exec_command(), app.terminal), // Pass both command and terminal flag
            })
            .collect();
        
        LOG_TO_FILE(format!("[SPAWNER_APP] Found {} matching applications", results.len()));
        
        let result_to_send = AsyncResult::PathSearchResult(results);
        if tx.send(result_to_send).await.is_err() {
            LOG_TO_FILE("[SPAWNER_APP] Failed to send app results to main thread".to_string());
        }
    });
}

// Simple fuzzy matching function
fn fuzzy_match(text: &[char], pattern: &[char]) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if text.is_empty() {
        return false;
    }
    
    let mut pattern_idx = 0;
    
    for &ch in text {
        if pattern_idx < pattern.len() && ch == pattern[pattern_idx] {
            pattern_idx += 1;
        }
        if pattern_idx == pattern.len() {
            return true;
        }
    }
    
    pattern_idx == pattern.len()
}