// src/types.rs
#![allow(dead_code)]
use thiserror::Error;

#[derive(Debug, Clone)]
pub enum AsyncResult {
    ZoxideResult(Vec<String>),
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
}