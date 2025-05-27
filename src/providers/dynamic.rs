// src/providers/dynamic.rs
use crate::{
    providers::{ScoredResult, SearchProvider},
    types::{ActionData, ActionMetadata, ActionResult, ActionType, ProviderError, ProviderResult},
    utils,
};
use async_trait::async_trait;
use chrono::{Local, Utc};
use handlebars::Handlebars;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::atomic::{AtomicBool, Ordering}};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicProviderConfig {
    pub provider: ProviderInfo,
    pub triggers: TriggerConfig,
    pub api: ApiConfig,
    pub commands: Vec<CommandConfig>,
    pub matchers: Vec<MatcherConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub priority: u8,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub prefixes: Vec<String>,
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(rename = "type")]
    pub api_type: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub method: String,
    pub params: Option<HashMap<String, String>>,
    pub body: Option<Value>,
    pub response_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatcherConfig {
    pub pattern: String,
    pub command: String,
    pub query_group: Option<usize>,
    pub use_location: Option<bool>,
}

pub struct DynamicProvider {
    config: DynamicProviderConfig,
    regex_matchers: Vec<(Regex, MatcherConfig)>,
    client: Client,
    handlebars: Handlebars<'static>,
    auth_failed: AtomicBool,
}

impl DynamicProvider {
    pub fn from_config(config: DynamicProviderConfig) -> Result<Self, ProviderError> {
        let mut regex_matchers = Vec::new();
        
        // Compile regex patterns
        for matcher in &config.matchers {
            match Regex::new(&matcher.pattern) {
                Ok(regex) => regex_matchers.push((regex, matcher.clone())),
                Err(e) => {
                    utils::log_warn(&format!(
                        "Invalid regex pattern '{}' in provider '{}': {}",
                        matcher.pattern, config.provider.id, e
                    ));
                }
            }
        }
        
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(false);
        
        Ok(Self {
            config,
            regex_matchers,
            client: Client::new(),
            handlebars,
            auth_failed: AtomicBool::new(false),
        })
    }
    
    fn get_api_key(&self) -> Option<String> {
        self.config.api.api_key_env.as_ref()
            .and_then(|env_var| std::env::var(env_var).ok())
    }

    fn check_api_key_availability(&self) -> bool {
        self.get_api_key().is_some()
    }

    fn create_api_key_help_result(&self, query: &str) -> ActionResult {
        let env_var = self.config.api.api_key_env.as_deref().unwrap_or("API_KEY");
        let action_id = utils::generate_id(&self.config.provider.id, "setup");
        
        ActionResult {
            id: action_id,
            provider: self.config.provider.id.clone(),
            action: ActionType::Custom { action_id: "setup".to_string() },
            title: format!("{} - Setup Required", self.config.provider.name),
            description: format!(
                "To use {}, set your API key: export {}=your-key-here", 
                self.config.provider.name, 
                env_var
            ),
            data: ActionData::Text(format!(
                "This provider requires an API key. Please set the {} environment variable.",
                env_var
            )),
            metadata: ActionMetadata {
                icon: Some("⚠️".to_string()),
                category: Some("setup".to_string()),
                tags: vec!["setup".to_string(), "api-key".to_string()],
                usage_count: 0,
                last_used: None,
            },
        }
    }

    fn create_auth_failed_result(&self, query: &str) -> ActionResult {
        let env_var = self.config.api.api_key_env.as_deref().unwrap_or("API_KEY");
        let action_id = utils::generate_id(&self.config.provider.id, "auth_failed");
        
        ActionResult {
            id: action_id,
            provider: self.config.provider.id.clone(),
            action: ActionType::Custom { action_id: "auth_failed".to_string() },
            title: format!("{} - Invalid API Key", self.config.provider.name),
            description: format!(
                "API key for {} appears to be invalid or expired. Please check your {} setting.", 
                self.config.provider.name, 
                env_var
            ),
            data: ActionData::Text(format!(
                "Authentication failed. Please verify your {} environment variable.",
                env_var
            )),
            metadata: ActionMetadata {
                icon: Some("🔑".to_string()),
                category: Some("error".to_string()),
                tags: vec!["authentication".to_string(), "api-key".to_string()],
                usage_count: 0,
                last_used: None,
            },
        }
    }
    
    fn get_location(&self) -> String {
        // You mentioned Orlando, Florida - we can make this configurable
        // or try to detect from system
        std::env::var("WAYFINDR_LOCATION")
            .unwrap_or_else(|_| "Orlando,FL,US".to_string())
    }
    
    async fn execute_command(
        &self,
        command: &CommandConfig,
        query: &str,
        use_location: bool,
    ) -> Result<String, ProviderError> {
        let mut context = HashMap::new();
        
        // Add template variables
        context.insert("query", query.to_string());
        context.insert("location", self.get_location());
        context.insert("date", Local::now().format("%Y-%m-%d").to_string());
        context.insert("datetime", Utc::now().to_rfc3339());
        
        if let Some(api_key) = self.get_api_key() {
            context.insert("api_key", api_key);
        }
        
        // Build URL
        let url = format!("{}{}", self.config.api.base_url, command.endpoint);
        
        // Build request
        let mut request = match command.method.as_str() {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            _ => return Err(ProviderError::Config("Unsupported HTTP method".to_string())),
        };
        
        // Add headers
        if let Some(headers) = &self.config.api.headers {
            for (key, value) in headers {
                request = request.header(key, value);
            }
        }
        
        // Add parameters
        if let Some(params) = &command.params {
            let mut query_params = Vec::new();
            for (key, template) in params {
                let value = self.render_template(template, &context)?;
                query_params.push((key.clone(), value));
            }
            request = request.query(&query_params);
        }
        
        // Add body if POST
        if command.method == "POST" {
            if let Some(body_template) = &command.body {
                // Process body template
                let body_str = serde_json::to_string(body_template)
                    .map_err(|e| ProviderError::Parsing(e.to_string()))?;
                let rendered_body = self.render_template(&body_str, &context)?;
                let body_value: Value = serde_json::from_str(&rendered_body)
                    .map_err(|e| ProviderError::Parsing(e.to_string()))?;
                request = request.json(&body_value);
            }
        }
        
        // Execute request
        let response = request.send().await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        
        let status = response.status();
        
        // Handle authentication failures specifically
        if status == 401 {
            self.auth_failed.store(true, Ordering::Relaxed);
            return Err(ProviderError::Api {
                status: 401,
                message: "Authentication failed - invalid or missing API key".to_string(),
            });
        }      
  
        if !response.status().is_success() {
            return Err(ProviderError::Api {
                status: response.status().as_u16(),
                message: format!("API request failed: {}", response.status()),
            });
        }
        
        let response_data: Value = response.json().await
            .map_err(|e| ProviderError::Parsing(e.to_string()))?;
        
        // Render response template
        self.handlebars.render_template(&command.response_template, &response_data)
            .map_err(|e| ProviderError::Parsing(e.to_string()))
    }
    
    fn render_template(&self, template: &str, context: &HashMap<&str, String>) -> Result<String, ProviderError> {
        let mut result = template.to_string();
        
        // Simple template rendering for URL parameters
        for (key, value) in context {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
            
            // Handle query|location pattern
            if *key == "query" && value.is_empty() {
                result = result.replace("{{query|location}}", &context.get("location").unwrap_or(&String::new()));
            }
        }
        
        Ok(result)
    }
}

#[async_trait]
impl SearchProvider for DynamicProvider {
    fn id(&self) -> &'static str {
        Box::leak(self.config.provider.id.clone().into_boxed_str())
    }
    
    fn name(&self) -> &str {
        &self.config.provider.name
    }
    
    fn can_handle(&self, query: &str) -> bool {
        if !self.config.provider.enabled {
            return false;
        }

        // Don't handle queries that are clearly for other providers
        if query.is_empty() || 
           query == "apps" || 
           query.starts_with("app:") ||
           query.starts_with("ai:") || 
           query.starts_with("ask:") {
            return false;
        }
        
        // Check prefixes
        for prefix in &self.config.triggers.prefixes {
            if query.starts_with(prefix) {
                return true;
            }
        }
        
        // Check patterns
        let query_lower = query.to_lowercase();
        for pattern in &self.config.triggers.patterns {
            if query_lower.contains(pattern) {
                return true;
            }
        }
        
        // Check regex matchers
        for (regex, _) in &self.regex_matchers {
            if regex.is_match(query) {
                return true;
            }
        }
        
        false
    }
    
    fn priority(&self) -> u8 {
        self.config.provider.priority
    }
    
    async fn search(&self, query: &str) -> ProviderResult<Vec<ScoredResult>> {
        if !self.check_api_key_availability() {
            let help_result = self.create_api_key_help_result(query);
            return Ok(
                vec![
                    ScoredResult::new(
                        help_result,
                        10, // Low score since it's just a setup message
                        self.config.provider.id.clone()
                    )
                ]
            )
        }

        // Check if authentication has previously failed
        if self.auth_failed.load(Ordering::Relaxed) {
            let auth_failed_result = self.create_auth_failed_result(query);
            return Ok(vec![ScoredResult::new(auth_failed_result, 10, self.config.provider.id.clone())]);
        }
        
        // Strip known prefixes
        let mut processed_query = query;
        for prefix in &self.config.triggers.prefixes {
            if let Some(stripped) = query.strip_prefix(prefix) {
                processed_query = stripped.trim();
                break;
            }
        }
        
        // Find matching command
        let mut command_id = None;
        let mut extracted_query = processed_query.to_string();
        let mut use_location = false;
        
        for (regex, matcher) in &self.regex_matchers {
            if let Some(captures) = regex.captures(processed_query) {
                command_id = Some(matcher.command.clone());
                
                // Extract query from capture group if specified
                if let Some(group_idx) = matcher.query_group {
                    if let Some(captured) = captures.get(group_idx) {
                        extracted_query = captured.as_str().to_string();
                    }
                }
                
                use_location = matcher.use_location.unwrap_or(false);
                break;
            }
        }
        
        // If no command matched, use the first one as default
        let command_id = command_id.unwrap_or_else(|| {
            self.config.commands.first()
                .map(|c| c.id.clone())
                .unwrap_or_default()
        });
        
        // Find the command
        let command = self.config.commands.iter()
            .find(|c| c.id == command_id)
            .ok_or_else(|| ProviderError::Config(format!("Command '{}' not found", command_id)))?;
        
        // Execute the command
        match self.execute_command(command, &extracted_query, use_location).await {
            Ok(response) => {
                let action_id = utils::generate_id(&self.config.provider.id, query);
                let r = response.clone();
                let result = ActionResult {
                    id: action_id,
                    provider: self.config.provider.id.clone(),
                    action: ActionType::Custom { action_id: command.id.clone() },
                    title: format!("{}: {}", self.config.provider.name, utils::truncate_text(query, 30)),
                    description: response,
                    data: ActionData::Text(r),
                    metadata: ActionMetadata {
                        icon: Some(self.get_icon()),
                        category: Some(self.config.provider.id.clone()),
                        tags: vec![self.config.provider.id.clone()],
                        usage_count: 0,
                        last_used: None,
                    },
                };
                
                Ok(vec![ScoredResult::new(result, 100, self.config.provider.id.clone())])
            }
            Err(e) => {
                utils::log_error(&format!("Dynamic provider '{}' error: {}", self.config.provider.id, e));
                Err(e)
            }
        }
    }
    
    fn configure(&mut self, _config: &crate::config::Config) {
        self.auth_failed.store(false, Ordering::Relaxed);
    }
}

impl DynamicProvider {
    fn get_icon(&self) -> String {
        match self.config.provider.id.as_str() {
            "weather" => "☁️",
            "sports" => "🏆",
            "stocks" => "📈",
            "news" => "📰",
            _ => "🔌",
        }.to_string()
    }
}

/// Load all dynamic providers from the configuration directory
pub fn load_dynamic_providers(config_dir: &Path) -> Vec<Box<dyn SearchProvider>> {
    let mut providers = Vec::new();
    let providers_dir = config_dir.join("providers");
    
    if !providers_dir.exists() {
        utils::log_info("No providers directory found, creating one");
        if let Err(e) = fs::create_dir_all(&providers_dir) {
            utils::log_error(&format!("Failed to create providers directory: {}", e));
            return providers;
        }
    }
    
    match fs::read_dir(&providers_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    match fs::read_to_string(&path) {
                        Ok(content) => {
                            match toml::from_str::<DynamicProviderConfig>(&content) {
                                Ok(config) => {
                                    match DynamicProvider::from_config(config) {
                                        Ok(provider) => {
                                            utils::log_info(&format!(
                                                "Loaded dynamic provider: {}", 
                                                provider.config.provider.name
                                            ));
                                            providers.push(Box::new(provider) as Box<dyn SearchProvider>);
                                        }
                                        Err(e) => {
                                            utils::log_error(&format!(
                                                "Failed to initialize provider from {}: {}",
                                                path.display(), e
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    utils::log_error(&format!(
                                        "Failed to parse provider config {}: {}",
                                        path.display(), e
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            utils::log_error(&format!(
                                "Failed to read provider config {}: {}",
                                path.display(), e
                            ));
                        }
                    }
                }
            }
        }
        Err(e) => {
            utils::log_error(&format!("Failed to read providers directory: {}", e));
        }
    }
    
    providers
}