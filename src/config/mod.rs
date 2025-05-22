// src/config/mod.rs
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR_NAME: &str = ".wayfindr";
const CONFIG_FILE_NAME: &str = "config.toml";
const DEFAULT_TERMINAL: &str = "kitty";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub search: SearchConfig,
    pub ui: UiConfig,
    pub paths: PathsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub default_terminal: String,
    pub log_level: LogLevel,
    pub max_results: usize,
    pub history_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub ai_prefix: String,
    pub app_prefix: String,
    pub fuzzy_threshold: f32,
    pub enable_live_search: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub show_icons: bool,
    pub show_categories: bool,
    pub animate_transitions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    pub config_dir: PathBuf,
    pub log_file: PathBuf,
    pub usage_stats_file: PathBuf,
    pub cache_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

impl Default for Config {
    fn default() -> Self {
        let config_dir = get_config_dir();
        Self {
            general: GeneralConfig {
                default_terminal: DEFAULT_TERMINAL.to_string(),
                log_level: LogLevel::Info,
                max_results: 50,
                history_limit: 16,
            },
            search: SearchConfig {
                ai_prefix: "ai:".to_string(),
                app_prefix: "app:".to_string(),
                fuzzy_threshold: 0.6,
                enable_live_search: true,
            },
            ui: UiConfig {
                show_icons: false,
                show_categories: true,
                animate_transitions: false,
            },
            paths: PathsConfig {
                config_dir: config_dir.clone(),
                log_file: config_dir.join("wayfindr.log"),
                usage_stats_file: config_dir.join("usage_stats.txt"),
                cache_dir: config_dir.join("cache"),
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = get_config_file_path();

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).context("Failed to read config file")?;
            let mut config: Config =
                toml::from_str(&content).context("Failed to parse config file")?;

            // Update paths to be absolute
            config.paths.config_dir = get_config_dir();
            config.paths.log_file = config.paths.config_dir.join("wayfindr.log");
            config.paths.usage_stats_file = config.paths.config_dir.join("usage_stats.txt");
            config.paths.cache_dir = config.paths.config_dir.join("cache");

            Ok(config)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = get_config_dir();
        fs::create_dir_all(&config_dir).context("Failed to create config directory")?;

        // Ensure other directories exist
        fs::create_dir_all(&self.paths.cache_dir).context("Failed to create cache directory")?;

        let config_path = get_config_file_path();
        let toml_content = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&config_path, toml_content).context("Failed to write config file")?;

        Ok(())
    }
}

fn get_config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(CONFIG_DIR_NAME)
}

fn get_config_file_path() -> PathBuf {
    get_config_dir().join(CONFIG_FILE_NAME)
}

// Global config instance
use std::sync::OnceLock;
static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn init_config() -> Result<()> {
    let config = Config::load()?;
    CONFIG
        .set(config)
        .map_err(|_| anyhow::anyhow!("Config already initialized"))?;
    Ok(())
}

pub fn get_config() -> &'static Config {
    CONFIG
        .get()
        .expect("Config not initialized. Call init_config() first.")
}
