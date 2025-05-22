// src/services/ai.rs
use crate::utils;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use thiserror::Error;

const GEMINI_API_URL_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models/";
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

#[derive(Serialize)]
struct GeminiRequestPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiRequestContent {
    parts: Vec<GeminiRequestPart>,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: Option<u32>,
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiRequestContent>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
}

#[derive(Deserialize, Debug)]
struct GeminiResponsePart {
    text: String,
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
}

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

pub async fn query_gemini_api(prompt: String) -> Result<String, AiError> {
    let api_key = env::var("GEMINI_API_KEY").map_err(|_| AiError::MissingApiKey)?;
    let client = Client::new();

    let request_body = GeminiRequest {
        contents: vec![GeminiRequestContent {
            parts: vec![GeminiRequestPart { text: prompt }],
        }],
        generation_config: Some(GeminiGenerationConfig {
            max_output_tokens: Some(256),
            temperature: Some(0.7),
        }),
    };

    let url = format!(
        "{}{}:generateContent?key={}",
        GEMINI_API_URL_BASE, DEFAULT_MODEL, api_key
    );

    utils::log_debug(&format!("Sending request to Gemini API: {}", DEFAULT_MODEL));

    let res = client.post(&url).json(&request_body).send().await?;

    if !res.status().is_success() {
        let status = res.status().as_u16();
        let error_text = res
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        utils::log_error(&format!("Gemini API error: {} - {}", status, error_text));
        return Err(AiError::ApiError {
            status,
            message: error_text,
        });
    }

    let response_text = res.text().await?;
    utils::log_debug(&format!("Received response from Gemini API"));

    let parsed_response: GeminiResponse = serde_json::from_str(&response_text).map_err(|e| {
        utils::log_error(&format!("JSON parsing error: {}", e));
        AiError::ResponseParsing(e.to_string())
    })?;

    if let Some(candidates) = parsed_response.candidates {
        if let Some(first_candidate) = candidates.first() {
            if let Some(first_part) = first_candidate.content.parts.first() {
                utils::log_debug(&format!("Successfully extracted AI response"));
                return Ok(first_part.text.clone());
            }
        }
    }

    utils::log_error("No usable content found in API response");
    Err(AiError::NoContent)
}
