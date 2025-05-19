// src/process_execution.rs
use std::path::Path;
use std::process::{Command as StdCommand, Stdio};
use crate::utils::LOG_TO_FILE; // For logging within this module if needed

// Note: AppError might need to be more generic or this module returns std::io::Error
// For now, let's assume these functions might be fallible and return io::Error or similar
// which can be converted to AppError higher up.

pub fn launch_kitty_to_view_file(file_path: &Path) -> Result<(), std::io::Error> {
    LOG_TO_FILE(format!(
        "[PROCESS_EXEC] Attempting to open logs from '{}' in Kitty...",
        file_path.display()
    ));

    let shell_command = format!(
        "cat '{}'; echo -e \"\n--- End of log. Press Ctrl+D or type 'exit' to close. ---\"; exec $SHELL",
        file_path.to_string_lossy()
    );

    let mut kitty_cmd = StdCommand::new("kitty");
    kitty_cmd.arg("-e")
             .arg("sh")
             .arg("-c")
             .arg(&shell_command);
    
    kitty_cmd.stdout(Stdio::null());
    kitty_cmd.stderr(Stdio::null());

    match kitty_cmd.spawn() {
        Ok(_) => {
            LOG_TO_FILE(format!(
                "[PROCESS_EXEC] Kitty launched to display file: {}",
                file_path.display()
            ));
            Ok(())
        }
        Err(e) => {
            LOG_TO_FILE(format!(
                "[PROCESS_EXEC] Error launching Kitty for file {}: {}",
                file_path.display(), e
            ));
            // Fallback could be eprintln, but this function is now generic.
            // The caller (cli module) handles specific eprintln fallback.
            Err(e)
        }
    }
}

pub fn launch_kitty_for_cd(path_to_cd: &str) -> Result<(), std::io::Error> {
    LOG_TO_FILE(format!("[PROCESS_EXEC] Attempting to open Kitty and cd to: {}", path_to_cd));

    let escaped_path = path_to_cd.replace("'", r"'\''");
    let shell_command = format!("cd '{}' && exec $SHELL", escaped_path);
    LOG_TO_FILE(format!("[PROCESS_EXEC] Shell command for Kitty: {}", shell_command));

    let mut cmd = StdCommand::new("kitty");
    cmd.arg("-e");
    cmd.arg("sh");
    cmd.arg("-c");
    cmd.arg(&shell_command);

    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    match cmd.spawn() {
        Ok(_) => {
            LOG_TO_FILE(format!(
                "[PROCESS_EXEC] Kitty spawn() call for 'cd' action returned Ok. Path: {}",
                path_to_cd
            ));
            Ok(())
        }
        Err(e) => {
            LOG_TO_FILE(format!(
                "[PROCESS_EXEC] Failed to spawn Kitty for 'cd' action. Path: {}, Error: {}",
                path_to_cd, e
            ));
            Err(e)
        }
    }
}