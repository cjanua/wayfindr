// src/services/execution.rs
use crate::{
    config::get_config,
    services::usage,
    types::{ActionData, ActionResult, ActionType, AppResult},
    utils,
};
use std::process::Command;

pub struct ExecutionService;

impl ExecutionService {
    pub fn new() -> Self {
        Self
    }

    /// Execute an action and return whether the app should exit
    pub async fn execute(&self, action: &ActionResult) -> AppResult<bool> {
        utils::log_info(&format!(
            "Executing action: {} ({})",
            action.title, action.id
        ));

        usage::record_usage(&action.id);
        utils::log_debug(&format!("Recorded usage for action: {}", action.id));

        match &action.action {
            ActionType::Launch { needs_terminal } => {
                self.execute_launch(action, *needs_terminal).await
            }
            ActionType::Navigate { path } => self.execute_navigate(path).await,
            ActionType::AiResponse => {
                // AI responses don't need execution, just display
                Ok(false)
            }
            ActionType::Custom { action_id } => self.execute_custom(action_id, action).await,
        }
    }

    async fn execute_launch(&self, action: &ActionResult, needs_terminal: bool) -> AppResult<bool> {
        let command = match &action.data {
            ActionData::Command(cmd) => cmd,
            _ => {
                return Err(crate::types::AppError::ActionExecution(
                    "Launch action requires command data".to_string(),
                ))
            }
        };

        let config = get_config();

        if needs_terminal {
            // Launch in terminal
            let full_command = format!("{} -e {}", config.general.default_terminal, command);
            self.execute_system_command(&full_command).await?;
        } else {
            // Launch directly
            self.execute_system_command(command).await?;
        }

        utils::log_info(&format!("Successfully launched application: {}", action.title));
        Ok(true) // Exit wayfindr after successfully launching applications
    }

    async fn execute_navigate(&self, path: &str) -> AppResult<bool> {
        let config = get_config();

        // Navigate to directory using terminal
        let shell_safe_path = format!("'{}'", path.replace("'", r"'\''"));
        let command = format!(
            "{} -d {} $SHELL",
            config.general.default_terminal, shell_safe_path
        );

        self.execute_system_command(&command).await?;

        Ok(true) // Exit after navigation
    }

    async fn execute_custom(&self, action_id: &str, _action: &ActionResult) -> AppResult<bool> {
        utils::log_warn(&format!("Custom action not implemented: {}", action_id));
        Ok(false)
    }

    async fn execute_system_command(&self, command: &str) -> AppResult<()> {
        utils::log_debug(&format!("Executing system command: {}", command));

        // Use hyprctl dispatch exec for Hyprland integration
        let output = Command::new("hyprctl")
            .args(&["dispatch", "exec", command])
            .output()
            .map_err(|e| {
                crate::types::AppError::ActionExecution(format!("Failed to execute command: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::types::AppError::ActionExecution(format!(
                "Command failed: {}",
                stderr
            )));
        }

        utils::log_debug("Command executed successfully");
        Ok(())
    }
}

impl Default for ExecutionService {
    fn default() -> Self {
        Self::new()
    }
}
