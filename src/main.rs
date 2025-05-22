// src/main.rs
use anyhow::{Context, Result};
use tokio::sync::mpsc;

mod app;
mod cli;
mod config;
mod providers;
mod services;
mod terminal;
mod types;
mod ui;
mod utils;

use app::App;
use types::{AppResult, SearchMessage};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize configuration first
    config::init_config().context("Failed to initialize configuration")?;
    
    // Initialize services
    services::usage::init_usage_service().context("Failed to initialize usage service")?;
    
    // Handle CLI arguments
    if cli::handle_cli_args()? {
        return Ok(());
    }
    
    // Setup panic handler
    setup_panic_handler();
    
    // Run the application
    run_application().await
}

fn setup_panic_handler() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = terminal::restore_terminal();
        original_hook(panic_info);
    }));
}

async fn run_application() -> Result<()> {
    // Setup terminal
    terminal::setup_terminal().context("Failed to setup terminal")?;
    
    // Create terminal and app
    let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
    let mut terminal = ratatui::Terminal::new(backend)?;
    let mut app = App::new().await?;
    
    // Create message channel for async communication
    let (search_tx, search_rx) = mpsc::channel::<SearchMessage>(32);
    
    // Run main loop
    let result = app.run(&mut terminal, search_tx, search_rx).await;
    
    // Cleanup
    terminal::restore_terminal().context("Failed to restore terminal")?;
    
    result.map_err(|e| anyhow::anyhow!("Application error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_initialization() {
        // This test ensures config can be initialized without panicking
        assert!(config::init_config().is_ok());
    }
}