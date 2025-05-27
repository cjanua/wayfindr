// src/main.rs - Updated with interface selection
use anyhow::{Context, Result};

mod app;
mod cli;
mod config;
mod interfaces; // New interfaces module
mod providers;
mod services;
mod terminal;
mod types;
mod ui;
mod utils;

use app::App;
use interfaces::{run_interface, InterfaceType};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize configuration first
    config::init_config().context("Failed to initialize configuration")?;

    // Initialize services
    services::usage::init_usage_service().context("Failed to initialize usage service")?;

    // Handle CLI arguments and get interface type
    let (should_exit_early, interface_type) = cli::handle_cli_args()?;
    if should_exit_early {
        return Ok(());
    }

    // Setup panic handler
    setup_panic_handler();

    // Run the application with the selected interface
    run_application(interface_type).await
}

fn setup_panic_handler() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Only restore terminal for TUI interface
        let _ = terminal::restore_terminal();
        original_hook(panic_info);
    }));
}

async fn run_application(interface_type: InterfaceType) -> Result<()> {
    // Create app instance
    let app = App::new().await?;

    // Log which interface is being used
    match interface_type {
        InterfaceType::Tui => utils::log_info("Starting wayfindr with TUI interface"),
        InterfaceType::Rofi => utils::log_info("Starting wayfindr with rofi interface"),
    }

    // Run with the selected interface
    run_interface(interface_type, app).await
        .map_err(|e| anyhow::anyhow!("Application error: {}", e))
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