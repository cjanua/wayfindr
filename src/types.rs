// src/types.rs
#![allow(dead_code)]
use thiserror::Error;
use crate::services::ai::AiError; // Importing AiError for the new variant

#[derive(Debug, Clone)]
pub struct ActionResult {
    pub spawner: String,
    pub action: String,
    pub description: String,
    pub data: String,
}

#[derive(Debug, Clone)]
pub enum AsyncResult {
    PathSearchResult(Vec<ActionResult>),
    AiResponse(String),
    Error(String),
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error), // Useful for errors from process_execution
    #[error("Terminal error")]
    TerminalError, // For crossterm related issues
    #[error("Action execution error: {0}")]
    ActionError(String),
    #[error("CLI argument error: {0}")] // Example, if cli.rs needs its own error
    CliError(String),
    #[error("AI service error: {0}")] // New variant for AI errors
    AiService(#[from] AiError),
    #[error("Configuration error: {0}")] // Example for config errors
    ConfigError(String),
}