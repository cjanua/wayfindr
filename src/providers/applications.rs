// src/providers/applications.rs
use crate::{
    providers::{ScoredResult, SearchProvider},
    services::usage,
    types::{ActionData, ActionMetadata, ActionResult, ActionType, ProviderError, ProviderResult},
    utils,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
                        app.categories = value
                            .split(';')
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                            .collect();
                    }
                    "NoDisplay" => app.no_display = value.to_lowercase() == "true",
                    "Terminal" => app.terminal = value.to_lowercase() == "true",
                    _ => {}
                }
            }
        }

        if app.name.is_empty() || app.exec.is_empty() || app.no_display {
            return None;
        }

        Some(app)
    }

    fn clean_exec_command(&self) -> String {
        let mut cleaned = self.exec.clone();
        let field_codes = [
            "%f", "%F", "%u", "%U", "%d", "%D", "%n", "%N", "%i", "%c", "%k", "%v", "%m",
        ];
        for code in &field_codes {
            cleaned = cleaned.replace(code, "");
        }
        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

pub struct ApplicationProvider {
    apps: Vec<DesktopApp>,
}

impl ApplicationProvider {
    pub fn new() -> Self {
        Self { apps: Vec::new() }
    }

    fn scan_desktop_files(&mut self) -> Result<(), ProviderError> {
        let mut apps = Vec::new();
        let mut seen_names = HashMap::new();

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

            let entries = fs::read_dir(path).map_err(|e| {
                ProviderError::Command(format!(
                    "Failed to read directory {}: {}",
                    path.display(),
                    e
                ))
            })?;

            for entry in entries.flatten() {
                let file_path = entry.path();

                if file_path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Some(app) = DesktopApp::from_desktop_file(&file_path) {
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

        utils::log_info(&format!("Scanned {} desktop applications", apps.len()));
        self.apps = apps;
        Ok(())
    }
}

#[async_trait]
impl SearchProvider for ApplicationProvider {
    fn id(&self) -> &'static str {
        "applications"
    }

    fn name(&self) -> &str {
        "Applications"
    }

    fn can_handle(&self, query: &str) -> bool {
        // Handle direct app: prefix or empty query (for top apps)
        query.is_empty() ||
        query.starts_with("app:") ||
        query.starts_with("apps") ||
        (!query.starts_with("ai:") && !query.starts_with("ask:")) // Handle general queries unless they're AI queries
    }

    fn priority(&self) -> u8 {
        70 // Higher priority for applications
    }

    async fn search(&self, query: &str) -> ProviderResult<Vec<ScoredResult>> {
        utils::log_info(&format!("ApplicationProvider::search called with query: '{}'", query));

        // Ensure apps are loaded
        let mut provider = self.clone();
        provider.scan_desktop_files()?;

        let processed_query = if query.starts_with("app:") {
            query.strip_prefix("app:").unwrap_or("").trim()
        } else if query == "apps" {
            ""
        } else {
            query
        };

        let mut matches = Vec::new();

        for app in &provider.apps {
            let app_id = utils::generate_id("app", &app.name);
            
            let score = if processed_query.is_empty() {
                // Empty query - check for usage data
                let usage_count = usage::get_usage_count(&app_id);
                let usage_boost = usage::get_usage_boost(&app_id);
                
                utils::log_debug(&format!("App '{}': usage_count={}, usage_boost={}", 
                    app.name, usage_count, usage_boost));
                
                if usage_count > 0 {
                    usage_boost
                } 
                else {
                    // If no apps have usage data, show some popular ones
                    if matches.len() < 5 && is_common_app(&app.name) {
                        20 // Give common apps a base score
                    } else {
                        continue;
                    }
                }
            } else {
                // Calculate relevance score for non-empty queries
                let base_score = utils::calculate_relevance_score(
                    processed_query,
                    &app.name,
                    app.comment.as_deref().unwrap_or(""),
                    &app.categories,
                );
                
                if base_score > 0 {
                    let usage_boost = usage::get_usage_boost(&app_id);
                    base_score + usage_boost
                } else {
                    continue;
                }
            };

            let result = ActionResult {
                id: app_id,
                provider: self.id().to_string(),
                action: ActionType::Launch { needs_terminal: app.terminal },
                title: app.name.clone(),
                description: app.comment.clone().unwrap_or_else(|| format!("Launch {}", app.name)),
                data: ActionData::Command(app.clean_exec_command()),
                metadata: ActionMetadata {
                    icon: app.icon.clone(),
                    category: app.categories.first().cloned(),
                    tags: app.categories.clone(),
                    usage_count: 0,
                    last_used: None,
                },
            };

            matches.push(ScoredResult::new(result, score, self.id().to_string()));
        }

        // Sort by score and limit results
        matches.sort_by(|a, b| b.score.cmp(&a.score));
        
        if processed_query.is_empty() {
            // For empty query, limit to top 5
            matches.truncate(5);
            utils::log_info(&format!("Empty query - returning {} top apps", matches.len()));
            
            // If we still have no matches, force some common apps
            if matches.is_empty() {
                utils::log_info("No apps with usage data found, creating fallback apps");
                return Ok(create_fallback_apps());
            }
        } else {
            // Normal limit for search queries
            matches.truncate(20);
            utils::log_info(&format!("Search query '{}' - returning {} matches", processed_query, matches.len()));
        }

        Ok(matches)
    }

    fn configure(&mut self, _config: &crate::config::Config) {
        // Could use config to set search paths, etc.
    }
}

impl Clone for ApplicationProvider {
    fn clone(&self) -> Self {
        Self {
            apps: self.apps.clone(),
        }
    }
}

impl Default for ApplicationProvider {
    fn default() -> Self {
        Self::new()
    }
}


// Helper function to identify common apps
fn is_common_app(app_name: &str) -> bool {
    let common_apps = [
        "firefox", "chromium", "google-chrome", "chrome",
        "code", "visual-studio-code", "vscode",
        "kitty", "alacritty", "gnome-terminal", "konsole", "xterm",
        "nautilus", "thunar", "dolphin", "pcmanfm",
        "spotify", "vlc", "gimp", "libreoffice",
        "discord", "slack", "telegram", "whatsapp",
        "gedit", "vim", "emacs", "sublime-text",
    ];
    
    let app_lower = app_name.to_lowercase();
    common_apps.iter().any(|&common| app_lower.contains(common))
}

// Create fallback apps when no real apps are found
fn create_fallback_apps() -> Vec<ScoredResult> {
    let fallback_apps = vec![
        ("Firefox", "firefox", "Web Browser", false),
        ("Terminal", "kitty", "Terminal Emulator", true),
        ("Files", "nautilus", "File Manager", false),
        ("Text Editor", "gedit", "Text Editor", false),
        ("Calculator", "gnome-calculator", "Calculator", false),
    ];
    
    fallback_apps.into_iter()
        .enumerate()
        .map(|(i, (title, command, description, terminal))| {
            let result = ActionResult::new_launch(
                utils::generate_id("fallback", command),
                "applications",
                title.to_string(),
                command.to_string(),
                terminal,
            ).with_description(description.to_string());
            
            ScoredResult::new(result, 100 - i as i32, "applications".to_string())
        })
        .collect()
}