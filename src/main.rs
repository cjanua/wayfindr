// src/main.rs
use std::{fs::OpenOptions, io::{stdout, Stdout, Write}, time::SystemTime};
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
use ratatui::widgets::{List, ListItem}; // Add List and ListItem
use std::process::Command as StdCommand; // For spawning new termina

mod types;
use types::{ActionResult, AppError, AsyncResult};

mod spawners;
use spawners::path_search::spawn_path_search;

#[cfg(test)]
mod tests;

#[derive(Debug)]
enum FocusBlock {
    Input,
    Output,
    History,
}

static LOG_TO_FILE: fn(String) = |message: String| {
    let log_file_path = "/tmp/wayfindr_action.log_robust"; // Use a new, distinct name for this test
    match OpenOptions::new().create(true).append(true).open(log_file_path) {
        Ok(mut file) => {
            let time_now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // If writeln! fails, we do nothing here to avoid any terminal I/O.
            let _ = writeln!(file, "[{}] {}", time_now, message);
        }
        Err(_) => {
            // If opening the log file fails, also do nothing.
            // This prevents any eprintln! from interfering with the TUI.
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
        // File logging
        
    
        LOG_TO_FILE(format!(
            "[ACTION] Attempting to execute: spawner='{}', action='{}', data='{}'",
            action_result.spawner, action_result.action, action_result.data
        ));
    
        match action_result.spawner.as_str() {
            "z" | "fs" => {
                if action_result.action == "cd" {
                    let path_to_cd = &action_result.data;
                    LOG_TO_FILE(format!("Attempting to open terminal at: {}", path_to_cd));
    
                    #[cfg(target_os = "linux")]
                    {
                        let path_escaped_for_shell_arg = path_to_cd.replace("'", r"'\''");
                        LOG_TO_FILE(format!("[DEBUG] Original path: {}", path_to_cd));
                        LOG_TO_FILE(format!(
                            "[DEBUG] Path escaped for shell argument: {}",
                            path_escaped_for_shell_arg
                        ));
    
                        let mut success = false;
                        let attempts = [
                            ("kitty", "CwdExecShell", vec!["@", "launch", "--cwd"]),
                            ("xfce4-terminal", "WorkDirExecShell", vec!["--working-directory"]),
                            ("lxterminal", "WorkDirExecShell", vec!["--working-directory"]),
                            ("terminator", "WorkDirExecShell", vec!["--working-directory"]),
                            ("gnome-terminal", "ShellCommandWrapper", vec!["--", "bash", "-c"]),
                            ("konsole", "ShellCommandWrapper", vec!["-e", "bash", "-c"]),
                            ("xterm", "ShellCommandWrapper", vec!["-e", "bash", "-c"]),
                            ("alacritty", "ShellCommandWrapper", vec!["-e", "bash", "-c"]),
                            ("xfce4-terminal", "DirectCommandAppend", vec!["--command="]),
                        ];
    
                        for (term_exe, setup_type, base_args) in &attempts {
                            LOG_TO_FILE(format!("\n[DEBUG] === Trying: {} ===", term_exe));
                            let mut cmd = StdCommand::new(term_exe);
                            let shell_to_launch = "bash"; 
    
                            match *setup_type {
                                "CwdExecShell" => {
                                    cmd.args(base_args);
                                    cmd.arg(&path_escaped_for_shell_arg);
                                    cmd.arg(shell_to_launch);
                                }
                                "WorkDirExecShell" => {
                                    cmd.args(base_args);
                                    cmd.arg(&path_escaped_for_shell_arg);
                                    cmd.arg("-e"); // Common execute flag
                                    cmd.arg(shell_to_launch);
                                }
                                "ShellCommandWrapper" => {
                                    cmd.args(base_args);
                                    let shell_command_str = format!(
                                        "cd '{}' && exec {}",
                                        &path_escaped_for_shell_arg, shell_to_launch
                                    );
                                    LOG_TO_FILE(format!(
                                        "[DEBUG] Shell command string for wrapper: [{}]",
                                        shell_command_str
                                    ));
                                    cmd.arg(&shell_command_str);
                                }
                                "DirectCommandAppend" => {
                                    // This requires the shell_command_str to be doubly quoted if it contains spaces/special chars
                                    let inner_shell_command = format!(
                                        "cd '{}' && exec {}",
                                        &path_escaped_for_shell_arg, shell_to_launch
                                    );
                                    let outer_command_for_bash = format!("bash -c \"{}\"", inner_shell_command.replace("\"", "\\\""));
                                    let arg_with_command = format!("{}{}", base_args[0], outer_command_for_bash); // base_args[0] is like "--command="
                                    LOG_TO_FILE(format!(
                                        "[DEBUG] Argument for DirectCommandAppend: [{}]",
                                        arg_with_command
                                    ));
                                    cmd.arg(&arg_with_command);
                                }
                                _ => {
                                    LOG_TO_FILE(format!("[DEBUG] Unknown setup_type: {}", setup_type));
                                    continue;
                                }
                            }
    
                            let final_program_str = format!("{:?}", cmd.get_program());
                            let final_args_str_vec: Vec<String> = cmd
                                .get_args()
                                .map(|arg| format!("{:?}", arg.to_string_lossy()))
                                .collect();
                            LOG_TO_FILE(format!(
                                "[DEBUG] Final command to spawn: {} {}",
                                final_program_str,
                                final_args_str_vec.join(" ")
                            ));
    
                            match cmd.spawn() {
                                Ok(_) => {
                                    LOG_TO_FILE(format!(
                                        "[INFO] spawn() call for '{}' returned Ok. Path: {}",
                                        term_exe, path_to_cd
                                    ));
                                    success = true;
                                    break; 
                                }
                                Err(e) => {
                                    LOG_TO_FILE(format!("[DEBUG] Failed to spawn '{}': {}", term_exe, e));
                                }
                            }
                        }
    
                        if !success {
                            let err_msg = format!(
                                "Failed to open any known terminal for path: {}. Please cd manually.",
                                path_to_cd
                            );
                            LOG_TO_FILE(format!("[ERROR] {}",err_msg));
                            // You might want to set app.err_msg here to show it in the TUI
                            // For example: app.err_msg = err_msg; (needs mutable app access or different error handling)
                        }
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let path_escaped_for_osascript = path_to_cd.replace("'", r#"'\''"#);
                        LOG_TO_FILE(format!("[DEBUG] macOS: Original path: {}", path_to_cd));
                        LOG_TO_FILE(format!("[DEBUG] macOS: Path escaped for osascript: {}", path_escaped_for_osascript));
    
                        let script = format!(
                            "tell application \"Terminal\"\n\
                                \tdo script \"cd ''{}'' && clear\"\n\
                                \tactivate\n\
                            end tell",
                            path_escaped_for_osascript
                        );
                        LOG_TO_FILE(format!("[DEBUG] macOS: osascript script:\n{}", script));
                        match StdCommand::new("osascript").arg("-e").arg(&script).spawn() {
                            Ok(_) => {
                               LOG_TO_FILE(format!("[INFO] macOS: spawn() for osascript returned Ok. Path: {}", path_to_cd));
                            }
                            Err(e) => {
                                LOG_TO_FILE(format!("[ERROR] macOS: Failed to spawn osascript for path {}: {}", path_to_cd, e));
                            }
                        }
                    }
                    #[cfg(target_os = "windows")]
                    {
                        LOG_TO_FILE(format!("[DEBUG] Windows: Original path: {}", path_to_cd));
                        match StdCommand::new("cmd")
                            .arg("/c")
                            .arg("start")
                            .arg("cmd.exe")
                            .arg("/K")
                            .arg(format!("cd /d \"{}\"", path_to_cd))
                            .spawn() {
                            Ok(_) => {
                                LOG_TO_FILE(format!("[INFO] Windows: spawn() for cmd.exe returned Ok. Path: {}", path_to_cd));
                            }
                            Err(e) => {
                                 LOG_TO_FILE(format!("[ERROR] Windows: Failed to spawn cmd.exe for path {}: {}", path_to_cd, e));
                            }
                        }
                    }
                    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
                    {
                        LOG_TO_FILE(format!("[INFO] Simulated execution (unknown OS): cd \"{}\"", path_to_cd));
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
        // eprintln!("Panic occurred: {:?}", panic_info);

        // Call the original panic hook, which prints the panic message and backtrace.
        original_hook(panic_info);
    }));

    // Setup terminal
    setup_terminal().context("Failed to setup terminal")?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut app = App::new();

    let app_result = run_app(&mut terminal, &mut app).await;
    if let Err(_e) = restore_terminal() {
        // eprintln!("[main] FATAL: Failed to restore terminal: {:?}", e);
    }

    app_result.map_err(|app_err| {
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

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
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
                                        app.selected_output_index = 0; // Optionally reset
                                    } else {
                                        app.focus = FocusBlock::History;
                                    }
                                }
                                FocusBlock::Output => app.focus = FocusBlock::History,
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
                                    if !query.is_empty() { // Only add non-empty queries to history
                                        app.history.insert(0, query.clone());
                                        if app.history.len() > 16 {
                                            app.history.pop();
                                        }
                                    }

                                    app.input.clear();
                                    app.clear_prev();
                                    app.history_index=None;

                                    if !query.is_empty() {
                                        spawn_path_search(query, sender.clone());
                                        app.is_loading = true;
                                    } else {
                                        // Execute action on the first result if input is empty on Enter
                                        if let Some(selected_action) = app.output.get(app.selected_output_index) {
                                            app.execute_action(selected_action).await?;
                                            // Potentially set app.exit_flag = true if action implies exit
                                        }
                                    }
                                }
                                FocusBlock::Output => {
                                    if let Some(selected_action) = app.output.get(app.selected_output_index) {
                                        LOG_TO_FILE(format!("[ACTION EXEC] Executing selected output: {:?}", selected_action.description));
                                        match app.execute_action(selected_action).await {
                                            Ok(_) => {
                                                LOG_TO_FILE("[ACTION EXEC] execute_action successful.".to_string());
                                                // DECIDE WHAT HAPPENS NEXT:
                                                // Option 1: Exit the application
                                                // app.exit_flag = true;
                                
                                                // Option 2: Clear results and return to input
                                                app.output.clear();
                                                app.err_msg.clear(); // Clear any previous error
                                                app.selected_output_index = 0;
                                                app.focus = FocusBlock::Input;
                                                LOG_TO_FILE("[ACTION EXEC] Cleared output, focus set to Input.".to_string());
                                
                                                // Option 3: Do nothing and stay (current behavior, likely problematic)
                                            }
                                            Err(e) => {
                                                LOG_TO_FILE(format!("[ACTION EXEC] execute_action failed: {:?}", e));
                                                app.err_msg = format!("Action failed: {:?}", e);
                                                // Keep focus on output to show error, or switch to input
                                                app.focus = FocusBlock::Input; // Or maybe keep Focus::Output to see error
                                            }
                                        }
                                    } else {
                                        LOG_TO_FILE("[ACTION EXEC] Enter in Output focus, but no item selected or output empty.".to_string());
                                    }
                                }
                                
                                FocusBlock::History => {
                                    if let Some(index) = app.history_index {
                                        app.input = app.history[index].clone();
                                        app.history_index = None;
                                    }
                                }
                            }
                        }
                        KeyCode::Up => {
                            match app.focus {
                                FocusBlock::Input => {
                                    if !app.history.is_empty() {
                                        let current_idx = app.history_index.unwrap_or(0); // Start from 0 if None
                                        if app.history_index.is_none() { // First Up press
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
                                FocusBlock::History => {
                                    if let Some(index) = app.history_index {
                                        if index < app.history.len() - 1 {
                                            app.history_index = Some(index + 1);
                                            app.input = app.history[index + 1].clone();
                                        }
                                    } else {
                                        app.history_index = Some(0);
                                        app.input = app.history[0].clone();
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
                                        } else { // Was at history[0], go to empty input
                                            app.history_index = None;
                                            app.input.clear();
                                        }
                                    }
                                    // If history_index is None, Down does nothing (already at empty input)
                                }
                                FocusBlock::Output => {
                                    if !app.output.is_empty() && app.selected_output_index < app.output.len() - 1 {
                                        app.selected_output_index += 1;
                                    }
                                }
                                FocusBlock::History => {
                                    if let Some(index) = app.history_index {
                                        if index > 0 {
                                            app.history_index = Some(index - 1);
                                            app.input = app.history[index - 1].clone();
                                        } else {
                                            app.history_index = None;
                                            app.input.clear();
                                        }
                                    } else if !app.history.is_empty() {
                                        app.history_index = Some(app.history.len() - 1);
                                        app.input = app.history[app.history.len() - 1].clone();
                                    }
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
                app.err_msg = "Async communication channel disconnected.".to_string();
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
            Constraint::Length(4), // Output area
            Constraint::Min(0),    // History area
        ])
        .split(frame.area());

    // Input Block styling based on focus
    let input_title_style = Style::default().fg(Color::Yellow);
    let input_border_style = if matches!(app.focus, FocusBlock::Input) {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };

    let input_text = format!("> {}", app.input);
    let input_paragraph = Paragraph::new(input_text)
        .style(input_title_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search")
                .border_style(input_border_style),
        );
    frame.render_widget(input_paragraph, main_layout[0]);

    // Output Block styling and list creation
    let output_title_style = Style::default().fg(Color::Cyan);
    let output_border_style = if matches!(app.focus, FocusBlock::Output) {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };

    let output_block_base = Block::default()
        .borders(Borders::ALL)
        .title("Output")
        .border_style(output_border_style);

    if app.is_loading {
        frame.render_widget(
            Paragraph::new("Loading...").style(output_title_style).block(output_block_base),
            main_layout[1],
        );
    } else if !app.err_msg.is_empty() {
        frame.render_widget(
            Paragraph::new(format!("Error: {}", app.err_msg))
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
                let item_text = format!("[{}] {} {}", res.spawner, res.action, res.description);
                if matches!(app.focus, FocusBlock::Output) && i == app.selected_output_index {
                    ListItem::new(item_text).style(Style::default().fg(Color::Black).bg(Color::Cyan))
                } else {
                    ListItem::new(item_text).style(output_title_style)
                }
            })
            .collect();
        frame.render_widget(
            List::new(items).block(output_block_base).highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan),
            ),
            main_layout[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new("No results").style(output_title_style).block(output_block_base),
            main_layout[1],
        );
    }

    // History Block
    let history_text = app.history.iter().take(10).cloned().collect::<Vec<String>>().join("\n");
    let history_block = Paragraph::new(history_text)
        .block(Block::default().borders(Borders::ALL).title("History"))
        .style(Style::default().fg(Color::White));
    frame.render_widget(history_block, main_layout[2]);
}
