// src/interfaces/rofi.rs - Rofi interface implementation
use crate::{
    app::App,
    config::get_config,
    services::usage,
    types::{ActionResult, ActionType, AppResult},
    utils,
};
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use tokio::process::Command as AsyncCommand;

/// Run wayfindr with the rofi interface
pub async fn run_rofi(mut app: App) -> AppResult<()> {
    let rofi = RofiInterface::new();
    rofi.run(&mut app).await
}

pub struct RofiInterface {
    config: RofiConfig,
}

#[derive(Debug, Clone)]
pub struct RofiConfig {
    pub prompt: String,
    pub placeholder: String,
    pub lines: u32,
    pub width: u32,
    pub theme: Option<String>,
    pub case_sensitive: bool,
    pub show_icons: bool,
}

impl Default for RofiConfig {
    fn default() -> Self {
        Self {
            prompt: "wayfindr".to_string(),
            placeholder: "Search apps, directories, or ask AI...".to_string(),
            lines: 12,
            width: 60,
            theme: Some("wayfindr".to_string()),
            case_sensitive: false,
            show_icons: true,
        }
    }
}

impl RofiInterface {
    pub fn new() -> Self {
        Self {
            config: RofiConfig::default(),
        }
    }

    pub async fn run(&self, app: &mut App) -> AppResult<()> {
        utils::log_info("Running wayfindr with rofi interface");

        // Check if rofi is available
        self.check_rofi_available()?;

        // Install theme if it doesn't exist
        self.ensure_theme_installed()?;

        // Gather all results for static mode
        let all_results = self.gather_all_results(app).await?;

        if all_results.is_empty() {
            utils::log_warn("No results found for rofi interface");
            return Ok(());
        }

        // Format results for rofi
        let rofi_entries = self.format_results_for_rofi(&all_results);

        // Execute rofi and get selection
        let selection = self.execute_rofi(&rofi_entries).await?;

        // Parse selection and execute if found
        if let Some(selected_result) = self.parse_selection(&selection, &all_results) {
            utils::log_info(&format!("Rofi selection: {}", selected_result.title));
            
            // Record usage
            usage::record_usage(&selected_result.id);
            
            // Execute the selected action
            let execution_service = crate::services::ExecutionService::new();
            execution_service.execute(&selected_result).await?;
        }

        Ok(())
    }

    fn check_rofi_available(&self) -> AppResult<()> {
        match Command::new("rofi").arg("-version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                utils::log_info(&format!("Found rofi: {}", version.lines().next().unwrap_or("unknown")));
                Ok(())
            }
            _ => Err(crate::types::AppError::ActionExecution(
                "rofi not found. Please install rofi to use this interface.".to_string(),
            )),
        }
    }

    fn ensure_theme_installed(&self) -> AppResult<()> {
        let config = get_config();
        let theme_dir = config.paths.config_dir.join("rofi");
        let theme_file = theme_dir.join("wayfindr.rasi");

        // Create rofi config directory if it doesn't exist
        if !theme_dir.exists() {
            std::fs::create_dir_all(&theme_dir)
                .map_err(|e| crate::types::AppError::ActionExecution(
                    format!("Failed to create rofi theme directory: {}", e)
                ))?;
        }

        // Install theme if it doesn't exist
        if !theme_file.exists() {
            std::fs::write(&theme_file, WAYFINDR_ROFI_THEME)
                .map_err(|e| crate::types::AppError::ActionExecution(
                    format!("Failed to write rofi theme: {}", e)
                ))?;
            utils::log_info(&format!("Installed wayfindr rofi theme to: {}", theme_file.display()));
        }

        Ok(())
    }

    async fn gather_all_results(&self, app: &mut App) -> AppResult<Vec<ActionResult>> {
        let mut all_results = Vec::new();

        // Get top apps first (these have usage data)
        utils::log_debug("Gathering top applications...");
        app.load_initial_results().await;
        all_results.extend(app.results.clone());

        // Get all apps
        utils::log_debug("Gathering all applications...");
        let all_apps_results = app.provider_manager.search_all("apps").await;
        for scored_result in all_apps_results {
            all_results.push(scored_result.result);
        }

        // Deduplicate by ID
        let mut seen_ids = std::collections::HashSet::new();
        all_results.retain(|result| seen_ids.insert(result.id.clone()));

        // Sort by usage boost and relevance
        all_results.sort_by(|a, b| {
            let boost_a = usage::get_usage_boost(&a.id);
            let boost_b = usage::get_usage_boost(&b.id);
            
            // Primary sort: usage boost (higher first)
            match boost_b.cmp(&boost_a) {
                std::cmp::Ordering::Equal => {
                    // Secondary sort: alphabetical by title
                    a.title.cmp(&b.title)
                }
                other => other,
            }
        });

        // Limit results to prevent overwhelming rofi
        all_results.truncate(50);

        utils::log_info(&format!("Gathered {} results for rofi", all_results.len()));
        Ok(all_results)
    }

    fn format_results_for_rofi(&self, results: &[ActionResult]) -> Vec<String> {
        results
            .iter()
            .map(|result| {
                let icon = self.get_result_icon(result);
                let provider_tag = self.get_provider_tag(&result.provider);
                
                // Format: "icon title [TAG]"
                // Simple format that's easy to parse back
                if result.description.is_empty() || result.description == result.title {
                    format!("{} {} [{}]", icon, result.title, provider_tag)
                } else {
                    format!("{} {} - {} [{}]", 
                        icon, 
                        result.title, 
                        self.truncate(&result.description, 40),
                        provider_tag
                    )
                }
            })
            .collect()
    }

    async fn execute_rofi(&self, entries: &[String]) -> AppResult<Option<String>> {
        let config = get_config();
        let theme_file = config.paths.config_dir.join("rofi").join("wayfindr.rasi");
        
        let mut cmd = AsyncCommand::new("rofi");
        cmd.arg("-dmenu")
           .arg("-i") // Case insensitive
           .arg("-p").arg(&self.config.prompt)
           .arg("-mesg").arg(&self.config.placeholder)
           .arg("-lines").arg(self.config.lines.to_string())
           .arg("-width").arg(self.config.width.to_string())
           .arg("-matching").arg("fuzzy")
           .arg("-no-custom") // Only allow selections from the list
           .arg("-format").arg("s"); // Return the selected string

        // Use our custom theme
        if theme_file.exists() {
            cmd.arg("-theme").arg(theme_file);
        }

        cmd.stdin(Stdio::piped())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| crate::types::AppError::ActionExecution(
                format!("Failed to start rofi: {}", e)
            ))?;

        // Send entries to rofi
        if let Some(mut stdin) = child.stdin.take() {
            for entry in entries {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(format!("{}\n", entry).as_bytes()).await
                    .map_err(|e| crate::types::AppError::ActionExecution(
                        format!("Failed to write to rofi stdin: {}", e)
                    ))?;
            }
        }

        let output = child.wait_with_output().await
            .map_err(|e| crate::types::AppError::ActionExecution(
                format!("Failed to read rofi output: {}", e)
            ))?;

        if output.status.success() {
            let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if selection.is_empty() {
                Ok(None)
            } else {
                Ok(Some(selection))
            }
        } else {
            // User cancelled or rofi failed
            Ok(None)
        }
    }

    fn parse_selection(&self, selection: &Option<String>, results: &[ActionResult]) -> Option<ActionResult> {
        let selection = selection.as_ref()?;
        
        // Find the result that matches this formatted string
        let formatted_results = self.format_results_for_rofi(results);
        
        for (i, formatted) in formatted_results.iter().enumerate() {
            if formatted == selection {
                return results.get(i).cloned();
            }
        }
        
        utils::log_warn(&format!("Could not parse rofi selection: {}", selection));
        None
    }

    fn get_result_icon(&self, result: &ActionResult) -> &'static str {
        match &result.action {
            ActionType::Launch { needs_terminal: true } => "⚡",
            ActionType::Launch { needs_terminal: false } => "🚀",
            ActionType::Navigate { .. } => "📁",
            ActionType::AiResponse => "🤖",
            ActionType::Custom { .. } => "⚙️",
        }
    }

    fn get_provider_tag(&self, provider: &str) -> &'static str {
        match provider {
            "applications" => "APP",
            "directories" => "DIR",
            "ai" => "AI",
            "weather" => "WEATHER",
            "news" => "NEWS",
            "sports" => "SPORTS",
            "stocks" => "STOCK",
            _ => "EXT",
        }
    }

    fn truncate(&self, text: &str, max_len: usize) -> String {
        if text.len() <= max_len {
            text.to_string()
        } else {
            format!("{}...", &text[..max_len.saturating_sub(3)])
        }
    }
}

// Custom wayfindr rofi theme
const WAYFINDR_ROFI_THEME: &str = r#"/*
 * Wayfindr Rofi Theme
 * A beautiful, modern theme for the wayfindr launcher
 */

configuration {
    show-icons: false;
    display-drun: "wayfindr";
    disable-history: false;
    click-to-exit: true;
    location: 0;
}

* {
    /* Catppuccin Mocha Colors */
    bg: #1e1e2e;
    bg-alt: #313244;
    bg-selected: #45475a;
    fg: #cdd6f4;
    fg-alt: #6c7086;
    
    primary: #89b4fa;
    secondary: #f38ba8;
    accent: #a6e3a1;
    urgent: #fab387;
    
    border: 2px;
    border-color: @primary;
    border-radius: 8px;
    
    font: "JetBrains Mono 11";
    text-font: "Inter 10";
}

window {
    transparency: "real";
    background-color: @bg;
    border: @border;
    border-color: @border-color;
    border-radius: @border-radius;
    width: 700px;
    location: center;
    anchor: center;
    x-offset: 0;
    y-offset: -100;
}

mainbox {
    background-color: transparent;
    children: [inputbar, message, listview];
    spacing: 8px;
    padding: 16px;
}

inputbar {
    background-color: @bg-alt;
    border-radius: @border-radius;
    padding: 12px 16px;
    children: [prompt, entry];
    border: 1px;
    border-color: @primary;
}

prompt {
    background-color: transparent;
    text-color: @primary;
    font: @font;
    margin: 0 8px 0 0;
}

entry {
    background-color: transparent;
    text-color: @fg;
    font: @text-font;
    placeholder: "Search apps, directories, or ask AI...";
    placeholder-color: @fg-alt;
    cursor: text;
}

message {
    background-color: @bg-alt;
    border-radius: @border-radius;
    padding: 8px 12px;
    margin: 0;
    text-color: @fg-alt;
    font: @text-font;
}

listview {
    background-color: transparent;
    lines: 12;
    columns: 1;
    spacing: 4px;
    cycle: false;
    dynamic: true;
    scrollbar: true;
    scrollbar-width: 4px;
    border: 0;
    margin: 4px 0 0 0;
}

scrollbar {
    background-color: @bg-alt;
    border-radius: 4px;
    handle-color: @primary;
    handle-width: 4px;
}

element {
    background-color: transparent;
    border-radius: @border-radius;
    padding: 10px 12px;
    text-color: @fg;
    margin: 0 0 2px 0;
    border: 1px;
    border-color: transparent;
}

element normal.normal {
    background-color: transparent;
    text-color: @fg;
}

element normal.urgent {
    background-color: @urgent;
    text-color: @bg;
}

element normal.active {
    background-color: @accent;
    text-color: @bg;
}

element selected.normal {
    background-color: @primary;
    text-color: @bg;
    border-color: @primary;
}

element selected.urgent {
    background-color: @urgent;
    text-color: @bg;
}

element selected.active {
    background-color: @accent;
    text-color: @bg;
}

element alternate.normal {
    background-color: transparent;
    text-color: @fg;
}

element alternate.urgent {
    background-color: @urgent;
    text-color: @bg;
}

element alternate.active {
    background-color: @accent;
    text-color: @bg;
}

element-text {
    background-color: transparent;
    text-color: inherit;
    font: @text-font;
    margin: 0;
    padding: 0;
    cursor: pointer;
}

element-icon {
    background-color: transparent;
    size: 20px;
    margin: 0 8px 0 0;
    cursor: pointer;
}

/* Custom styling for different result types */
element-text selected {
    text-color: @bg;
}
"#;