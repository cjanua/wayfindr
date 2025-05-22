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

    let output = hyprctl_dispatch_exec(&command_for_hyprctl)?;
    LOG_TO_FILE(format!("[PROCESS_EXEC] Output from command: {}", output));
    Ok(())
}

pub fn launch_application(exec_command: &str) -> Result<(), std::io::Error> {
    LOG_TO_FILE(format!("[PROCESS_EXEC] Launching application: {}", exec_command));

    // Parse the command and arguments
    let parts: Vec<&str> = exec_command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Empty command"
        ));
    }

    let command = parts[0];
    let args = &parts[1..];

    // Use hyprctl to launch the application
    let full_command = if args.is_empty() {
        command.to_string()
    } else {
        format!("{} {}", command, args.join(" "))
    };

    LOG_TO_FILE(format!("[PROCESS_EXEC] hyprctl dispatch exec {}", full_command));

    let output = hyprctl_dispatch_exec(&full_command)?;
    LOG_TO_FILE(format!("[PROCESS_EXEC] App launch output: {}", output));
    Ok(())
}


pub fn hyprctl_dispatch_exec(command: &str) -> Result<String, std::io::Error> {
    LOG_TO_FILE(format!("[PROCESS_EXEC] Executing command: {}", command));
    let output = StdCommand::new("hyprctl")
        .args(&["dispatch", "exec", command])
        .output()?;

    if output.status.success() {
        LOG_TO_FILE(format!("[PROCESS_EXEC] Command executed successfully: {}", command));
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        LOG_TO_FILE(format!(
            "[PROCESS_EXEC] Error executing command {}: {}",
            command, stderr
        ));
        Err(std::io::Error::new(std::io::ErrorKind::Other, "TerminalError"))
    }
}