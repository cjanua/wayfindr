// src/spawners/ai_query.rs
use tokio::sync::mpsc as tokio_mpsc;
use crate::types::AsyncResult;
use crate::services::ai; // To call query_gemini_api
use crate::utils::LOG_TO_FILE;

pub fn spawn_ai_query(user_query: String, tx: tokio_mpsc::Sender<AsyncResult>) {
    tokio::spawn(async move {
        LOG_TO_FILE(format!("[SPAWNER_AI] Received query: {}", user_query));

        // Construct a more specific prompt for Gemini based on user's examples
        let system_prompt_prefix = "You are a helpful assistant. Provide concise, factual responses. If a math question is asked, provide only the numerical answer. For other statements, respond in the most reasonable way possible. If you CANNOT come up with a reasonable response, output [INVALID]. User question: ";
        let full_prompt = format!("{}{}", system_prompt_prefix, user_query);
        
        LOG_TO_FILE(format!("[SPAWNER_AI] Constructed full prompt: {}", full_prompt));

        let result = match ai::query_gemini_api(full_prompt).await {
            Ok(response_text) => {
                LOG_TO_FILE(format!("[SPAWNER_AI] AI Response received: {}", response_text));
                AsyncResult::AiResponse(response_text)
            }
            Err(e) => {
                LOG_TO_FILE(format!("[SPAWNER_AI] AI Error: {}", e.to_string()));
                // Convert AiError into a string for the generic AsyncResult::Error
                // Or, you could add a new AsyncResult::AiError(AiError) variant
                // For now, simple string error.
                AsyncResult::Error(format!("AI Service Error: {}", e))
            }
        };

        if tx.send(result).await.is_err() {
            LOG_TO_FILE("[SPAWNER_AI] Failed to send AI result to main thread.".to_string());
        }
    });
}