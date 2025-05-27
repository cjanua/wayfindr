// src/interfaces/tui.rs - TUI interface (extracted from main app)
use crate::app::App;
use crate::types::{AppResult, SearchMessage};
use crate::{terminal, ui};
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

/// Run wayfindr with the TUI interface
pub async fn run_tui(mut app: App) -> AppResult<()> {
    // Setup terminal
    terminal::setup_terminal().map_err(|e| crate::types::AppError::Terminal(e.to_string()))?;

    // Create terminal and message channel
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = ratatui::Terminal::new(backend)
        .map_err(|e| crate::types::AppError::Terminal(e.to_string()))?;
    
    let (search_tx, search_rx) = mpsc::channel::<SearchMessage>(32);

    // Run main TUI loop
    let result = app.run(&mut terminal, search_tx, search_rx).await;

    // Cleanup
    terminal::restore_terminal().map_err(|e| crate::types::AppError::Terminal(e.to_string()))?;

    result
}