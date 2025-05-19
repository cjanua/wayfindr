// src/main.rs
use std::io::{Stdout};
use std::time::Duration;
use tokio::sync::mpsc as tokio_mpsc;

use anyhow::{Context, Result as AnyhowResult};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::prelude::*;
use ratatui::backend::CrosstermBackend; // Explicit backend

// Module declarations
mod types;
mod app;
mod cli;
mod process_execution;
mod ui;
mod utils;
mod terminal;
mod spawners;
mod services;

use types::{ActionResult, AppError, AsyncResult};
use app::{App, FocusBlock};
use utils::LOG_TO_FILE;
use spawners::{
    path_search::spawn_path_search,
    ai_query::spawn_ai_query,
};

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> AnyhowResult<()> {
    // Handle CLI arguments first. If it returns Ok(true), we exit early.
    if cli::handle_cli_args()? {
        return Ok(());
    }

    // Prepare hook for panic handling to ensure terminal restoration.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = terminal::restore_terminal(); // Attempt to restore, ignore error here for panic hook
        original_hook(panic_info);
    }));

    // Setup terminal for TUI.
    terminal::setup_terminal().context("Failed to setup terminal")?;
    let mut ratatui_terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?; // Use fully qualified
    let mut app_state = App::new();

    // Run the main application loop.
    let app_result = run_app_loop(&mut ratatui_terminal, &mut app_state).await;
    
    // Ensure terminal is restored on normal exit or error from run_app_loop.
    if let Err(e) = terminal::restore_terminal() {
        LOG_TO_FILE(format!("[main] FATAL: Failed to restore terminal on exit: {:?}", e));
        // Depending on how critical this is, you might want to propagate this error too.
    }

    // Propagate error from app_result if any.
    app_result.map_err(|app_err| {
        LOG_TO_FILE(format!("[main] Error in main application loop: {:?}", app_err));
        anyhow::anyhow!("Error in main application loop: {:?}", app_err)
    })?;

    Ok(())
}

async fn run_app_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>, // Explicit Backend
    app: &mut App,
) -> Result<(), AppError> {
    let (sender, mut receiver) = tokio_mpsc::channel::<AsyncResult>(1);

    loop {
        if app.exit_flag {
            return Ok(());
        }

        terminal.draw(|frame| ui::ui(frame, app)).map_err(|e| AppError::Io(e))?; // Draw UI

        // Event polling
        if event::poll(Duration::from_millis(50)).map_err(|_| AppError::TerminalError)? {
            if let Event::Key(key) = event::read().map_err(|_| AppError::TerminalError)? {
                if key.kind == KeyEventKind::Press {
                    // --- Input Mode Specific History Reset ---
                    if app.focus == FocusBlock::Input {
                        match key.code {
                            KeyCode::Char(_) | KeyCode::Backspace => app.history_index = None,
                            _ => {}
                        }
                    }
                    // --- Key Handling ---
                    match key.code {
                        KeyCode::Esc => app.exit_flag = true,
                        KeyCode::Char(c) => {
                            if app.focus == FocusBlock::Input { app.input.push(c); }
                        }
                        KeyCode::Backspace => {
                            if app.focus == FocusBlock::Input { app.input.pop(); }
                        }
                        KeyCode::Tab => { // Cycle focus: Input -> Output -> History -> Input
                            match app.focus {
                                FocusBlock::Input => {
                                    if !app.output.is_empty() { app.focus = FocusBlock::Output; app.selected_output_index = 0; }
                                    else if !app.history.is_empty() { app.focus = FocusBlock::History; if app.history_index.is_none() { app.history_index = Some(0); app.input = app.history[0].clone();}}
                                }
                                FocusBlock::Output => {
                                    if !app.history.is_empty() { app.focus = FocusBlock::History; if app.history_index.is_none() { app.history_index = Some(0); app.input = app.history[0].clone();}}
                                    else { app.focus = FocusBlock::Input; }
                                }
                                FocusBlock::History => app.focus = FocusBlock::Input,
                            }
                        }
                        KeyCode::Enter => {
                            LOG_TO_FILE(format!("[EVENT] Enter. Focus: {:?}, Selected Output: {}, Output Len: {}", app.focus, app.selected_output_index, app.output.len()));
                            match app.focus {
                                FocusBlock::Input => {
                                    let query = app.input.trim().to_string();
                                    if !query.is_empty() {
                                        // Add to history (if not a duplicate of the last one)
                                        if app.history.is_empty() || app.history[0] != query {
                                            app.history.insert(0, query.clone());
                                        }
                                        if app.history.len() > 16 { app.history.pop(); }
                                        
                                        app.input.clear();
                                        app.clear_prev(); // Clear previous output/errors
                                        app.history_index = None;
                                        app.is_loading = true; // Set loading true

                                        // Check for AI command prefix
                                        if query.to_lowercase().starts_with("ai:") || query.to_lowercase().starts_with("ask:") {
                                            let ai_prompt_content = query.split_at(query.find(':').unwrap_or(0) + 1).1.trim().to_string();
                                            if !ai_prompt_content.is_empty() {
                                                LOG_TO_FILE(format!("[AI_TRIGGER] Spawning AI query for: {}", ai_prompt_content));
                                                spawn_ai_query(ai_prompt_content, sender.clone());
                                            } else {
                                                app.err_msg = "AI query is empty.".to_string();
                                                app.is_loading = false;
                                            }
                                        } else {
                                            // Default to path search
                                            spawn_path_search(query, sender.clone());
                                        }
                                    } else if !app.output.is_empty() { // Input empty, try to activate selected output
                                        app.focus = FocusBlock::Output; 
                                        // The logic below for FocusBlock::Output will handle it in the next iteration if not immediate.
                                        // To make it immediate, you could replicate the FocusBlock::Output logic here or refactor.
                                        // For now, simply changing focus might be enough if user presses Enter again.
                                    }
                                }
                                FocusBlock::Output => {
                                    if let Some(selected_action) = app.output.get(app.selected_output_index).cloned() { // Clone to avoid borrow issues
                                        LOG_TO_FILE(format!("[ACTION_EXEC] Queuing: {:?}", selected_action.description));
                                        if selected_action.spawner == "AI" {
                                            // Maybe copy to clipboard or do nothing further.
                                            LOG_TO_FILE(format!("[ACTION_EXEC] Selected an AI response. No further action defined yet. Text: {}", selected_action.description));
                                            app.focus = FocusBlock::Input; // Optionally return to input
                                        } else  {
                                            match app.handle_action_execution(&selected_action).await {
                                                Ok(_) => {
                                                    LOG_TO_FILE("[ACTION_EXEC] handle_action_execution successful.".to_string());
                                                    if selected_action.action == "cd" { app.exit_flag = true; }
                                                    else { app.output.clear(); app.err_msg.clear(); app.selected_output_index = 0; app.focus = FocusBlock::Input; }
                                                }
                                                Err(e) => {
                                                    LOG_TO_FILE(format!("[ACTION_EXEC] handle_action_execution failed: {:?}", e));
                                                    app.err_msg = format!("Action failed: {}", e); // Use error message from AppError
                                                    app.focus = FocusBlock::Input;
                                                }
                                            }
                                        }
                                    } else { app.focus = FocusBlock::Input; }
                                }
                                FocusBlock::History => {
                                    if let Some(index) = app.history_index {
                                        if index < app.history.len() {
                                            app.input = app.history[index].clone();
                                            let query_from_history = app.input.trim().to_string();
                                            app.history_index = None; app.focus = FocusBlock::Input;

                                            if !query_from_history.is_empty() {
                                                app.clear_prev(); app.is_loading = true;
                                                // Also check for AI prefix from history
                                                if query_from_history.to_lowercase().starts_with("ai:") || query_from_history.to_lowercase().starts_with("ask:") {
                                                    let ai_prompt_content = query_from_history.split_at(query_from_history.find(':').unwrap_or(0) + 1).1.trim().to_string();
                                                    if !ai_prompt_content.is_empty() {
                                                        spawn_ai_query(ai_prompt_content, sender.clone());
                                                    } else {
                                                        app.err_msg = "AI query from history is empty.".to_string();
                                                        app.is_loading = false;
                                                    }
                                                } else {
                                                    spawn_path_search(query_from_history, sender.clone());
                                                }
                                            }
                                        }
                                    } else { app.focus = FocusBlock::Input; }
                                }
                            }
                        }
                        KeyCode::Up => match app.focus {
                            FocusBlock::Input => if !app.history.is_empty() {
                                let idx = app.history_index.map_or(0, |i| (i + 1).min(app.history.len() - 1));
                                app.history_index = Some(idx); app.input = app.history[idx].clone();
                            },
                            FocusBlock::Output => if app.selected_output_index > 0 { app.selected_output_index -= 1; },
                            FocusBlock::History => if let Some(idx) = app.history_index {
                                if idx > 0 { app.history_index = Some(idx-1); app.input = app.history[idx-1].clone(); }
                                else { app.history_index = None; app.input.clear(); }
                            },
                        },
                        KeyCode::Down => match app.focus {
                            FocusBlock::Input => if let Some(idx) = app.history_index {
                                if idx > 0 { app.history_index = Some(idx-1); app.input = app.history[idx-1].clone();}
                                else { app.history_index = None; app.input.clear(); }
                            },
                            FocusBlock::Output => if !app.output.is_empty() && app.selected_output_index < app.output.len() - 1 { app.selected_output_index += 1; },
                            FocusBlock::History => if !app.history.is_empty() {
                                let idx: usize = app.history_index.map_or(0, |i| (i + 1).min(app.history.len()-1));
                                app.history_index = Some(idx); app.input = app.history[idx].clone();
                            },
                        },
                        _ => { /* app.err_msg = format!("Unhandled key: {:?}", key.code); */ } // Potentially noisy
                    }
                }
            }
        }

        // Async message handling
        match receiver.try_recv() {
            Ok(async_result) => {
                app.is_loading = false;
                match async_result {
                    AsyncResult::PathSearchResult(results) => {
                        app.output = results; app.err_msg.clear(); app.selected_output_index = 0;
                        app.focus = if app.output.is_empty() { FocusBlock::Input } else { FocusBlock::Output };
                    }
                    AsyncResult::AiResponse(response_text) => { // Handle AI Response
                        if response_text.contains("[INVALID]") {
                            app.err_msg = "AI couldn't figure it out.".to_string();
                            app.focus = FocusBlock::Input;
                        } else {
                            app.err_msg.clear();
                            app.output = vec![ActionResult {
                                spawner: "AI".to_string(),
                                action: "".to_string(),
                                description: response_text,
                                data: "".to_string(), // No specific data for action execution
                            }];
                            app.selected_output_index = 0;
                            app.focus = FocusBlock::Output; // Focus on the AI response
                        }
                    }
                    AsyncResult::Error(err_text) => {
                        app.clear_prev(); app.err_msg = err_text; app.focus = FocusBlock::Input;
                    }
                    // _ => { app.clear_prev(); app.err_msg = "Received unexpected async result.".to_string(); app.focus = FocusBlock::Input; }
                }
            }
            Err(tokio_mpsc::error::TryRecvError::Empty) => {}
            Err(tokio_mpsc::error::TryRecvError::Disconnected) => {
                app.is_loading = false; app.focus = FocusBlock::Input;
                // LOG_TO_FILE("[WARN] Async task channel disconnected.".to_string()); // Could be normal if tasks complete.
            }
        }
    }
}