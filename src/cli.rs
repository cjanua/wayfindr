// src/cli.rs
use std::path::PathBuf;
use clap::Parser;
use crate::utils::{DEFAULT_LOG_FILE_PATH};
use crate::process_execution; // For launching kitty

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[arg(long, value_name = "FILE_PATH", num_args = 0..=1, value_hint = clap::ValueHint::FilePath)]
    pub logs: Option<Option<PathBuf>>,
}

/// Handles CLI arguments.
/// Returns `Ok(true)` if the program should exit early, `Ok(false)` to continue.
/// Returns `Err` on critical failure.
pub fn handle_cli_args() -> Result<bool, anyhow::Error> {
    let cli_args = CliArgs::parse();

    if let Some(option_for_path_or_default_signal) = cli_args.logs {
        let log_file_to_view: PathBuf = match option_for_path_or_default_signal {
            Some(specific_path) => specific_path,
            None => PathBuf::from(DEFAULT_LOG_FILE_PATH),
        };

        if !log_file_to_view.exists() {
            eprintln!(
                "Error: Log file not found at '{}'",
                log_file_to_view.display()
            );
            eprintln!("Tip: The application writes logs to this file when actions are performed or if it's run without the --logs flag.");
            return Ok(true); // Exit early
        }

        match process_execution::launch_kitty_to_view_file(&log_file_to_view) {
            Ok(_) => {
                eprintln!("Kitty launched to display logs. The application will now exit.");
            }
            Err(e) => {
                eprintln!("Error launching Kitty: {}", e);
                eprintln!("Please ensure 'kitty' is installed and in your PATH.");
                eprintln!("Falling back to printing log to stdout (first 50 lines):");
                if let Ok(content) = std::fs::read_to_string(&log_file_to_view) {
                    content.lines().take(50).for_each(|line| eprintln!("{}", line));
                    if content.lines().count() > 50 {
                        eprintln!("... (log truncated)");
                    }
                } else {
                    eprintln!("Could not read log file for fallback.")
                }
            }
        }
        return Ok(true); // Exit after handling --logs
    }

    Ok(false) // Continue to TUI
}