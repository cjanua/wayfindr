// src/services/ai.rs
#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;

use crate::utils::LOG_TO_FILE; // Optional: for logging AI service actions

const GEMINI_API_URL_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models/";
// Using a generally available and capable model. You can change this.
// gemini-1.5-flash-latest is good for speed and cost.
const DEFAULT_MODEL: &str = "gemini-1.5-flash-latest";

#[derive(Error, Debug)]
pub enum AiError {
    #[error("Missing API Key (GEMINI_API_KEY not set)")]
    MissingApiKey,
    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] reqwest::Error),
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },
    #[error("Failed to parse API response: {0}")]
    ResponseParsing(String),
    #[error("No content received from API")]
    NoContent,
}

// --- Request Structures ---
#[derive(Serialize)]
struct GeminiRequestPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiRequestContent {
    parts: Vec<GeminiRequestPart>,
    // role: Option<String>, // "user" or "model", typically "user" for requests
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: Option<u32>,
    temperature: Option<f32>,
    // top_p: Option<f32>,
    // top_k: Option<u32>,
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiRequestContent>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    // safety_settings: Option<Vec<SafetySetting>>, // For advanced safety controls
}


// --- Response Structures ---
#[derive(Deserialize, Debug)]
struct GeminiResponsePart {
    text: String,
    // function_call: Option<serde_json::Value>, // For function calling
}

#[derive(Deserialize, Debug)]
struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
    role: String,
}

#[derive(Deserialize, Debug)]
struct GeminiCandidate {
    content: GeminiResponseContent,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
    // safety_ratings: Vec<SafetyRating>,
    // citation_metadata: Option<CitationMetadata>,
}

#[derive(Deserialize, Debug)]
struct GeminiPromptFeedback {
    // block_reason: Option<String>,
    // safety_ratings: Vec<SafetyRating>,
}


#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "promptFeedback")]
    prompt_feedback: Option<GeminiPromptFeedback>,
}


pub async fn query_gemini_api(prompt: String) -> Result<String, AiError> {
    let api_key = env::var("GEMINI_API_KEY").map_err(|_| AiError::MissingApiKey)?;
    let client = Client::new();

    let request_body = GeminiRequest {
        contents: vec![GeminiRequestContent {
            parts: vec![GeminiRequestPart { text: prompt }],
            // role: Some("user".to_string()),
        }],
        generation_config: Some(GeminiGenerationConfig {
            max_output_tokens: Some(256), // Adjust as needed
            temperature: Some(0.7),       // Adjust for creativity vs. factuality
        }),
    };

    let model_to_use = DEFAULT_MODEL;
    let url = format!("{}{}:generateContent?key={}", GEMINI_API_URL_BASE, model_to_use, api_key);
    
    LOG_TO_FILE(format!("[AI_SERVICE] Sending query to Gemini. URL: {}, Model: {}", GEMINI_API_URL_BASE, model_to_use));

    let res = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status().as_u16();
        let error_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        LOG_TO_FILE(format!("[AI_SERVICE] API Error: {} - {}", status, error_text));
        return Err(AiError::ApiError { status, message: error_text });
    }

    let response_text = res.text().await?;
    LOG_TO_FILE(format!("[AI_SERVICE] Received response: {}", response_text));

    let parsed_response: GeminiResponse = serde_json::from_str(&response_text)
        .map_err(|e| {
            LOG_TO_FILE(format!("[AI_SERVICE] JSON Parsing Error: {}", e));
            AiError::ResponseParsing(e.to_string())
        })?;


    if let Some(candidates) = parsed_response.candidates {
        if let Some(first_candidate) = candidates.first() {
            if let Some(first_part) = first_candidate.content.parts.first() {
                LOG_TO_FILE(format!("[AI_SERVICE] Extracted text: {}", first_part.text));
                return Ok(first_part.text.clone());
            }
        }
    }
    
    LOG_TO_FILE("[AI_SERVICE] No usable content found in API response".to_string());
    Err(AiError::NoContent)
}