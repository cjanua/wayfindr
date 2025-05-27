// src/cli.rs
use crate::utils::DEFAULT_LOG_FILE_PATH;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[arg(long, value_name = "FILE_PATH", num_args = 0..=1, value_hint = clap::ValueHint::FilePath)]
    pub logs: Option<Option<PathBuf>>,
    
    #[command(subcommand)]
    pub provider: Option<ProviderCommands>,
}

#[derive(Subcommand, Debug)]
pub enum ProviderCommands {
    /// List all providers
    List,
    /// Enable a provider
    Enable { name: String },
    /// Disable a provider
    Disable { name: String },
    /// Show provider configuration
    Show { name: String },
    /// Create new provider from template
    Create { name: String },
    /// Test provider with query
    Test { name: String, query: String },
    /// Install default provider configurations
    InstallDefaults,
}

/// Handles CLI arguments.
/// Returns `Ok(true)` if the program should exit early, `Ok(false)` to continue.
/// Returns `Err` on critical failure.
pub fn handle_cli_args() -> Result<bool, anyhow::Error> {
    let cli_args = CliArgs::parse();

    // Handle --logs
    if let Some(option_for_path_or_default_signal) = cli_args.logs {
        let log_file_to_view: PathBuf = match option_for_path_or_default_signal {
            Some(specific_path) => specific_path,
            None => {
                // Use the config system to get the actual log file path
                let config = crate::config::get_config();
                config.paths.log_file.clone()
            }
        };

        if !log_file_to_view.exists() {
            eprintln!(
                "Error: Log file not found at '{}'",
                log_file_to_view.display()
            );
            eprintln!("Tip: The application writes logs to this file when actions are performed or if it's run without the --logs flag.");
            return Ok(true); // Exit early
        }

        if let Ok(content) = std::fs::read_to_string(&log_file_to_view) {
            content.lines().for_each(|line| eprintln!("{}", line));
        }
        return Ok(true); // Exit after handling --logs
    }
    
    // Handle --provider subcommands
    if let Some(provider_cmd) = cli_args.provider {
        crate::providers::management::handle_provider_command(provider_cmd)?;
        return Ok(true); 
    }

    Ok(false) // Continue to TUI
}