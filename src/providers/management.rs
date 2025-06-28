// src/providers/management.rs
use crate::{cli::ProviderCommands, config::get_config};
use anyhow::Result;
use colored::*;
use std::fs;
use std::path::Path;

pub fn handle_provider_command(cmd: ProviderCommands) -> Result<()> {
    match cmd {
        ProviderCommands::List => list_providers(),
        ProviderCommands::Enable { name } => enable_provider(&name),
        ProviderCommands::Disable { name } => disable_provider(&name),
        ProviderCommands::Show { name } => show_provider(&name),
        ProviderCommands::Create { name } => create_provider(&name),
        ProviderCommands::Test { name, query } => test_provider(&name, &query),
        ProviderCommands::InstallDefaults => install_default_providers(),
    }
}

fn get_providers_dir() -> std::path::PathBuf {
    get_config().paths.config_dir.join("providers")
}

fn list_providers() -> Result<()> {
    let providers_dir = get_providers_dir();
    
    println!("{}", "Available providers:".green().bold());
    println!();
    
    // List dynamic providers
    if providers_dir.exists() {
        for entry in fs::read_dir(&providers_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                let content = fs::read_to_string(&path)?;
                let basename = path.file_stem().unwrap().to_string_lossy();
                
                // Parse basic info without full deserialization
                let enabled = content.contains("enabled = true");
                let name = extract_field(&content, "name").unwrap_or_else(|| basename.to_string());
                
                if enabled {
                    println!("  {} {} - {}", "✓".green(), basename, name);
                } else {
                    println!("  {} {} - {}", "✗".red(), basename.dimmed(), name.dimmed());
                }
            }
        }
    } else {
        println!("  No dynamic providers found. Use 'wayfindr --provider install-defaults' to get started.");
    }
    
    println!();
    println!("{}", "Built-in providers:".green().bold());
    println!("  {} applications - Application search", "✓".green());
    println!("  {} directories - Directory navigation (zoxide)", "✓".green());
    
    let ai_enabled = std::env::var("GEMINI_API_KEY").is_ok();
    if ai_enabled {
        println!("  {} ai - AI Assistant", "✓".green());
    } else {
        println!("  {} ai - AI Assistant (requires GEMINI_API_KEY)", "✗".red());
    }
    
    Ok(())
}

fn enable_provider(name: &str) -> Result<()> {
    let providers_dir = get_providers_dir();
    let file_path = providers_dir.join(format!("{}.toml", name));
    
    if !file_path.exists() {
        anyhow::bail!("Provider '{}' not found", name);
    }
    
    let content = fs::read_to_string(&file_path)?;
    let updated = content.replace("enabled = false", "enabled = true");
    fs::write(&file_path, updated)?;
    
    println!("{} Provider '{}' enabled", "✓".green(), name);
    Ok(())
}

fn disable_provider(name: &str) -> Result<()> {
    let providers_dir = get_providers_dir();
    let file_path = providers_dir.join(format!("{}.toml", name));
    
    if !file_path.exists() {
        anyhow::bail!("Provider '{}' not found", name);
    }
    
    let content = fs::read_to_string(&file_path)?;
    let updated = content.replace("enabled = true", "enabled = false");
    fs::write(&file_path, updated)?;
    
    println!("{} Provider '{}' disabled", "⚠".yellow(), name);
    Ok(())
}

fn show_provider(name: &str) -> Result<()> {
    let providers_dir = get_providers_dir();
    let file_path = providers_dir.join(format!("{}.toml", name));
    
    if !file_path.exists() {
        anyhow::bail!("Provider '{}' not found", name);
    }
    
    println!("{}: {}", "Provider".green().bold(), name);
    println!("{}: {}", "File".green(), file_path.display());
    println!();
    
    let content = fs::read_to_string(&file_path)?;
    println!("{}", content);
    
    Ok(())
}

fn create_provider(name: &str) -> Result<()> {
    let providers_dir = get_providers_dir();
    fs::create_dir_all(&providers_dir)?;
    
    let file_path = providers_dir.join(format!("{}.toml", name));
    
    if file_path.exists() {
        anyhow::bail!("Provider '{}' already exists", name);
    }
    
    let template = format!(r#"[provider]
id = "{}"
name = "My {} Provider"
priority = 50
enabled = false

[triggers]
# Add prefixes that trigger this provider (e.g., "weather:", "stock:")
prefixes = ["{}:"]
# Add patterns that indicate this provider should handle the query
patterns = ["{}"]

[api]
type = "rest"
base_url = "https://api.example.com"
# Set this to the environment variable name for your API key
# api_key_env = "MY_API_KEY"

# Optional headers
# [api.headers]
# "User-Agent" = "wayfindr/1.0"

[[commands]]
id = "search"
name = "Search {}"
endpoint = "/search"
method = "GET"
response_template = """
Results for {{{{query}}}}:
{{{{#each results}}}}
  - {{{{this.title}}}}
{{{{/each}}}}
"""

# Use {{{{query}}}} for user input, {{{{api_key}}}} for API key
[commands.params]
q = "{{{{query}}}}"

[[matchers]]
# Regex pattern to match queries
pattern = "^{}\\s+(.+)$"
command = "search"
# Which capture group contains the query (1-based)
query_group = 1
"#, name, name, name, name, name, name);
    
    fs::write(&file_path, template)?;
    
    println!("{} Created provider template: {}", "✓".green(), file_path.display());
    println!();
    println!("Edit this file to configure your provider, then enable it with:");
    println!("  wayfindr --provider enable {}", name);
    
    Ok(())
}

fn test_provider(name: &str, query: &str) -> Result<()> {
    let providers_dir = get_providers_dir();
    let file_path = providers_dir.join(format!("{}.toml", name));
    
    if !file_path.exists() {
        anyhow::bail!("Provider '{}' not found", name);
    }
    
    println!("{} provider '{}' with query: '{}'", "Testing".yellow(), name, query);
    println!("Note: This requires the provider to be enabled and have required API keys set");
    println!();
    println!("To actually test, run wayfindr and search for: {}", query);
    
    // TODO: Could implement a test mode that loads and runs just this provider
    
    Ok(())
}

fn install_default_providers() -> Result<()> {
    let providers_dir = get_providers_dir();
    fs::create_dir_all(&providers_dir)?;
    
    println!("{}", "Installing default provider configurations...".green().bold());
    println!();
    
    // Weather provider
    install_weather_provider(&providers_dir)?;
    
    // News provider
    install_news_provider(&providers_dir)?;
    
    // Calculator provider
    install_calculator_provider(&providers_dir)?;
    
    println!();
    println!("{}", "Done! Don't forget to:".green().bold());
    println!("  1. Set required API keys in your environment");
    println!("  2. Enable providers with: wayfindr --provider enable <n>");
    println!("  3. Set your location: export WAYFINDR_LOCATION='Your City'");
    
    Ok(())
}

fn install_weather_provider(providers_dir: &Path) -> Result<()> {
    let file_path = providers_dir.join("weather.toml");
    
    if file_path.exists() {
        println!("  {} weather.toml (already exists)", "↷".yellow());
        return Ok(());
    }
    
    let content = r#"[provider]
id = "weather"
name = "Weather Provider"
priority = 60
enabled = false  # Enable after setting OPENWEATHER_API_KEY

[triggers]
prefixes = ["weather:", "w:"]
patterns = ["weather", "temperature", "forecast"]

[api]
type = "rest"
base_url = "https://api.openweathermap.org/data/2.5"
api_key_env = "OPENWEATHER_API_KEY"

[[commands]]
id = "current"
name = "Current Weather"
endpoint = "/weather"
method = "GET"
response_template = """
🌡️ {{name}}: {{weather.0.description}}
Temp: {{main.temp}}°C (feels like {{main.feels_like}}°C)
Humidity: {{main.humidity}}% | Wind: {{wind.speed}} m/s"""

[commands.params]
q = "{{query|location}}"
appid = "{{api_key}}"
units = "metric"

[[matchers]]
pattern = "^weather$"
command = "current"
use_location = true

[[matchers]]
pattern = "^weather in (.+)$"
command = "current"
query_group = 1
"#;
    
    fs::write(&file_path, content)?;
    println!("  {} Created weather.toml", "✓".green());
    Ok(())
}

fn install_news_provider(providers_dir: &Path) -> Result<()> {
    let file_path = providers_dir.join("news.toml");
    
    if file_path.exists() {
        println!("  {} news.toml (already exists)", "↷".yellow());
        return Ok(());
    }
    
    let content = r#"[provider]
id = "news"
name = "News Headlines"
priority = 55
enabled = false  # Enable after setting NEWS_API_KEY

[triggers]
prefixes = ["news:", "n:"]
patterns = ["news", "headlines", "breaking"]

[api]
type = "rest"
base_url = "https://newsapi.org/v2"
api_key_env = "NEWS_API_KEY"

[[commands]]
id = "headlines"
name = "Top Headlines"
endpoint = "/top-headlines"
method = "GET"
response_template = """
📰 Top Headlines:
{{#each articles}}
  • {{title}} ({{source.name}})
{{/each}}"""

[commands.params]
apiKey = "{{api_key}}"
country = "us"
pageSize = "5"

[[matchers]]
pattern = "^news$"
command = "headlines"
"#;
    
    fs::write(&file_path, content)?;
    println!("  {} Created news.toml", "✓".green());
    Ok(())
}

fn install_calculator_provider(providers_dir: &Path) -> Result<()> {
    let file_path = providers_dir.join("calc.toml");
    
    if file_path.exists() {
        println!("  {} calc.toml (already exists)", "↷".yellow());
        return Ok(());
    }
    
    let content = r#"[provider]
id = "calc"
name = "Calculator"
priority = 70
enabled = true  # No API key required

[triggers]
prefixes = ["calc:", "="]
patterns = ["calculate", "compute"]

[api]
type = "rest"
base_url = "https://api.mathjs.org/v4"

[[commands]]
id = "evaluate"
name = "Calculate"
endpoint = "/"
method = "GET"
response_template = "🔢 {{query}} = {{result}}"

[commands.params]
expr = "{{query}}"

[[matchers]]
pattern = "^(.+)$"
command = "evaluate"
query_group = 1
"#;
    
    fs::write(&file_path, content)?;
    println!("  {} Created calc.toml", "✓".green());
    Ok(())
}

// Helper function to extract field value from TOML content
fn extract_field(content: &str, field: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(pos) = line.find(&format!("{} =", field)) {
            let value_part = &line[pos + field.len() + 2..].trim();
            if let Some(quoted) = value_part.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                return Some(quoted.to_string());
            }
        }
    }
    None
}