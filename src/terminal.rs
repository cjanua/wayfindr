// src/terminal.rs

use anyhow::{Context, Result as AnyhowResult};
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::stdout;

pub fn setup_terminal() -> AnyhowResult<()> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    stdout()
        .execute(EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    Ok(())
}

pub fn restore_terminal() -> AnyhowResult<()> {
    if crossterm::terminal::is_raw_mode_enabled()? {
        disable_raw_mode().context("Failed to disable raw mode")?;
    }
    stdout()
        .execute(LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    Ok(())
}
