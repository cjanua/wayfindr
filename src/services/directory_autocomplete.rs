// src/services/directory_autocomplete.rs - Directory path completion
use crate::types::ActionResult;
use crate::utils;
use std::path::Path;
use std::fs;

pub struct DirectoryAutocomplete;

impl DirectoryAutocomplete {
    pub fn new() -> Self {
        Self
    }

    /// Get directory completions for a partial path
    pub fn get_completions(&self, input: &str) -> Vec<ActionResult> {
        if input.is_empty() {
            return self.get_common_directories();
        }

        // Check if this looks like a path
        if self.looks_like_path(input) {
            self.complete_path(input)
        } else {
            Vec::new()
        }
    }

    fn looks_like_path(&self, input: &str) -> bool {
        input.starts_with('/') || 
        input.starts_with("~/") || 
        input.starts_with("./") || 
        input.starts_with("../") ||
        input.contains('/')
    }

    fn complete_path(&self, input: &str) -> Vec<ActionResult> {
        let expanded = shellexpand::tilde(input).into_owned();
        let path = Path::new(&expanded);
        
        if expanded.ends_with('/') {
            // User typed "/path/to/dir/" - show contents of that dir
            self.scan_directory(path, "", input.starts_with("~/"))
        } else {
            // User typed "/path/to/par" - show completions starting with "par"
            match path.parent() {
                Some(parent) => {
                    // Create a longer lived value for the filename
                    let filename = path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    self.scan_directory(parent, &filename, input.starts_with("~/"))
                },
                None => self.scan_directory(path, "", input.starts_with("~/")),
            }
        }
    }

    fn scan_directory(&self, dir: &Path, partial: &str, tilde_prefix: bool) -> Vec<ActionResult> {
        let mut results = Vec::new();

        if !dir.exists() || !dir.is_dir() {
            return results;
        }

        match fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    
                    // Only include directories
                    if !entry_path.is_dir() {
                        continue;
                    }

                    let name = entry_path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy();

                    // Skip hidden directories unless user is explicitly typing them
                    if name.starts_with('.') && !partial.starts_with('.') {
                        continue;
                    }

                    // Filter by partial match
                    if !partial.is_empty() && !name.to_lowercase().starts_with(&partial.to_lowercase()) {
                        continue;
                    }

                    // Create the full path for display
                    let display_path = if tilde_prefix {
                        // Convert back to ~ notation if originally had it
                        if let Some(home) = std::env::var("HOME").ok() {
                            let full_path = entry_path.to_string_lossy();
                            if full_path.starts_with(&home) {
                                full_path.replacen(&home, "~", 1)
                            } else {
                                full_path.to_string()
                            }
                        } else {
                            entry_path.to_string_lossy().to_string()
                        }
                    } else {
                        entry_path.to_string_lossy().to_string()
                    };

                    let result = ActionResult::new_navigate(
                        utils::generate_id("autocomplete", &display_path),
                        "directories",
                        name.to_string(),
                        entry_path.to_string_lossy().to_string(),
                    ).with_description(format!("Navigate to {}", display_path));

                    results.push(result);
                }
            }
            Err(e) => {
                utils::log_debug(&format!("Failed to read directory {}: {}", dir.display(), e));
            }
        }

        // Sort by name
        results.sort_by(|a, b| a.title.cmp(&b.title));
        
        // Limit results
        results.truncate(20);

        results
    }

    fn get_common_directories(&self) -> Vec<ActionResult> {
        let common_dirs = vec![
            ("~", "Home"),
            ("~/Documents", "Documents"),
            ("~/Downloads", "Downloads"), 
            ("~/Desktop", "Desktop"),
            ("~/Pictures", "Pictures"),
            ("~/Videos", "Videos"),
            ("~/Music", "Music"),
            ("~/Dev", "Development"),
            ("~/Projects", "Projects"),
            ("/tmp", "Temporary"),
            ("/usr", "System Programs"),
            ("/etc", "Configuration"),
            ("/var", "Variable Data"),
        ];

        common_dirs
            .into_iter()
            .filter_map(|(path, name)| {
                let expanded = shellexpand::tilde(path).into_owned();
                if Path::new(&expanded).exists() {
                    Some(ActionResult::new_navigate(
                        utils::generate_id("common_dir", path),
                        "directories",
                        name.to_string(),
                        expanded,
                    ).with_description(format!("Navigate to {}", path)))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for DirectoryAutocomplete {
    fn default() -> Self {
        Self::new()
    }
}