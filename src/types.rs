// src/types.rs
use thiserror::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub id: String,
    pub provider: String,
    pub action: ActionType,
    pub title: String,
    pub description: String,
    pub data: ActionData,
    pub metadata: ActionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ActionType {
    Launch { needs_terminal: bool },
    Navigate { path: String },
    AiResponse,
    Custom { action_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionData {
    Command(String),
    Path(String),
    Text(String),
    Custom(serde_json::Value),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionMetadata {
    pub icon: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub usage_count: u32,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub enum SearchMessage {
    Query { query: String, provider_id: Option<String> },
    Results(Vec<crate::providers::ScoredResult>),
    Error(String),
    Loading(bool),
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Configuration error: {0}")]
    Config(#[from] anyhow::Error),
    
    #[error("Terminal error: {0}")]
    Terminal(String),
    
    #[error("Action execution error: {0}")]
    ActionExecution(String),
    
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),
    
    #[error("Search error: {0}")]
    Search(String),
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Data parsing error: {0}")]
    Parsing(String),
    
    #[error("External command failed: {0}")]
    Command(String),
    
    #[error("Provider unavailable: {0}")]
    Unavailable(String),
}

// Result type aliases for convenience
pub type AppResult<T> = Result<T, AppError>;
pub type ProviderResult<T> = Result<T, ProviderError>;

impl ActionResult {
    pub fn new_launch(
        id: impl Into<String>,
        provider: impl Into<String>,
        title: impl Into<String>,
        command: impl Into<String>,
        needs_terminal: bool,
    ) -> Self {
        Self {
            id: id.into(),
            provider: provider.into(),
            action: ActionType::Launch { needs_terminal },
            title: title.into(),
            description: String::new(),
            data: ActionData::Command(command.into()),
            metadata: ActionMetadata::default(),
        }
    }
    
    pub fn new_navigate(
        id: impl Into<String>,
        provider: impl Into<String>,
        title: impl Into<String>,
        path: impl Into<String>,
    ) -> Self {
        let path = path.into();
        Self {
            id: id.into(),
            provider: provider.into(),
            action: ActionType::Navigate { path: path.clone() },
            title: title.into(),
            description: String::new(),
            data: ActionData::Path(path),
            metadata: ActionMetadata::default(),
        }
    }
    
    pub fn new_ai_response(
        id: impl Into<String>,
        title: impl Into<String>,
        response: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            provider: "ai".to_string(),
            action: ActionType::AiResponse,
            title: title.into(),
            description: String::new(),
            data: ActionData::Text(response.into()),
            metadata: ActionMetadata::default(),
        }
    }
    
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
    
    pub fn with_metadata(mut self, metadata: ActionMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}