// src/process_execution.rs
// use std::path::Path;
use std::process::{Command as StdCommand};

use crate::utils::LOG_TO_FILE; // For logging within this module if needed


pub fn launch_kitty_for_cd(path_to_cd: &str) -> Result<(), std::io::Error> {
    LOG_TO_FILE(format!("[PROCESS_EXEC] Attempting to open Kitty and cd to: {}", path_to_cd));

    let shell_safe_path = format!("'{}'", path_to_cd.replace("'", r"'\''"));
    let command_for_hyprctl = format!(
        "kitty -d {} $SHELL",
        shell_safe_path
    );
    LOG_TO_FILE(format!("[PROCESS_EXEC] hyprctl dispatch exec {}", command_for_hyprctl));

    let output = StdCommand::new("hyprctl")
        .args(&["dispatch", "exec", &command_for_hyprctl])
        .output()?;

    if output.status.success() {
        LOG_TO_FILE(format!("[PROCESS_EXEC] Kitty launched to cd to: {}", path_to_cd));
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        LOG_TO_FILE(format!(
            "[PROCESS_EXEC] Error launching Kitty for cd {}: {}",
            path_to_cd, stderr
        ));
        Err(std::io::Error::new(std::io::ErrorKind::Other, "TerminalError"))
    }
}