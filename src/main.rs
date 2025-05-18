// src/main.rs
use std::io::{stdout, Stdout};
use tokio::sync::mpsc as tokio_mpsc;
use std::time::Duration;

use anyhow::{Context, Result as AnyhowResult};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

mod types;
use types::{AsyncResult, AppError};

mod spawners;
use spawners::path_search::spawn_path_search;

#[cfg(test)]
mod tests;

struct App {
    input: String,
    output: Vec<String>,
    history: Vec<String>,
    exit_flag: bool,
    err_msg: String,
    is_loading: bool,

    spawner_results: Vec<String>,
}
impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            output: vec!(),

            history: vec!(),
            exit_flag: false,
            err_msg: String::new(),
            is_loading: false,

            spawner_results: vec!(),
        }
    }

    fn clear_prev(&mut self) {
        self.output.clear();
        self.err_msg.clear();
        self.spawner_results.clear();
    }
}

fn setup_terminal() -> AnyhowResult<()> {
    // Enable raw mode and enter alternate screen
    enable_raw_mode().context("Failed to enable raw mode")?;
    stdout()
        .execute(EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    Ok(())
}

fn restore_terminal() -> AnyhowResult<()> {
    // It's important to disable raw mode FIRST, then leave the alternate screen.
    if crossterm::terminal::is_raw_mode_enabled()? {
        disable_raw_mode().context("Failed to disable raw mode")?;
    }
    // Using stdout() directly for ExecuteCommand trait
    stdout()
        .execute(LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;
    Ok(())
}


#[tokio::main]
async fn main() -> AnyhowResult<()> {
    // Prepare hook for panic handling
    // This will ensure that the terminal is restored even if a panic occurs.    
    let original_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        // Attempt to restore the terminal
        if let Err(e) = restore_terminal() {
            // If restoring fails, print the error to stderr.
            // Printing to stdout might not work if it's still in raw mode.
            Err::<(), AppError>(AppError::TerminalError)
                .context(format!("Failed to restore terminal after panic {:?}", e))
                .unwrap_err();
        }
        eprintln!("Panic occurred: {:?}", panic_info);

        // Call the original panic hook, which prints the panic message and backtrace.
        original_hook(panic_info);
    }));

    // Setup terminal
    setup_terminal().context("Failed to setup terminal")?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();

    let app_result = run_app(&mut terminal, &mut app);
    if let Err(e) = restore_terminal() {
        eprintln!("[main] FATAL: Failed to restore terminal: {:?}", e);
    }

    app_result.map_err(|app_err| {
        anyhow::anyhow!("Error in main loop: {:?}", app_err)
    })?;

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<(), AppError> {
    // Create a channel for zoxide results
    // This will be used to send results from the async task back to the main thread
    let (sender, mut receiver) = tokio_mpsc::channel::<AsyncResult>(1);

    loop {
        if app.exit_flag {
            return Ok(());
        }

        terminal.draw(|frame| ui(frame, app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc => app.exit_flag = true,
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Enter => {
                            // Append to history
                            app.history.insert(0, app.input.clone());
                            if app.history.len() > 16 {
                                app.history.pop();
                            }
                            let query = app.input.trim().to_string();
                            app.input.clear();
                            app.clear_prev();

                            if !query.is_empty() {
                                spawn_path_search(query, sender.clone());
                                app.is_loading = true;
                            }
                        }
                        _ => {
                            // Handle other keys if needed
                            app.err_msg = format!("Unhandled key: {:?}", key.code);
                        }
                    }
                }
            }
        }

        match receiver.try_recv() {
            Ok(async_result) => {
                app.is_loading = false;
                match async_result {
                    AsyncResult::ZoxideResult(results) => {
                        app.spawner_results = results;
                        app.err_msg.clear();
                        if !app.spawner_results.is_empty() {
                            app.output = app.spawner_results.clone();
                        } else {
                            app.output = vec!["No results found".to_string()];
                        }
                    }
                    AsyncResult::Error(err_text) => {
                        app.clear_prev();
                        app.err_msg = err_text;
                    }
                    _ => {
                        app.err_msg = "Error receiving results from spawner".to_string();
                    }
                }
            }
            Err(tokio_mpsc::error::TryRecvError::Empty) => {
                // No message from async tasks, do nothing here regarding results
                // app.is_loading remains true if it was set
            }
            Err(tokio_mpsc::error::TryRecvError::Disconnected) => {
                // The sender has been dropped. This might mean all tasks are complete,
                // or something went wrong.
                app.is_loading = false; // No longer expecting messages from this channel
                app.err_msg = "Async communication channel disconnected.".to_string();
                // You might want to re-create the channel if you expect more tasks
                // or handle this as a critical error. For now, just stop loading.
                
            }
        }
    }
}

fn ui(
    frame: &mut Frame,
    app: &App,
) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input area
            Constraint::Length(3), // Empty output box
            Constraint::Min(0),    // History area
        ])
        .split(frame.area());

    let input_text = format!("> {}", app.input);
    let input_block = Paragraph::new(input_text)
        .block(Block::default().borders(Borders::ALL).title("Search"))
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(input_block, main_layout[0]);

    let output_text = if app.is_loading {
        "Loading...".to_string()
    } else if !app.err_msg.is_empty() {
        format!("Error: {}", app.err_msg) // Display error message
    } else if !app.output.is_empty() {
        // app.output is now expected to hold the primary result string(s)
        app.output.join("\n") // If app.output can have multiple lines
    } else {
        // No results, no error, not loading
        "No results".to_string()
    };
    let output_block = Paragraph::new(output_text.to_string())
        .block(Block::default().borders(Borders::ALL).title("Output"))
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(output_block, main_layout[1]);

    let mut history_text = app.history[0..std::cmp::min(app.history.len(), 10)].join("\n");
    if app.history.len() > 11 {
        history_text.push_str("\n...\n");
    }
    let history_block = Paragraph::new(history_text.to_string())
        .block(Block::default().borders(Borders::ALL).title("History"))
        .style(Style::default().fg(Color::White));

    frame.render_widget(history_block, main_layout[2]);
}