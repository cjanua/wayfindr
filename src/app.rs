// src/app.rs
use crate::types::{ActionResult, AppError};
use crate::utils::LOG_TO_FILE;
use crate::process_execution; // For launching kitty for cd

#[derive(Debug, Clone, Copy, PartialEq, Eq)] // Added derive for FocusBlock
pub enum FocusBlock {
    Input,
    Output,
    History,
}

pub struct App {
    pub input: String,
    pub output: Vec<ActionResult>,
    pub history: Vec<String>,
    pub exit_flag: bool,
    pub err_msg: String,
    pub is_loading: bool,
    pub selected_output_index: usize,
    pub focus: FocusBlock,
    pub history_index: Option<usize>,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            output: vec![],
            history: vec![],
            exit_flag: false,
            err_msg: String::new(),
            is_loading: false,
            selected_output_index: 0,
            focus: FocusBlock::Input,
            history_index: None,
        }
    }

    pub fn clear_prev(&mut self) {
        self.output.clear();
        self.err_msg.clear();
    }

    // This is now the primary action execution logic tied to App state
    pub async fn handle_action_execution(&mut self, action_result: &ActionResult) -> Result<(), AppError> {
        LOG_TO_FILE(format!(
            "[APP_ACTION] Attempting to execute: spawner='{}', action='{}', data='{}'",
            action_result.spawner, action_result.action, action_result.data
        ));
    
        match action_result.spawner.as_str() {
            "z" | "fs" => {
                if action_result.action == "cd" {
                    let path_to_cd = &action_result.data;
                    match process_execution::launch_kitty_for_cd(path_to_cd) {
                        Ok(_) => {
                            // If successful, the run_app loop will set self.exit_flag = true
                            LOG_TO_FILE(format!("[APP_ACTION] Kitty for 'cd' launched successfully for path: {}", path_to_cd));
                             // The decision to exit is now made in run_app based on action_result.action == "cd"
                        }
                        Err(e) => {
                            LOG_TO_FILE(format!("[APP_ACTION] Failed to launch Kitty for 'cd'. Path: {}, Error: {}", path_to_cd, e));
                            // Propagate error to be displayed in UI
                            self.err_msg = format!("Failed to open Kitty for path '{}': {}", path_to_cd, e);
                            // It might be better to return the error for run_app to handle UI update for err_msg
                            return Err(AppError::ActionError(format!("Failed to open Kitty: {}", e)));
                        }
                    }
                    Ok(())
                } else {
                    let err_msg = format!("Unknown action '{}' for spawner '{}'", action_result.action, action_result.spawner);
                    LOG_TO_FILE(err_msg.clone());
                    self.err_msg = err_msg.clone();
                    Err(AppError::ActionError(err_msg))
                }
            }
            "app" => {
                if action_result.action == "launch" {
                    let exec_command = &action_result.data;
                    match process_execution::launch_application(exec_command) {
                        Ok(_) => {
                            LOG_TO_FILE(format!("[APP_ACTION] Application launched successfully: {}", exec_command));
                            // Don't exit for app launches, just clear and return to input
                        }
                        Err(e) => {
                            LOG_TO_FILE(format!("[APP_ACTION] Failed to launch application. Command: {}, Error: {}", exec_command, e));
                            self.err_msg = format!("Failed to launch application '{}': {}", exec_command, e);
                            return Err(AppError::ActionError(format!("Failed to launch application: {}", e)));
                        }
                    }
                    Ok(())
                } else {
                    let err_msg = format!("Unknown action '{}' for spawner '{}'", action_result.action, action_result.spawner);
                    LOG_TO_FILE(err_msg.clone());
                    self.err_msg = err_msg.clone();
                    Err(AppError::ActionError(err_msg))
                }
            }
            _ => {
                let err_msg = format!("Unknown spawner '{}'", action_result.spawner);
                LOG_TO_FILE(err_msg.clone());
                self.err_msg = err_msg.clone();
                Err(AppError::ActionError(err_msg))
            }
        }
    }
}