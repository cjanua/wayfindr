use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

pub const DEFAULT_LOG_FILE_PATH: &str = "/tmp/wayfindr_action.log_robust";

pub static LOG_TO_FILE: fn(String) = |message: String| {
    match OpenOptions::new().create(true).append(true).open(DEFAULT_LOG_FILE_PATH) {
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