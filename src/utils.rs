// src/utils.rs
use crate::config::{get_config, LogLevel};
use chrono::Local;
use std::fs::OpenOptions;
use std::io::Write;

pub const DEFAULT_LOG_FILE_PATH: &str = "~/.wayfindr/wayfindr.log";

pub fn log_debug(message: &str) {
    log_with_level(LogLevel::Debug, message);
}

pub fn log_info(message: &str) {
    log_with_level(LogLevel::Info, message);
}

pub fn log_warn(message: &str) {
    log_with_level(LogLevel::Warn, message);
}

pub fn log_error(message: &str) {
    log_with_level(LogLevel::Error, message);
}

fn log_with_level(level: LogLevel, message: &str) {
    let config = get_config();

    // Check if we should log this level
    let should_log = match (&config.general.log_level, &level) {
        (LogLevel::Off, _) => false,
        (LogLevel::Error, LogLevel::Error) => true,
        (LogLevel::Warn, LogLevel::Error | LogLevel::Warn) => true,
        (LogLevel::Info, LogLevel::Error | LogLevel::Warn | LogLevel::Info) => true,
        (LogLevel::Debug, _) => true,
        _ => false,
    };

    if !should_log {
        return;
    }

    let timestamp = Local::now().format("%m/%d %H:%M:%S").to_string();
    let level_str = match level {
        LogLevel::Debug => "DEBUG",
        LogLevel::Info => "INFO",
        LogLevel::Warn => "WARN",
        LogLevel::Error => "ERROR",
        LogLevel::Off => return, // Should never reach here
    };

    let formatted_message = format!("[{}] [{}] {}", timestamp, level_str, message);

    // Try to write to log file
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.paths.log_file)
    {
        let _ = writeln!(file, "{}", formatted_message);
    }

    // For development, also print to stderr for errors/warnings
    if matches!(level, LogLevel::Error | LogLevel::Warn) {
        eprintln!("{}", formatted_message);
    }
}

pub fn generate_id(prefix: &str, content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    let hash = hasher.finish();

    format!("{}_{:x}", prefix, hash)
}

pub fn truncate_text(text: &str, max_length: usize) -> String {
    let text: &str = text.trim();
    if text.len() <= max_length {
        text.to_string()
    } else {
        format!("{}...", &text[..max_length.saturating_sub(3)])
    }
}

pub fn fuzzy_match(text: &str, pattern: &str) -> bool {
    let text_chars: Vec<char> = text.to_lowercase().chars().collect();
    let pattern_chars: Vec<char> = pattern.to_lowercase().chars().collect();

    if pattern_chars.is_empty() {
        return true;
    }
    if text_chars.is_empty() {
        return false;
    }

    let mut pattern_idx = 0;

    for &ch in &text_chars {
        if pattern_idx < pattern_chars.len() && ch == pattern_chars[pattern_idx] {
            pattern_idx += 1;
        }
        if pattern_idx == pattern_chars.len() {
            return true;
        }
    }

    pattern_idx == pattern_chars.len()
}

pub fn calculate_relevance_score(
    query: &str,
    title: &str,
    description: &str,
    categories: &[String],
) -> i32 {
    let query_lower = query.to_lowercase();
    let title_lower = title.to_lowercase();
    let description_lower = description.to_lowercase();

    let mut score = 0;

    // Exact title match gets highest score
    if title_lower == query_lower {
        score += 1000;
    }
    // Title starts with query
    else if title_lower.starts_with(&query_lower) {
        score += 500;
    }
    // Title contains query
    else if title_lower.contains(&query_lower) {
        score += 200;
    }
    // Description contains query
    else if description_lower.contains(&query_lower) {
        score += 100;
    }
    // Category contains query
    else if categories
        .iter()
        .any(|c| c.to_lowercase().contains(&query_lower))
    {
        score += 50;
    }
    // Fuzzy match as last resort
    else if fuzzy_match(&title_lower, &query_lower) {
        score += 25;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match() {
        assert_eq!(fuzzy_match("firefox", "fire"), true);
        assert_eq!(fuzzy_match("firefox", "fox"), true);
        assert_eq!(fuzzy_match("firefox", "xyz"), false);
        assert_eq!(fuzzy_match("", "test"), false);
        assert_eq!(fuzzy_match("test", ""), true);
    }

    #[test]
    fn test_calculate_relevance_score() {
        let categories = vec!["browser".to_string()];

        // Exact match
        assert_eq!(
            calculate_relevance_score("firefox", "firefox", "Web browser", &categories),
            1000
        );

        // Starts with
        assert_eq!(
            calculate_relevance_score("fire", "firefox", "Web browser", &categories),
            500
        );

        // Contains
        assert_eq!(
            calculate_relevance_score("fox", "firefox", "Web browser", &categories),
            200
        );
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("short", 10), "short");
        assert_eq!(truncate_text("this is a very long text", 10), "this is...");
        assert_eq!(truncate_text("12345678901", 10), "1234567...");
    }
}
