// src/cli.rs - Updated with interface selection
use crate::utils::DEFAULT_LOG_FILE_PATH;
use clap::{Parser, Subcommand};
use std::{collections::HashMap, path::{Path, PathBuf}};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    #[arg(long, value_name = "FILE_PATH", num_args = 0..=1, value_hint = clap::ValueHint::FilePath)]
    pub logs: Option<Option<PathBuf>>,
    
    #[arg(long, value_name = "FILE_PATH", num_args = 0..=1, value_hint = clap::ValueHint::FilePath)]
    pub usage: Option<Option<PathBuf>>,
    
    /// Use rofi interface instead of TUI
    #[arg(long)]
    pub rofi: bool,
    
    /// Specify interface type (tui, rofi)
    #[arg(long, value_name = "TYPE")]
    pub interface: Option<String>,
    
    #[command(subcommand)]
    pub provider: Option<ProviderCommands>,
}

#[derive(Subcommand, Debug)]
pub enum ProviderCommands {
    /// List all providers
    List,
    /// Enable a provider
    Enable { name: String },
    /// Disable a provider
    Disable { name: String },
    /// Show provider configuration
    Show { name: String },
    /// Create new provider from template
    Create { name: String },
    /// Test provider with query
    Test { name: String, query: String },
    /// Install default provider configurations
    InstallDefaults,
}

/// Parse CLI arguments and return the interface type to use
/// Returns `Ok((should_exit, interface_type))` 
pub fn handle_cli_args() -> Result<(bool, crate::interfaces::InterfaceType), anyhow::Error> {
    let cli_args = CliArgs::parse();

    // Handle --logs
    if let Some(option_for_path_or_default_signal) = cli_args.logs {
        let log_file_to_view: PathBuf = match option_for_path_or_default_signal {
            Some(specific_path) => specific_path,
            None => {
                let config = crate::config::get_config();
                config.paths.log_file.clone()
            }
        };

        if !log_file_to_view.exists() {
            eprintln!(
                "Error: Log file not found at '{}'",
                log_file_to_view.display()
            );
            eprintln!("Tip: The application writes logs to this file when actions are performed or if it's run without the --logs flag.");
            return Ok((true, crate::interfaces::InterfaceType::Tui)); // Exit early, interface doesn't matter
        }

        if let Ok(content) = std::fs::read_to_string(&log_file_to_view) {
            content.lines().for_each(|line| eprintln!("{}", line));
        }
        return Ok((true, crate::interfaces::InterfaceType::Tui)); // Exit after handling --logs
    }
    
    // Handle --usage
    if let Some(option_for_path_or_default_signal) = cli_args.usage {
        let usage_file_to_view: PathBuf = match option_for_path_or_default_signal {
            Some(specific_path) => specific_path,
            None => {
                let config = crate::config::get_config();
                config.paths.usage_stats_file.clone()
            }
        };

        if !usage_file_to_view.exists() {
            eprintln!(
                "📊 No usage statistics found at '{}'",
                usage_file_to_view.display()
            );
            eprintln!("💡 Tip: Usage statistics are created when you launch applications through wayfindr.");
            eprintln!("   Try launching some apps first, then check back!");
            return Ok((true, crate::interfaces::InterfaceType::Tui)); // Exit early
        }

        display_usage_statistics(&usage_file_to_view)?;
        return Ok((true, crate::interfaces::InterfaceType::Tui)); // Exit after handling --usage
    }
    
    // Handle --provider subcommands
    if let Some(provider_cmd) = cli_args.provider {
        crate::providers::management::handle_provider_command(provider_cmd)?;
        return Ok((true, crate::interfaces::InterfaceType::Tui)); // Exit after handling provider command
    }

    // Determine interface type
    let interface_type = if cli_args.rofi {
        crate::interfaces::InterfaceType::Rofi
    } else if let Some(interface_str) = cli_args.interface {
        interface_str.parse().map_err(|e| anyhow::anyhow!("Invalid interface type: {}", e))?
    } else {
        // Default to TUI
        crate::interfaces::InterfaceType::Tui
    };

    Ok((false, interface_type)) // Continue to main application
}

fn display_usage_statistics(usage_file_path: &PathBuf) -> Result<(), anyhow::Error> {
    use std::collections::HashMap;
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct UsageEntry {
        pub count: u32,
        pub last_used: DateTime<Utc>,
        pub first_used: DateTime<Utc>,
    }
    
    let content = std::fs::read_to_string(usage_file_path)?;
    let mut entries: HashMap<String, UsageEntry> = HashMap::new();
    
    // Parse the usage stats file
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        if let Some((key, json_data)) = line.split_once('|') {
            if let Ok(entry) = serde_json::from_str::<UsageEntry>(json_data) {
                entries.insert(key.to_string(), entry);
            }
        }
    }
    
    if entries.is_empty() {
        eprintln!("📊 No usage statistics found in the file.");
        eprintln!("💡 Launch some applications through wayfindr to see statistics here!");
        return Ok(());
    }
    
    // Load application names for better display
    let app_names = load_application_names();
    
    // Sort entries by usage count (descending)
    let mut sorted_entries: Vec<(&String, &UsageEntry)> = entries.iter().collect();
    sorted_entries.sort_by(|a, b| b.1.count.cmp(&a.1.count));
    
    // Display header
    eprintln!("📊 Wayfindr Usage Statistics");
    eprintln!("══════════════════════════════════════════════════════════════");
    eprintln!();
    
    // Display summary
    let total_apps = entries.len();
    let total_launches: u32 = entries.values().map(|e| e.count).sum();
    let most_used = sorted_entries.first().map(|(app_id, entry)| {
        let display_name = resolve_app_name(app_id, &app_names);
        (display_name, entry.count)
    });
    
    eprintln!("📈 Summary:");
    eprintln!("   Total applications used: {}", total_apps);
    eprintln!("   Total launches: {}", total_launches);
    if let Some((app_name, count)) = most_used {
        eprintln!("   Most used app: {} ({} launches)", app_name, count);
    }
    eprintln!();
    
    // Display top applications
    eprintln!("🏆 Top Applications:");
    eprintln!("   Rank  Usage Count  Last Used              Application");
    eprintln!("   ────  ───────────  ─────────────────────  ─────────────────────────────────────");
    
    for (rank, (app_id, entry)) in sorted_entries.iter().enumerate().take(20) {
        // Resolve the app name
        let display_name = resolve_app_name(app_id, &app_names);
        
        // Truncate long names but show more info
        let truncated_name = if display_name.len() > 35 {
            format!("{}...", &display_name[..32])
        } else {
            display_name
        };
        
        // Format last used date
        let last_used_str = entry.last_used.format("%Y-%m-%d %H:%M").to_string();
        
        // Calculate usage boost for context
        let usage_boost = match entry.count {
            0 => 0,
            1..=2 => 50,
            3..=5 => 150,
            6..=10 => 300,
            11..=20 => 500,
            21..=50 => 750,
            _ => 1000,
        };
        
        eprintln!(
            "   {:2}    {:11}  {}  {} (boost: +{})",
            rank + 1,
            entry.count,
            last_used_str,
            truncated_name,
            usage_boost
        );
    }
    
    if sorted_entries.len() > 20 {
        eprintln!("   ... and {} more applications", sorted_entries.len() - 20);
    }
    
    eprintln!();
    eprintln!("💡 Tips:");
    eprintln!("   • Frequently used apps appear higher in search results");
    eprintln!("   • Usage boost affects search ranking (shown above)");
    eprintln!("   • Clear stats with: rm '{}'", usage_file_path.display());
    eprintln!("   • View raw data: cat '{}'", usage_file_path.display());
    eprintln!("   • Try rofi interface: wayfindr --rofi");
    
    Ok(())
}

fn load_application_names() -> HashMap<String, String> {
    use std::fs;
    use std::path::Path;
    use std::collections::HashMap;
    
    let mut app_names = HashMap::new();
    
    // Search standard desktop application directories
    let search_paths = [
        "/usr/share/applications",
        "/usr/local/share/applications",
        "~/.local/share/applications",
    ];
    
    for path_str in &search_paths {
        let expanded_path = if path_str.starts_with("~") {
            // Simple tilde expansion
            if let Some(home) = std::env::var("HOME").ok() {
                path_str.replace("~", &home)
            } else {
                continue;
            }
        } else {
            path_str.to_string()
        };
        
        let path = Path::new(&expanded_path);
        if !path.exists() {
            continue;
        }
        
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_path = entry.path();
                
                if file_path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Some((app_id, app_name)) = parse_desktop_file(&file_path) {
                        app_names.insert(app_id, app_name);
                    }
                }
            }
        }
    }
    
    app_names
}

fn parse_desktop_file(path: &Path) -> Option<(String, String)> {
    use std::fs;
    
    let content = fs::read_to_string(path).ok()?;
    let mut app_name = String::new();
    let mut exec_command = String::new();
    
    let mut in_desktop_entry = false;
    for line in content.lines() {
        let line = line.trim();
        
        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        } else if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = false;
            continue;
        }
        
        if !in_desktop_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "Name" => app_name = value.to_string(),
                "Exec" => exec_command = value.to_string(),
                _ => {}
            }
        }
    }
    
    if !app_name.is_empty() && !exec_command.is_empty() {
        // Generate the same ID that wayfindr would generate
        let app_id = generate_app_id(&app_name);
        Some((app_id, app_name))
    } else {
        None
    }
}

fn generate_app_id(app_name: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    app_name.hash(&mut hasher);
    let hash = hasher.finish();
    
    format!("app_{:x}", hash)
}

fn resolve_app_name(app_id: &str, app_names: &HashMap<String, String>) -> String {
    // First, try to find the exact app name from desktop files
    if let Some(name) = app_names.get(app_id) {
        return name.clone();
    }
    
    // If not found, try to make the ID more readable
    if app_id.starts_with("app_") {
        let hash_part = app_id.strip_prefix("app_").unwrap_or(app_id);
        
        // For known common apps, try to reverse-engineer the name
        let common_apps = [
            ("brave", "Brave Browser"),
            ("firefox", "Firefox"),
            ("chrome", "Google Chrome"),
            ("chromium", "Chromium"),
            ("code", "Visual Studio Code"),
            ("kitty", "Kitty Terminal"),
            ("alacritty", "Alacritty Terminal"),
            ("nautilus", "Files (Nautilus)"),
            ("thunar", "Thunar File Manager"),
            ("spotify", "Spotify"),
            ("discord", "Discord"),
            ("slack", "Slack"),
            ("gimp", "GIMP"),
            ("libreoffice", "LibreOffice"),
            ("vlc", "VLC Media Player"),
        ];
        
        // Try to match against common app hashes
        for (app_name, display_name) in &common_apps {
            if generate_app_id(app_name) == *app_id {
                return display_name.to_string();
            }
        }
        
        // If we can't resolve it, show a more user-friendly format
        format!("App ({})", &hash_part[..8])
    } else {
        // For non-app IDs, just return as-is
        app_id.to_string()
    }
}