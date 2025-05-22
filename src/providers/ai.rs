// src/providers/ai.rs
use async_trait::async_trait;
use crate::{
    providers::{SearchProvider, ScoredResult},
    services::ai::query_gemini_api,
    types::{ActionResult, ActionType, ActionData, ActionMetadata, ProviderResult},
    utils,
};

pub struct AiProvider {
    enabled: bool,
}

impl AiProvider {
    pub fn new() -> Self {
        Self {
            enabled: std::env::var("GEMINI_API_KEY").is_ok(),
        }
    }
}

#[async_trait]
impl SearchProvider for AiProvider {
    fn id(&self) -> &'static str {
        "ai"
    }

    fn name(&self) -> &str {
        "AI Assistant"
    }

    fn can_handle(&self, query: &str) -> bool {
        // Only handle AI queries that are explicitly prefixed
        // This prevents accidental AI calls during live search
        self.enabled && (query.starts_with("ai:") || query.starts_with("ask:"))
    }

    fn priority(&self) -> u8 {
        80 // High priority for AI queries
    }

    async fn search(&self, query: &str) -> ProviderResult<Vec<ScoredResult>> {
        if !self.enabled {
            return Ok(Vec::new());
        }

        // Extract the actual AI query
        let ai_query = if query.starts_with("ai:") {
            query.strip_prefix("ai:").unwrap_or("").trim()
        } else if query.starts_with("ask:") {
            query.strip_prefix("ask:").unwrap_or("").trim()
        } else {
            // This should not happen due to can_handle check, but be safe
            return Ok(Vec::new());
        };

        // Require non-empty query after prefix
        if ai_query.is_empty() {
            return Ok(Vec::new());
        }

        utils::log_info(&format!("Processing AI query: {}", ai_query));

        let system_prompt = "You are a helpful assistant. Provide concise, factual responses. If a math question is asked, provide only the numerical answer. For other statements, respond in the most reasonable way possible. If you CANNOT come up with a reasonable response, output [INVALID]. User question: ";
        let full_prompt = format!("{}{}", system_prompt, ai_query);

        match query_gemini_api(full_prompt).await {
            Ok(response) => {
                if response.contains("[INVALID]") {
                    return Ok(Vec::new());
                }

                let action_id = utils::generate_id("ai", ai_query);
                let result = ActionResult {
                    id: action_id,
                    provider: self.id().to_string(),
                    action: ActionType::AiResponse,
                    title: format!("AI: {}", utils::truncate_text(ai_query, 50)),
                    description: response.clone(),
                    data: ActionData::Text(response),
                    metadata: ActionMetadata {
                        icon: Some("brain".to_string()),
                        category: Some("ai".to_string()),
                        tags: vec!["ai".to_string(), "assistant".to_string()],
                        usage_count: 0,
                        last_used: None,
                    },
                };

                let scored_result = ScoredResult::new(result, 1000, self.id().to_string());
                Ok(vec![scored_result])
            }
            Err(e) => {
                utils::log_error(&format!("AI query failed: {}", e));
                Err(crate::types::ProviderError::Api {
                    status: 500,
                    message: format!("AI service error: {}", e),
                })
            }
        }
    }

    fn configure(&mut self, _config: &crate::config::Config) {
        // Re-check if API key is available
        self.enabled = std::env::var("GEMINI_API_KEY").is_ok();
    }
}

impl Default for AiProvider {
    fn default() -> Self {
        Self::new()
    }
}