// src/main.rs
use std::{
    fs::OpenOptions,
    io::{stdout, Stdout, Write},
    path::PathBuf,
    time::{SystemTime, Duration},
    process::Command as StdCommand, // For spawning new terminal
};
use clap::Parser;
use tokio::sync::mpsc as tokio_mpsc;

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
use ratatui::widgets::{List, ListItem}; // Add List and ListItem

mod types;
use types::{ActionResult, AppError, AsyncResult};

mod spawners;
use spawners::path_search::spawn_path_search;

#[cfg(test)]
mod tests;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(long, value_name = "FILE_PATH", num_args = 0..=1, value_hint = clap::ValueHint::FilePath)]
    logs: Option<Option<PathBuf>>
}


#[derive(Debug)]
enum FocusBlock {
    Input,
    Output,
    History,
}

const DEFAULT_LOG_FILE_PATH: &str = "/tmp/wayfindr_action.log_robust";

static LOG_TO_FILE: fn(String) = |message: String| {
    match OpenOptions::new().create(true).append(true).open(DEFAULT_LOG_FILE_PATH) {
        Ok(mut file) => {
            let time_now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let _ = writeln!(file, "[{}] {}", time_now, message);
        }
        Err(_) => {
            // If opening the log file fails, also do nothing.
        }
    }
};

struct App {
    input: String,
    output: Vec<ActionResult>,
    history: Vec<String>,
    exit_flag: bool,
    err_msg: String,
    is_loading: bool,
    selected_output_index: usize,
    focus: FocusBlock,
    history_index: Option<usize>,
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

            selected_output_index: 0,
            focus: FocusBlock::Input,
            history_index: None,
        }
    }

    fn clear_prev(&mut self) {
        self.output.clear();
        self.err_msg.clear();
    }

    async fn execute_action(&self, action_result: &ActionResult) -> Result<(), AppError> {
        LOG_TO_FILE(format!(
            "[ACTION] Attempting to execute: spawner='{}', action='{}', data='{}'",
            action_result.spawner, action_result.action, action_result.data
        ));
    
        match action_result.spawner.as_str() {
            "z" | "fs" => {
                if action_result.action == "cd" {
                    let path_to_cd = &action_result.data;
                    LOG_TO_FILE(format!("Attempting to open terminal at: {}", path_to_cd));
    
                    let escaped_path = path_to_cd.replace("'", r"'\''");
                    let shell_command = format!("cd '{}' && exec $SHELL", escaped_path);
                    LOG_TO_FILE(format!("[execute_action] Shell command for Kitty: {}", shell_command));

                    let mut cmd = StdCommand::new("kitty");
                    cmd.arg("-e");    // Tells kitty to execute the following command
                    cmd.arg("sh");    // The shell to use (sh is minimal and widely available)
                    cmd.arg("-c");    // Tell sh to read commands from the next string argument
                    cmd.arg(&shell_command);

                    // Detach the process
                    cmd.stdout(std::process::Stdio::null());
                    cmd.stderr(std::process::Stdio::null());

                    match cmd.spawn() {
                        Ok(_) => {
                            LOG_TO_FILE(format!(
                                "[INFO] Kitty spawn() call returned Ok. Path: {}",
                                path_to_cd
                            ));
                            // self.exit_flag = true; // If opening a new terminal means this app should exit
                        }
                        Err(e) => {
                            LOG_TO_FILE(format!("[ERROR] Failed to spawn Kitty for path {}: {}", path_to_cd, e));
                            // Fallback or error message if needed
                            // return Err(AppError::ActionError(format!("Failed to open Kitty: {}", e)));
                            // For now, just log and continue, or attempt other terminals if you re-add that logic.
                            // If Kitty is the primary target and fails, it's an error for this specific action.
                            // We'll let the original error handling (if any) outside this function manage UI feedback.
                             return Err(AppError::ActionError(format!("Failed to open Kitty (is it installed and in PATH?): {}. Path: {}", e, path_to_cd)));
                        }
                    }
                    Ok(())
                } else {
                    LOG_TO_FILE(format!("Unknown action '{}' for spawner '{}'", action_result.action, action_result.spawner));
                    Err(AppError::ActionError(format!(
                        "Unknown action '{}' for spawner '{}'",
                        action_result.action, action_result.spawner
                    )))
                }
            }
            _ => {
                LOG_TO_FILE(format!("Unknown spawner '{}'", action_result.spawner));
                Err(AppError::ActionError(format!(
                    "Unknown spawner '{}'",
                    action_result.spawner
                )))
            }
        }
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
    let cli_args = CliArgs::parse();
    // Prepare hook for panic handling
    // This will ensure that the terminal is restored even if a panic occurs.
    // --- Handle --logs flag ---
    if let Some(option_for_path_or_default_signal) = cli_args.logs {
        let log_file_to_view: PathBuf = match option_for_path_or_default_signal {
            Some(specific_path) => specific_path, // User provided --logs /path/to/actual.log
            None => PathBuf::from(DEFAULT_LOG_FILE_PATH), // User provided --logs (no value), use default
        };

        if !log_file_to_view.exists() {
            eprintln!(
                "Error: Log file not found at '{}'",
                log_file_to_view.display()
            );
            eprintln!("Tip: The application writes logs to this file when actions are performed or if it's run without the --logs flag.");
            return Ok(()); // Exit if log file doesn't exist
        }

        // Command to execute in Kitty: cat the log file, then start a shell
        let shell_command = format!(
            "cat '{}'; echo -e \"\n--- End of log. Press Ctrl+D or type 'exit' to close. ---\"; exec $SHELL",
            log_file_to_view.to_string_lossy()
        );

        eprintln!( // Use eprintln for CLI output before TUI potentially starts
            "Attempting to open logs from '{}' in Kitty...",
            log_file_to_view.display()
        );

        let mut kitty_cmd = StdCommand::new("kitty");
        kitty_cmd.arg("-e") // Tells kitty to execute the following command
                 .arg("sh")   // The shell to use
                 .arg("-c")   // Tell sh to read commands from string
                 .arg(&shell_command);
        
        // Detach Kitty process
        // kitty_cmd.stdout(std::process::Stdio::null());
        // kitty_cmd.stderr(std::process::Stdio::null());


        match kitty_cmd.spawn() {
            Ok(_) => {
                eprintln!("Kitty launched to display logs. The application will now exit.");
                // Successfully launched Kitty, so we can exit the main program.
            }
            Err(e) => {
                eprintln!("Error launching Kitty: {}", e);
                eprintln!("Please ensure 'kitty' is installed and in your PATH.");
                eprintln!("Falling back to printing log to stdout (first 50 lines):");
                // Fallback: print some log content to stdout if Kitty fails
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
        return Ok(()); // Exit after handling --logs
    }

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal(); // Attempt to restore, ignore error here
        original_hook(panic_info);
    }));


    // Setup terminal
    setup_terminal().context("Failed to setup terminal")?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();

    let app_result = run_app(&mut terminal, &mut app).await;
    if let Err(e) = restore_terminal() {
        // If TUI was running, errors here are tricky. eprintln might be garbled.
        // For now, just log it to the file if possible.
        LOG_TO_FILE(format!("[main] FATAL: Failed to restore terminal on exit: {:?}", e));
    }


    app_result.map_err(|app_err| {
        LOG_TO_FILE(format!("[main] Error in main loop: {:?}", app_err));
        anyhow::anyhow!("Error in main loop: {:?}", app_err)
    })?;

    Ok(())
}

async fn run_app(
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

        if event::poll(Duration::from_millis(50)).map_err(|_e| AppError::TerminalError)? {
            if let Event::Key(key) = event::read().map_err(|_e| AppError::TerminalError)? {
                if key.kind == KeyEventKind::Press {
                    if matches!(app.focus, FocusBlock::Input) {
                        match key.code {
                            KeyCode::Char(_) | KeyCode::Backspace => {
                                app.history_index = None;
                            }
                            _ => {}
                        }
                    }

                    match key.code {
                        KeyCode::Esc => app.exit_flag = true,
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Tab => {
                            match app.focus {
                                FocusBlock::Input => {
                                    if !app.output.is_empty() {
                                        app.focus = FocusBlock::Output;
                                        app.selected_output_index = 0;
                                    } else if !app.history.is_empty() { // Only go to history if output is empty
                                        app.focus = FocusBlock::History;
                                        // Initialize history_index if focusing history for the first time via Tab
                                        if app.history_index.is_none() && !app.history.is_empty() {
                                            app.history_index = Some(0); // Select first item
                                            app.input = app.history[0].clone(); // And display it
                                        }
                                    }
                                }
                                FocusBlock::Output => {
                                    if !app.history.is_empty() {
                                        app.focus = FocusBlock::History;
                                        if app.history_index.is_none() && !app.history.is_empty() {
                                            app.history_index = Some(0);
                                            app.input = app.history[0].clone();
                                        }
                                    } else {
                                         app.focus = FocusBlock::Input; // If no history, go back to input
                                    }
                                },
                                FocusBlock::History => app.focus = FocusBlock::Input,
                            }
                        }
                        KeyCode::Enter => {
                            LOG_TO_FILE(format!("[EVENT] Enter key pressed. Focus: {:?}, Selected Output Index: {}, Output len: {}", 
                                app.focus, // You'll need to derive Debug for Focus enum
                                app.selected_output_index,
                                app.output.len()
                            ));
                            match app.focus {
                                FocusBlock::Input => {
                                    let query = app.input.trim().to_string();
                                    if !query.is_empty() {
                                        app.history.insert(0, query.clone());
                                        if app.history.len() > 16 { app.history.pop(); }
                                        app.input.clear();
                                        app.clear_prev();
                                        app.history_index=None;
                                        spawn_path_search(query, sender.clone());
                                        app.is_loading = true;
                                    } else { // Input is empty
                                        if !app.output.is_empty() && app.output.get(app.selected_output_index).is_some() {
                                            // If input empty but output has items, effectively "Enter" on selected output.
                                            // This mimics activating the current selection if any.
                                            app.focus = FocusBlock::Output; // Shift focus to trigger output logic below
                                            // The logic for FocusBlock::Output will handle the execution
                                        }
                                    }
                                }

                                FocusBlock::Output => {
                                    if let Some(selected_action) = app.output.get(app.selected_output_index) {
                                        LOG_TO_FILE(format!("[ACTION EXEC] Executing selected output: {:?}", selected_action.description));
                                        // Clone the action before the await, as app state might change
                                        let action_to_execute = selected_action.clone();
                                        match app.execute_action(&action_to_execute).await {
                                            Ok(_) => {
                                                LOG_TO_FILE("[ACTION EXEC] execute_action successful.".to_string());
                                                // For 'cd' like actions, it's common for the TUI to exit.
                                                // Or, it could clear results and return to input for further commands.
                                                // Let's choose exit for 'cd' actions for now for simplicity.
                                                // If you want it to stay open, comment out app.exit_flag = true;
                                                // and ensure state is reset (e.g., clear output, focus input).
                                                if action_to_execute.action == "cd" {
                                                    app.exit_flag = true;
                                                } else {
                                                    // For other actions, maybe clear and refocus
                                                    app.output.clear();
                                                    app.err_msg.clear();
                                                    app.selected_output_index = 0;
                                                    app.focus = FocusBlock::Input;
                                                }
                                            }
                                            Err(e) => {
                                                LOG_TO_FILE(format!("[ACTION EXEC] execute_action failed: {:?}", e));
                                                app.err_msg = format!("Action failed: {:?}", e);
                                                app.focus = FocusBlock::Input; 
                                            }
                                        }
                                    } else {
                                        LOG_TO_FILE("[ACTION EXEC] Enter in Output focus, but no item selected or output empty.".to_string());
                                        app.focus = FocusBlock::Input; // Go back to input if nothing to act on
                                    }
                                }

                                
                                FocusBlock::History => {
                                    if let Some(index) = app.history_index {
                                        if index < app.history.len() { // Check bounds
                                            app.input = app.history[index].clone();
                                            // After selecting from history, clear history_index and move focus to input
                                            // and potentially trigger a search with this input.
                                            let query = app.input.trim().to_string();
                                            app.history_index = None;
                                            app.focus = FocusBlock::Input;

                                            if !query.is_empty() {
                                                // Optionally, immediately trigger search from history selection
                                                // app.history.insert(0, query.clone()); // Don't re-add to history here
                                                // if app.history.len() > 16 { app.history.pop(); }
                                                // app.input.clear(); // Input is already set
                                                app.clear_prev();
                                                spawn_path_search(query, sender.clone());
                                                app.is_loading = true;
                                            }
                                        }
                                    } else {
                                        // If history_index is None but focus is History (e.g. after Tab),
                                        // Enter could mean "confirm current input" or "do nothing"
                                        // For now, let's switch focus to input.
                                        app.focus = FocusBlock::Input;
                                    }
                                }

                            }
                        }
                        KeyCode::Up => {
                            match app.focus {
                                FocusBlock::Input => {
                                    if !app.history.is_empty() {
                                        let current_idx = app.history_index.unwrap_or(0); 
                                        if app.history_index.is_none() { 
                                            app.history_index = Some(0);
                                            app.input = app.history[0].clone();
                                        } else if current_idx < app.history.len() - 1 {
                                            let next_idx = current_idx + 1;
                                            app.history_index = Some(next_idx);
                                            app.input = app.history[next_idx].clone();
                                        }
                                    }
                                }
                                FocusBlock::Output => {
                                    if app.selected_output_index > 0 {
                                        app.selected_output_index -= 1;
                                    }
                                }
                                FocusBlock::History => { // Navigating history items when focus is on History block
                                    if !app.history.is_empty() {
                                       let new_idx = match app.history_index {
                                            Some(idx) if idx < app.history.len() -1 => idx + 1,
                                            Some(idx) => idx, // Stay at the end
                                            None => 0, // Start from beginning
                                       };
                                       app.history_index = Some(new_idx);
                                       app.input = app.history[new_idx].clone(); // Update input preview
                                    }
                                }
                            }
                        }

                        KeyCode::Down => {
                            match app.focus {
                                FocusBlock::Input => {
                                    if let Some(current_idx) = app.history_index {
                                        if current_idx > 0 {
                                            let prev_idx = current_idx - 1;
                                            app.history_index = Some(prev_idx);
                                            app.input = app.history[prev_idx].clone();
                                        } else { 
                                            app.history_index = None;
                                            app.input.clear();
                                        }
                                    }
                                }
                                FocusBlock::Output => {
                                    if !app.output.is_empty() && app.selected_output_index < app.output.len() - 1 {
                                        app.selected_output_index += 1;
                                    }
                                }
                                FocusBlock::History => { // Navigating history items
                                    if let Some(current_idx) = app.history_index {
                                        if current_idx > 0 {
                                            let prev_idx = current_idx - 1;
                                            app.history_index = Some(prev_idx);
                                            app.input = app.history[prev_idx].clone(); // Update input preview
                                        } else { // At the top of history, further Down could clear input or cycle
                                            app.history_index = None; // Go to "new input" state
                                            app.input.clear();
                                        }
                                    }
                                    // If history_index is None, Down does nothing (already at "new input")
                                }
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
                    AsyncResult::PathSearchResult(results) => {
                        app.output = results;
                        app.err_msg.clear();
                        app.selected_output_index = 0; // Reset selection to first item
                        if !app.output.is_empty() { // If results found, switch focus to output
                            app.focus = FocusBlock::Output;
                        } else { // No results, keep focus on input
                            app.focus = FocusBlock::Input;
                        }
                    }
                    AsyncResult::Error(err_text) => {
                        app.clear_prev();
                        app.err_msg = err_text;
                        app.focus = FocusBlock::Input; // Keep focus on input
                    }
                    _ => {
                        app.clear_prev();
                        app.err_msg = "Error receiving results from spawner".to_string();
                        app.focus = FocusBlock::Input; // Keep focus on input
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
                // app.err_msg = "Async communication channel disconnected.".to_string();
                // You might want to re-create the channel if you expect more tasks
                // or handle this as a critical error. For now, just stop loading.
                app.focus = FocusBlock::Input;
            }
        }
    }
}

fn ui(frame: &mut Frame, app: &App) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input area
            Constraint::Min(1),    // Output area (ensure at least 1 line)
            Constraint::Length(5), // History area (fixed height for demo)
        ])
        .split(frame.area());

    // Input Block styling based on focus
    let input_title_style = Style::default().fg(Color::Yellow);
    let input_border_style = if matches!(app.focus, FocusBlock::Input) || matches!(app.focus, FocusBlock::History){
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let input_paragraph = Paragraph::new(format!("> {}", app.input))
        .style(input_title_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search (Input)")
                .border_style(input_border_style),
        );
    frame.render_widget(input_paragraph, main_layout[0]);

    // Output Block styling and list creation
    let output_border_style = if matches!(app.focus, FocusBlock::Output) {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let output_block_base = Block::default()
        .borders(Borders::ALL)
        .title("Results (Output)")
        .border_style(output_border_style);

    if app.is_loading {
        frame.render_widget(
            Paragraph::new("Loading...").style(Style::default().fg(Color::Cyan)).block(output_block_base),
            main_layout[1],
        );
    } else if !app.err_msg.is_empty() {
        frame.render_widget(
            Paragraph::new(app.err_msg.as_str()) // Removed "Error: " prefix as it's in the message
                .style(Style::default().fg(Color::Red))
                .block(output_block_base),
            main_layout[1],
        );
    } else if !app.output.is_empty() {
        let items: Vec<ListItem> = app
            .output
            .iter()
            .enumerate()
            .map(|(i, res)| {
                let item_text = format!("[{}] {} :: {}", res.spawner, res.action, res.description);
                if matches!(app.focus, FocusBlock::Output) && i == app.selected_output_index {
                    ListItem::new(item_text).style(Style::default().fg(Color::Black).bg(Color::Cyan))
                } else {
                    ListItem::new(item_text).style(Style::default().fg(Color::Cyan))
                }
            })
            .collect();
        frame.render_widget(
            List::new(items).block(output_block_base), //.highlight_style already handled by item style
            main_layout[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new("No results. Type a query and press Enter.").style(Style::default().fg(Color::DarkGray)).block(output_block_base),
            main_layout[1],
        );
    }

    // History Block
    let history_border_style = if matches!(app.focus, FocusBlock::History) {
        Style::default().fg(Color::Green) // Highlight border when history is focused for navigation
    } else {
        Style::default()
    };
    let history_items: Vec<ListItem> = app.history.iter().enumerate().map(|(idx, entry)|{
        if matches!(app.focus, FocusBlock::History) && app.history_index == Some(idx) {
            ListItem::new(entry.as_str()).style(Style::default().fg(Color::Black).bg(Color::Magenta))
        } else {
            ListItem::new(entry.as_str()).style(Style::default().fg(Color::Gray))
        }
    }).collect();

    let history_list = List::new(history_items)
        .block(Block::default().borders(Borders::ALL).title("History").border_style(history_border_style));
    frame.render_widget(history_list, main_layout[2]);
}
