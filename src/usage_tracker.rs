// src/usage_tracker.rs
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use crate::utils::LOG_TO_FILE;

const USAGE_FILE_PATH: &str = "/tmp/wayfindr_usage_stats.txt";

#[derive(Debug, Clone)]
pub struct UsageStats {
    counts: HashMap<String, u32>,
    file_path: String,
}

impl UsageStats {
    pub fn new() -> Self {
        let mut stats = UsageStats {
            counts: HashMap::new(),
            file_path: USAGE_FILE_PATH.to_string(),
        };
        stats.load_from_file();
        stats
    }

    pub fn with_custom_path(path: &str) -> Self {
        let mut stats = UsageStats {
            counts: HashMap::new(),
            file_path: path.to_string(),
        };
        stats.load_from_file();
        stats
    }

    fn load_from_file(&mut self) {
        let path = Path::new(&self.file_path);
        if !path.exists() {
            LOG_TO_FILE(format!("[USAGE_STATS] Usage file doesn't exist yet: {}", self.file_path));
            return;
        }

        match File::open(path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                let mut loaded_count = 0;
                
                for line in reader.lines() {
                    if let Ok(line) = line {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        
                        if let Some((app_name, count_str)) = line.split_once('|') {
                            if let Ok(count) = count_str.parse::<u32>() {
                                self.counts.insert(app_name.to_string(), count);
                                loaded_count += 1;
                            }
                        }
                    }
                }
                
                LOG_TO_FILE(format!("[USAGE_STATS] Loaded {} usage entries from {}", loaded_count, self.file_path));
            }
            Err(e) => {
                LOG_TO_FILE(format!("[USAGE_STATS] Failed to load usage file {}: {}", self.file_path, e));
            }
        }
    }

    fn save_to_file(&self) {
        match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.file_path)
        {
            Ok(mut file) => {
                // Write header
                if let Err(e) = writeln!(file, "# Wayfindr Application Usage Statistics") {
                    LOG_TO_FILE(format!("[USAGE_STATS] Failed to write header: {}", e));
                    return;
                }
                if let Err(e) = writeln!(file, "# Format: app_name|usage_count") {
                    LOG_TO_FILE(format!("[USAGE_STATS] Failed to write format line: {}", e));
                    return;
                }
                if let Err(e) = writeln!(file, "") {
                    LOG_TO_FILE(format!("[USAGE_STATS] Failed to write empty line: {}", e));
                    return;
                }

                // Write data sorted by usage count (descending)
                let mut sorted_apps: Vec<(&String, &u32)> = self.counts.iter().collect();
                sorted_apps.sort_by(|a, b| b.1.cmp(a.1));

                let mut written_count = 0;
                for (app_name, count) in sorted_apps {
                    if let Err(e) = writeln!(file, "{}|{}", app_name, count) {
                        LOG_TO_FILE(format!("[USAGE_STATS] Failed to write entry {}: {}", app_name, e));
                    } else {
                        written_count += 1;
                    }
                }

                LOG_TO_FILE(format!("[USAGE_STATS] Saved {} usage entries to {}", written_count, self.file_path));
            }
            Err(e) => {
                LOG_TO_FILE(format!("[USAGE_STATS] Failed to open usage file for writing {}: {}", self.file_path, e));
            }
        }
    }

    pub fn increment_usage(&mut self, app_name: &str) {
        let count = self.counts.entry(app_name.to_string()).or_insert(0);
        *count += 1;
        
        LOG_TO_FILE(format!("[USAGE_STATS] Incremented usage for '{}' to {}", app_name, count));
        
        // Save immediately to persist changes
        self.save_to_file();
    }

    pub fn get_usage_count(&self, app_name: &str) -> u32 {
        self.counts.get(app_name).copied().unwrap_or(0)
    }

    pub fn get_usage_boost(&self, app_name: &str) -> i32 {
        let count = self.get_usage_count(app_name);
        
        // Usage boost scoring:
        // 0 uses = 0 points
        // 1-2 uses = 10 points  
        // 3-5 uses = 25 points
        // 6-10 uses = 50 points
        // 11-20 uses = 100 points
        // 21-50 uses = 250 points
        // 51+ uses = 500 points
        
        match count {
            0 => 0,
            1..=2 => 10,
            3..=5 => 25,
            6..=10 => 50,
            11..=20 => 100,
            21..=50 => 250,
            _ => 500,
        }
    }

    pub fn get_top_apps(&self, limit: usize) -> Vec<(String, u32)> {
        let mut sorted_apps: Vec<(String, u32)> = self.counts
            .iter()
            .map(|(name, count)| (name.clone(), *count))
            .collect();
        
        sorted_apps.sort_by(|a, b| b.1.cmp(&a.1));
        sorted_apps.truncate(limit);
        sorted_apps
    }

    pub fn reset_usage(&mut self, app_name: &str) {
        if self.counts.remove(app_name).is_some() {
            LOG_TO_FILE(format!("[USAGE_STATS] Reset usage count for '{}'", app_name));
            self.save_to_file();
        }
    }

    pub fn clear_all_usage(&mut self) {
        let count = self.counts.len();
        self.counts.clear();
        LOG_TO_FILE(format!("[USAGE_STATS] Cleared all {} usage entries", count));
        self.save_to_file();
    }

    // Debug function to print current stats
    pub fn print_stats(&self) {
        LOG_TO_FILE(format!("[USAGE_STATS] Current usage statistics:"));
        let top_apps = self.get_top_apps(10);
        for (app, count) in top_apps {
            LOG_TO_FILE(format!("[USAGE_STATS]   {} -> {} uses (boost: {})", app, count, self.get_usage_boost(&app)));
        }
    }
}