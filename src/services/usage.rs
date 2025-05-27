// src/services/usage.rs

use crate::config;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEntry {
    pub count: u32,
    pub last_used: DateTime<Utc>,
    pub first_used: DateTime<Utc>,
}

impl UsageEntry {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            count: 1,
            last_used: now,
            first_used: now,
        }
    }

    pub fn increment(&mut self) {
        self.count += 1;
        self.last_used = Utc::now();
    }
}

pub struct UsageService {
    entries: HashMap<String, UsageEntry>,
    file_path: std::path::PathBuf,
    dirty: bool,
}

impl UsageService {
    pub fn new() -> Result<Self> {
        let config = config::get_config();
        let file_path = config.paths.usage_stats_file.clone();

        let mut service = Self {
            entries: HashMap::new(),
            file_path,
            dirty: false,
        };

        service.load()?;
        Ok(service)
    }

    fn load(&mut self) -> Result<()> {
        if !self.file_path.exists() {
            crate::utils::log_info(&format!("Usage stats file doesn't exist yet: {}", self.file_path.display()));
            return Ok(());
        }

        let file = File::open(&self.file_path).context("Failed to open usage stats file")?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line.context("Failed to read line from usage stats")?;
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, json_data)) = line.split_once('|') {
                match serde_json::from_str::<UsageEntry>(json_data) {
                    Ok(entry) => {
                        self.entries.insert(key.to_string(), entry);
                    }
                    Err(e) => {
                        crate::utils::log_warn(&format!(
                            "Failed to parse usage entry for '{}': {}",
                            key, e
                        ));
                    }
                }
            }
        }

        crate::utils::log_info(&format!("Loaded {} usage entries from {}", self.entries.len(), self.file_path.display()));
        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        // Ensure parent directory exists
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create usage stats directory")?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.file_path)
            .context("Failed to open usage stats file for writing")?;

        writeln!(file, "# Wayfindr Usage Statistics")?;
        writeln!(file, "# Format: action_id|json_data")?;
        writeln!(file)?;

        // Sort by usage count (descending) for better readability
        let mut sorted_entries: Vec<_> = self.entries.iter().collect();
        sorted_entries.sort_by(|a, b| b.1.count.cmp(&a.1.count));

        for (key, entry) in sorted_entries {
            let json_data =
                serde_json::to_string(entry).context("Failed to serialize usage entry")?;
            writeln!(file, "{}|{}", key, json_data)?;
        }

        // Ensure data is written to disk
        file.flush().context("Failed to flush usage stats file")?;
        
        self.dirty = false;
        crate::utils::log_debug(&format!("Saved {} usage entries to {}", self.entries.len(), self.file_path.display()));
        Ok(())
    }

    pub fn record_usage(&mut self, action_id: &str) {
        match self.entries.get_mut(action_id) {
            Some(entry) => entry.increment(),
            None => {
                self.entries
                    .insert(action_id.to_string(), UsageEntry::new());
            }
        }
        self.dirty = true;
        
        // Force immediate save to ensure usage is persisted
        if let Err(e) = self.save() {
            crate::utils::log_error(&format!("Failed to save usage stats immediately: {}", e));
        }
        
        crate::utils::log_info(&format!("Recorded usage for '{}' (total: {})", 
            action_id, 
            self.entries.get(action_id).map(|e| e.count).unwrap_or(0)
        ));
    }

    pub fn get_usage_count(&self, action_id: &str) -> u32 {
        self.entries.get(action_id).map(|e| e.count).unwrap_or(0)
    }

    pub fn get_usage_boost(&self, action_id: &str) -> i32 {
        let count = self.get_usage_count(action_id);

        // Exponential boost for frequently used items
        match count {
            0 => 0,
            1..=2 => 50,
            3..=5 => 150,
            6..=10 => 300,
            11..=20 => 500,
            21..=50 => 750,
            _ => 1000,
        }
    }

    pub fn get_top_used(&self, limit: usize) -> Vec<(&String, &UsageEntry)> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| b.1.count.cmp(&a.1.count));
        entries.truncate(limit);
        entries
    }

    pub fn get_top_used_with_counts(&self, limit: usize) -> Vec<(String, u32)> {
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .map(|(id, entry)| (id.clone(), entry.count))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(limit);
        entries
    }

    pub fn cleanup_old_entries(&mut self, days: i64) {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let initial_count = self.entries.len();

        self.entries.retain(|_, entry| entry.last_used > cutoff);

        if self.entries.len() < initial_count {
            self.dirty = true;
            crate::utils::log_info(&format!(
                "Cleaned up {} old usage entries",
                initial_count - self.entries.len()
            ));
        }
    }

    pub fn reset_usage(&mut self, action_id: &str) -> bool {
        let removed = self.entries.remove(action_id).is_some();
        if removed {
            self.dirty = true;
        }
        removed
    }

    pub fn clear_all(&mut self) {
        let count = self.entries.len();
        self.entries.clear();
        if count > 0 {
            self.dirty = true;
            crate::utils::log_info(&format!("Cleared {} usage entries", count));
        }
    }
}

impl Drop for UsageService {
    fn drop(&mut self) {
        if let Err(e) = self.save() {
            crate::utils::log_error(&format!("Failed to save usage stats on drop: {}", e));
        }
    }
}

// Singleton usage service
use std::sync::{Mutex, OnceLock};

static USAGE_SERVICE: OnceLock<Mutex<UsageService>> = OnceLock::new();

pub fn init_usage_service() -> Result<()> {
    let service = UsageService::new()?;
    USAGE_SERVICE
        .set(Mutex::new(service))
        .map_err(|_| anyhow::anyhow!("Usage service already initialized"))?;
    Ok(())
}

pub fn record_usage(action_id: &str) {
    if let Some(service) = USAGE_SERVICE.get() {
        if let Ok(mut service) = service.lock() {
            service.record_usage(action_id);
        } else {
            crate::utils::log_error("Failed to acquire lock on usage service for recording");
        }
    } else {
        crate::utils::log_error("Usage service not initialized");
    }
}

pub fn get_usage_boost(action_id: &str) -> i32 {
    USAGE_SERVICE
        .get()
        .and_then(|service| service.lock().ok())
        .map(|service| service.get_usage_boost(action_id))
        .unwrap_or(0)
}

pub fn get_top_used(limit: usize) -> Vec<String> {
    USAGE_SERVICE
        .get()
        .and_then(|service| service.lock().ok())
        .map(|service| {
            service
                .get_top_used(limit)
                .into_iter()
                .map(|(id, _)| id.clone())
                .collect()
        })
        .unwrap_or_default()
}

pub fn get_usage_count(action_id: &str) -> u32 {
    USAGE_SERVICE
        .get()
        .and_then(|service| service.lock().ok())
        .map(|service| service.get_usage_count(action_id))
        .unwrap_or(0)
}

pub fn get_top_used_with_counts(limit: usize) -> Vec<(String, u32)> {
    USAGE_SERVICE
        .get()
        .and_then(|service| service.lock().ok())
        .map(|service| service.get_top_used_with_counts(limit))
        .unwrap_or_default()
}