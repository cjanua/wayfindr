// src/interfaces/rofi.rs - Cleaned up to use system rofi theme
use crate::{
    app::App,
    services::usage,
    types::{ActionResult, ActionType, AppResult},
    utils,
};
use std::process::Command;
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
            utils::log_info(&format!("Rofi selection: {} ({})", selected_result.title, selected_result.provider));
            
            self.handle_selection(&selected_result, app).await?;
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

    async fn gather_all_results(&self, app: &mut App) -> AppResult<Vec<ActionResult>> {
        let mut all_results: Vec<ActionResult> = Vec::new();

        // Get top apps first (these have usage data)
        utils::log_debug("Gathering top applications...");
        app.load_initial_results().await;
        all_results.extend(app.results.clone());

        // Get all apps with search
        utils::log_debug("Gathering all applications...");
        let all_apps_results = app.provider_manager.search_all("apps").await;
        for scored_result in all_apps_results {
            all_results.push(scored_result.result);
        }

        // Get directory results from zoxide and direct paths
        utils::log_debug("Gathering directories from zoxide...");
        let dir_provider = app.provider_manager.get_provider("directories");
        if let Some(provider) = dir_provider {
            let dir_searches = vec!["", "Documents", "Downloads", "dev", "repos", "home"];
            for search_term in dir_searches {
                if let Ok(dir_results) = provider.search(search_term).await {
                    for scored_result in dir_results {
                        all_results.push(scored_result.result);
                    }
                }
            }
        }

        // Add common directories as fallback
        utils::log_debug("Adding common directory shortcuts...");
        let common_dirs = vec![
            ("~", "Home Directory"),
            ("~/Documents", "Documents"),
            ("~/Downloads", "Downloads"),
            ("~/Desktop", "Desktop"),
            ("~/Pictures", "Pictures"),
            ("~/Videos", "Videos"),
            ("~/Music", "Music"),
            ("/tmp", "Temporary Files"),
            ("/", "Root Directory"),
        ];
        
        for (path, description) in common_dirs {
            let expanded_path = shellexpand::tilde(path).into_owned();
            if std::path::Path::new(&expanded_path).exists() {
                let dir_result = ActionResult::new_navigate(
                    utils::generate_id("dir", path),
                    "directories",
                    description.to_string(),
                    expanded_path,
                ).with_description(format!("Navigate to {}", path));
                all_results.push(dir_result);
            }
        }

        // Add AI helper entries (only if API key exists)
        if std::env::var("GEMINI_API_KEY").is_ok() {
            utils::log_debug("Adding AI helper entries...");
            let ai_helpers = vec![
                ("Quick Math", "ai: 2+2", "Ask AI to calculate something"),
                ("What's the weather?", "ai: what's the weather today", "Get weather information"),
                ("Define a word", "ai: define artificial intelligence", "Ask AI to define a word"),
                ("Explain concept", "ai: explain quantum computing", "Ask AI to explain something"),
                ("General question", "ai: hello", "Ask AI anything"),
            ];
            
            for (title, query, description) in ai_helpers {
                let ai_result = ActionResult {
                    id: utils::generate_id("ai_helper", query),
                    provider: "ai_helper".to_string(),
                    action: crate::types::ActionType::Custom { action_id: "ai_query".to_string() },
                    title: title.to_string(),
                    description: description.to_string(),
                    data: crate::types::ActionData::Text(query.to_string()),
                    metadata: crate::types::ActionMetadata {
                        icon: Some("🤖".to_string()),
                        category: Some("ai".to_string()),
                        tags: vec!["ai".to_string(), "helper".to_string()],
                        usage_count: 0,
                        last_used: None,
                    },
                };
                all_results.push(ai_result);
            }
        }

        // Add provider-specific entries
        let provider_helpers = vec![
            ("Weather", "weather", "Get weather information", "☁️"),
            ("News", "news", "Get latest news", "📰"),
            ("Sports", "sports", "Get sports updates", "🏆"),
            ("Stocks", "stocks", "Get stock information", "📈"),
        ];
        
        for (title, command, description, icon) in provider_helpers {
            let helper_result = ActionResult {
                id: utils::generate_id("helper", command),
                provider: "helper".to_string(),
                action: crate::types::ActionType::Custom { action_id: command.to_string() },
                title: title.to_string(),
                description: format!("{} - Use: {}", description, command),
                data: crate::types::ActionData::Text(command.to_string()),
                metadata: crate::types::ActionMetadata {
                    icon: Some(icon.to_string()),
                    category: Some("helper".to_string()),
                    tags: vec!["helper".to_string(), command.to_string()],
                    usage_count: 0,
                    last_used: None,
                },
            };
            all_results.push(helper_result);
        }

        // Deduplicate by ID
        let mut seen_ids = std::collections::HashSet::new();
        all_results.retain(|result| seen_ids.insert(result.id.clone()));

        // Sort by provider priority and usage boost
        all_results.sort_by(|a, b| {
            let boost_a = usage::get_usage_boost(&a.id);
            let boost_b = usage::get_usage_boost(&b.id);
            
            let priority_a = match a.provider.as_str() {
                "applications" => 1000 + boost_a,
                "directories" => 500,
                "ai_helper" => 300,
                "helper" => 200,
                _ => 100,
            };
            let priority_b = match b.provider.as_str() {
                "applications" => 1000 + boost_b,
                "directories" => 500,
                "ai_helper" => 300,
                "helper" => 200,
                _ => 100,
            };
            
            match priority_b.cmp(&priority_a) {
                std::cmp::Ordering::Equal => a.title.cmp(&b.title),
                other => other,
            }
        });

        // Limit results to prevent overwhelming rofi
        all_results.truncate(80);

        utils::log_info(&format!("Gathered {} results for rofi", all_results.len()));
        
        Ok(all_results)
    }

    fn format_results_for_rofi(&self, results: &[ActionResult]) -> Vec<String> {
        results
            .iter()
            .map(|result| {
                let icon = self.get_result_icon(result);
                let provider_tag = self.get_provider_tag(&result.provider);
                
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

        // Use system rofi theme (selected via 'rofi theme selector')
        // No explicit theme argument - let rofi use the user's configured theme

        cmd.stdin(std::process::Stdio::piped())
           .stdout(std::process::Stdio::piped())
           .stderr(std::process::Stdio::piped());

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

    async fn handle_selection(&self, selected_result: &ActionResult, app: &mut App) -> AppResult<()> {
        match &selected_result.action {
            crate::types::ActionType::Custom { action_id } if selected_result.provider == "ai_helper" => {
                self.handle_ai_query(selected_result, app).await
            }
            crate::types::ActionType::Custom { action_id } if selected_result.provider == "helper" => {
                self.handle_helper_query(selected_result, app).await
            }
            _ => {
                // Handle normal actions (apps, directories, etc.)
                usage::record_usage(&selected_result.id);
                let execution_service = crate::services::ExecutionService::new();
                execution_service.execute(&selected_result).await?;
                Ok(())
            }
        }
    }

    async fn handle_ai_query(&self, result: &ActionResult, app: &mut App) -> AppResult<()> {
        if let crate::types::ActionData::Text(ref ai_query) = result.data {
            utils::log_info(&format!("Executing AI query: {}", ai_query));
            
            let ai_results = app.provider_manager.search_all(ai_query).await;
            
            if let Some(ai_result) = ai_results.first() {
                usage::record_usage(&ai_result.result.id);
                
                if let crate::types::ActionData::Text(ref ai_response) = ai_result.result.data {
                    // Show AI response in a new rofi dialog
                    let display_entries = vec![
                        format!("🤖 AI: {}", ai_query.strip_prefix("ai: ").unwrap_or(ai_query)),
                        "".to_string(),
                        format!("💬 {}", ai_response),
                        "".to_string(),
                        "Press Enter to close".to_string(),
                    ];
                    let _ = self.execute_rofi(&display_entries).await;
                }
            } else {
                let error_entries = vec![
                    "❌ No AI response received".to_string(),
                    "Check your GEMINI_API_KEY".to_string(),
                    "Press Enter to close".to_string(),
                ];
                let _ = self.execute_rofi(&error_entries).await;
            }
        }
        Ok(())
    }

    async fn handle_helper_query(&self, result: &ActionResult, app: &mut App) -> AppResult<()> {
        let query = match result.data {
            crate::types::ActionData::Text(ref text) => text.clone(),
            _ => return Ok(()),
        };
        
        utils::log_info(&format!("Executing search for: {}", query));
        
        let search_results = app.provider_manager.search_all(&query).await;
        
        if search_results.is_empty() {
            utils::log_warn(&format!("No results found for: {}", query));
            return Ok(());
        }
        
        if search_results.len() == 1 {
            let result = &search_results[0].result;
            usage::record_usage(&result.id);
            let execution_service = crate::services::ExecutionService::new();
            execution_service.execute(result).await?;
        } else {
            // Multiple results - show them in a second rofi instance
            let sub_results: Vec<ActionResult> = search_results.into_iter()
                .map(|sr| sr.result)
                .collect();
            
            let sub_entries = self.format_results_for_rofi(&sub_results);
            let sub_selection = self.execute_rofi(&sub_entries).await?;
            
            if let Some(final_result) = self.parse_selection(&sub_selection, &sub_results) {
                usage::record_usage(&final_result.id);
                let execution_service = crate::services::ExecutionService::new();
                execution_service.execute(&final_result).await?;
            }
        }
        Ok(())
    }

    fn get_result_icon(&self, result: &ActionResult) -> &'static str {
        // Use icon from metadata if available, otherwise fall back to action type
        if let Some(ref icon) = result.metadata.icon {
            // For emoji icons, return as-is (but we need to convert to &'static str)
            // This is a limitation - we'll use the action type for now
        }
        
        match &result.action {
            ActionType::Launch { needs_terminal: true } => "⚡",
            ActionType::Launch { needs_terminal: false } => "🚀",
            ActionType::Navigate { .. } => "📁",
            ActionType::AiResponse => "🤖",
            ActionType::Custom { .. } => match result.provider.as_str() {
                "ai_helper" => "🤖",
                "helper" => "⚙️",
                _ => "⚙️",
            },
        }
    }

    fn get_provider_tag(&self, provider: &str) -> &'static str {
        match provider {
            "applications" => "APP",
            "directories" => "DIR", 
            "ai_helper" => "AI",
            "helper" => "CMD",
            "weather" => "WTH",
            "news" => "NEWS",
            "sports" => "SPT",
            "stocks" => "STK",
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