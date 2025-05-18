// src/types.rs
#![allow(dead_code)]
use thiserror::Error;

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
    AnotherProcessResult(String),
    YetAnotherResult(i32),
    Error(String),
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Terminal error")]
    TerminalError,
    #[error("Action execution error: {0}")]
    ActionError(String),
}