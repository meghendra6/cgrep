// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration file support for cgrep
//!
//! Loads configuration from .cgreprc.toml in current directory or ~/.config/cgrep/config.toml

use serde::Deserialize;
use std::path::PathBuf;

/// Output format for results (mirrored from cli for library use)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigOutputFormat {
    #[default]
    Text,
    Json,
}

/// Configuration loaded from .cgreprc.toml or ~/.config/cgrep/config.toml
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Maximum number of results to return
    pub max_results: Option<usize>,
    /// Default output format (text or json)
    pub default_format: Option<String>,
    /// Patterns to exclude from search
    pub exclude_patterns: Vec<String>,
}

impl Config {
    /// Load configuration from files
    ///
    /// Precedence (highest to lowest):
    /// 1. .cgreprc.toml in current directory
    /// 2. ~/.config/cgrep/config.toml
    pub fn load() -> Self {
        // Try current directory first
        if let Some(config) = Self::load_from_path(&PathBuf::from(".cgreprc.toml")) {
            return config;
        }

        // Try home directory config
        if let Some(home) = dirs::home_dir() {
            let config_path = home.join(".config").join("cgrep").join("config.toml");
            if let Some(config) = Self::load_from_path(&config_path) {
                return config;
            }
        }

        Self::default()
    }

    fn load_from_path(path: &PathBuf) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        match toml::from_str(&content) {
            Ok(config) => Some(config),
            Err(e) => {
                eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
                None
            }
        }
    }

    /// Get output format from config, parsing the string to ConfigOutputFormat
    pub fn output_format(&self) -> Option<ConfigOutputFormat> {
        self.default_format.as_ref().and_then(|s| match s.to_lowercase().as_str() {
            "json" => Some(ConfigOutputFormat::Json),
            "text" => Some(ConfigOutputFormat::Text),
            _ => None,
        })
    }

    /// Merge CLI options with config (CLI wins)
    pub fn merge_max_results(&self, cli_value: Option<usize>) -> usize {
        cli_value
            .or(self.max_results)
            .unwrap_or(10)
    }
}
