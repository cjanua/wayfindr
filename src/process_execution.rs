// src/process_execution.rs
// use std::path::Path;
use std::process::{Command as StdCommand};

use crate::utils::LOG_TO_FILE; // For logging within this module if needed


pub fn launch_kitty_for_cd(path_to_cd: &str) -> Result<(), std::io::Error> {
    LOG_TO_FILE(format!("[PROCESS_EXEC] Attempting to open Kitty and cd to: {}", path_to_cd));

    let escaped_path = path_to_cd.replace("'", r"'\''");
    let shell_cmd = format!("cd '{}' && exec $SHELL", escaped_path);
    LOG_TO_FILE(format!("[PROCESS_EXEC] Shell command for Kitty: {}", shell_cmd));
    let command_for_hyprctl = format!(
        "kitty -e sh -c \"{}\"", // Note: The outer quotes are for Rust's format! string.
                                 // The \" becomes a literal " in command_for_hyprctl.
        shell_cmd
    );

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