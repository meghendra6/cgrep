// SPDX-License-Identifier: MIT OR Apache-2.0

//! Configuration file support for cgrep
//!
//! Loads configuration from .cgreprc.toml in current directory or ~/.config/cgrep/config.toml

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Output format for results (mirrored from cli for library use)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigOutputFormat {
    #[default]
    Text,
    Json,
    Json2,
}

/// Search mode for queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    #[default]
    Keyword,
    Semantic,
    Hybrid,
}

/// Embedding feature enablement mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingEnabled {
    Off,
    #[default]
    Auto,
    On,
}

/// Embedding provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingProviderType {
    #[default]
    Builtin,
    Dummy,
    /// Command provider (external process).
    Command,
}

/// Search configuration
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    /// Default search mode (keyword, semantic, hybrid)
    pub default_mode: Option<SearchMode>,
    /// Number of candidates to fetch for reranking in hybrid mode
    pub candidate_k: Option<usize>,
    /// Weight for text/keyword scoring in hybrid mode (0.0-1.0)
    pub weight_text: Option<f32>,
    /// Weight for vector/semantic scoring in hybrid mode (0.0-1.0)
    pub weight_vector: Option<f32>,
}

impl SearchConfig {
    /// Get default search mode (defaults to Keyword)
    pub fn mode(&self) -> SearchMode {
        self.default_mode.unwrap_or_default()
    }

    /// Get candidate k for hybrid search (defaults to 200)
    pub fn candidate_k(&self) -> usize {
        self.candidate_k.unwrap_or(200)
    }

    /// Get text weight for hybrid scoring (defaults to 0.7)
    pub fn weight_text(&self) -> f32 {
        self.weight_text.unwrap_or(0.7)
    }

    /// Get vector weight for hybrid scoring (defaults to 0.3)
    pub fn weight_vector(&self) -> f32 {
        self.weight_vector.unwrap_or(0.3)
    }
}

/// Keyword ranking configuration (non-embedding signals).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RankingConfig {
    /// Enable multi-signal keyword ranking.
    pub enabled: Option<bool>,
    /// Path/filename token overlap boost weight.
    pub path_weight: Option<f32>,
    /// Symbol exact/prefix boost weight.
    pub symbol_weight: Option<f32>,
    /// Language filter match boost weight.
    pub language_weight: Option<f32>,
    /// Changed-files boost weight.
    pub changed_weight: Option<f32>,
    /// Kind boost weight for identifier-like queries.
    pub kind_weight: Option<f32>,
    /// Penalty weight for weak identifier matches.
    pub weak_signal_penalty: Option<f32>,
    /// Number of top results with score explanation.
    pub explain_top_k: Option<usize>,
}

impl RankingConfig {
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(false)
    }

    pub fn path_weight(&self) -> f32 {
        clamp_weight(self.path_weight, 1.0, 0.0, 3.0)
    }

    pub fn symbol_weight(&self) -> f32 {
        clamp_weight(self.symbol_weight, 1.0, 0.0, 3.0)
    }

    pub fn language_weight(&self) -> f32 {
        clamp_weight(self.language_weight, 1.0, 0.0, 3.0)
    }

    pub fn changed_weight(&self) -> f32 {
        clamp_weight(self.changed_weight, 1.0, 0.0, 3.0)
    }

    pub fn kind_weight(&self) -> f32 {
        clamp_weight(self.kind_weight, 1.0, 0.0, 3.0)
    }

    pub fn weak_signal_penalty(&self) -> f32 {
        clamp_weight(self.weak_signal_penalty, 1.0, 0.0, 3.0)
    }

    pub fn explain_top_k(&self) -> usize {
        self.explain_top_k
            .filter(|value| (1..=50).contains(value))
            .unwrap_or(5)
    }
}

fn clamp_weight(value: Option<f32>, default: f32, min: f32, max: f32) -> f32 {
    let raw = value.unwrap_or(default);
    if !raw.is_finite() {
        return default;
    }
    raw.clamp(min, max)
}

/// Embedding configuration
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct EmbeddingConfig {
    /// Whether embeddings are enabled (off, auto, on)
    pub enabled: Option<EmbeddingEnabled>,
    /// Provider type (builtin, command, dummy)
    pub provider: Option<EmbeddingProviderType>,
    /// Provider inference batch size (builtin fastembed)
    pub batch_size: Option<usize>,
    /// Maximum characters per text passed to provider (builtin fastembed)
    pub max_chars: Option<usize>,
    /// Model identifier for the embedding provider (used by command provider)
    pub model: Option<String>,
    /// Command to execute for command provider
    pub command: Option<String>,
    /// Number of lines per chunk
    pub chunk_lines: Option<usize>,
    /// Number of overlap lines between chunks
    pub chunk_overlap: Option<usize>,
    /// Maximum file size in bytes to process
    pub max_file_bytes: Option<usize>,
    /// Maximum number of chunks for semantic search
    pub semantic_max_chunks: Option<usize>,
    /// Maximum number of symbols per file to embed
    pub max_symbols_per_file: Option<usize>,
    /// Maximum preview lines per symbol when building embedding text
    pub symbol_preview_lines: Option<usize>,
    /// Maximum characters per symbol when building embedding text
    pub symbol_max_chars: Option<usize>,
    /// Allowlist of symbol kinds to embed (e.g., ["function","class"])
    pub symbol_kinds: Option<Vec<String>>,
}

impl EmbeddingConfig {
    /// Get enabled mode (defaults to Auto)
    pub fn enabled(&self) -> EmbeddingEnabled {
        self.enabled.unwrap_or_default()
    }

    /// Get provider type (defaults to Builtin)
    pub fn provider(&self) -> EmbeddingProviderType {
        self.provider.unwrap_or_default()
    }

    /// Get provider batch size override (if configured)
    pub fn batch_size(&self) -> Option<usize> {
        self.batch_size
    }

    /// Get provider max chars override (if configured)
    pub fn max_chars(&self) -> Option<usize> {
        self.max_chars
    }

    /// Get model identifier (defaults to "local-model-id")
    pub fn model(&self) -> &str {
        self.model.as_deref().unwrap_or("local-model-id")
    }

    /// Get command (defaults to "embedder")
    pub fn command(&self) -> &str {
        self.command.as_deref().unwrap_or("embedder")
    }

    /// Get chunk lines (defaults to 80)
    pub fn chunk_lines(&self) -> usize {
        self.chunk_lines.unwrap_or(80)
    }

    /// Get chunk overlap (defaults to 20)
    pub fn chunk_overlap(&self) -> usize {
        self.chunk_overlap.unwrap_or(20)
    }

    /// Get max file bytes (defaults to 2MB)
    pub fn max_file_bytes(&self) -> usize {
        self.max_file_bytes.unwrap_or(2_000_000)
    }

    /// Get semantic max chunks (defaults to 200000)
    pub fn semantic_max_chunks(&self) -> usize {
        self.semantic_max_chunks.unwrap_or(200_000)
    }

    /// Get max symbols per file (defaults to 500)
    pub fn max_symbols_per_file(&self) -> usize {
        self.max_symbols_per_file.unwrap_or(500)
    }

    /// Get symbol preview lines (defaults to 12)
    pub fn symbol_preview_lines(&self) -> usize {
        self.symbol_preview_lines.unwrap_or(12)
    }

    /// Get symbol max chars (defaults to 1200)
    pub fn symbol_max_chars(&self) -> usize {
        self.symbol_max_chars.unwrap_or(1200)
    }

    /// Get allowlist of symbol kinds (lowercased)
    pub fn symbol_kinds(&self) -> Option<Vec<String>> {
        self.symbol_kinds
            .as_ref()
            .map(|kinds| kinds.iter().map(|k| k.to_lowercase()).collect::<Vec<_>>())
    }
}

/// Indexing configuration
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct IndexConfig {
    /// Paths/patterns to exclude from indexing
    pub exclude_paths: Vec<String>,
    /// Maximum file size in bytes to index (default: 1MB)
    pub max_file_size: Option<u64>,
    /// Whether index build should respect .gitignore/.ignore rules
    pub respect_git_ignore: Option<bool>,
}

impl IndexConfig {
    /// Get exclude paths
    pub fn exclude_paths(&self) -> &[String] {
        &self.exclude_paths
    }

    /// Get max file size (default: 1MB)
    pub fn max_file_size(&self) -> u64 {
        self.max_file_size.unwrap_or(1024 * 1024)
    }

    /// Whether index build should respect ignore files (default: true)
    pub fn respect_git_ignore(&self) -> bool {
        self.respect_git_ignore.unwrap_or(true)
    }
}

/// Cache configuration
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: Option<bool>,
    /// Cache TTL in milliseconds
    pub ttl_ms: Option<u64>,
}

impl CacheConfig {
    /// Get enabled (defaults to true)
    pub fn enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    /// Get TTL in milliseconds (defaults to 600000 = 10 minutes)
    pub fn ttl_ms(&self) -> u64 {
        self.ttl_ms.unwrap_or(600_000)
    }
}

/// Profile configuration for different usage modes
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProfileConfig {
    /// Output format for this profile
    pub format: Option<ConfigOutputFormat>,
    /// Context lines around matches
    pub context: Option<usize>,
    /// Context pack size for agent mode
    pub context_pack: Option<usize>,
    /// Maximum results for this profile
    pub max_results: Option<usize>,
    /// Search mode for this profile
    pub mode: Option<SearchMode>,
    /// Whether to use agent caching (for agent profile)
    pub agent_cache: Option<bool>,
}

impl ProfileConfig {
    /// Create the "human" profile preset
    pub fn human() -> Self {
        Self {
            format: Some(ConfigOutputFormat::Text),
            context: Some(2),
            context_pack: None,
            max_results: Some(20),
            mode: Some(SearchMode::Keyword),
            agent_cache: None,
        }
    }

    /// Create the "agent" profile preset
    pub fn agent() -> Self {
        Self {
            format: Some(ConfigOutputFormat::Json2),
            context: Some(6),
            context_pack: Some(8),
            max_results: Some(50),
            mode: Some(SearchMode::Hybrid),
            agent_cache: Some(true),
        }
    }

    /// Create the "fast" profile preset (for quick exploration)
    pub fn fast() -> Self {
        Self {
            format: Some(ConfigOutputFormat::Text),
            context: Some(0),
            context_pack: None,
            max_results: Some(10),
            mode: Some(SearchMode::Keyword),
            agent_cache: None,
        }
    }

    /// Get format (defaults to Text)
    pub fn format(&self) -> ConfigOutputFormat {
        self.format.unwrap_or_default()
    }

    /// Get context lines (defaults to 2)
    pub fn context(&self) -> usize {
        self.context.unwrap_or(2)
    }

    /// Get context pack size (defaults to context value)
    pub fn context_pack(&self) -> usize {
        self.context_pack.unwrap_or_else(|| self.context())
    }

    /// Get max results (defaults to 20)
    pub fn max_results(&self) -> usize {
        self.max_results.unwrap_or(20)
    }

    /// Get search mode (defaults to Keyword)
    pub fn mode(&self) -> SearchMode {
        self.mode.unwrap_or_default()
    }

    /// Get agent cache setting (defaults to false)
    pub fn agent_cache(&self) -> bool {
        self.agent_cache.unwrap_or(false)
    }
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

    /// Search configuration
    #[serde(default)]
    pub search: SearchConfig,

    /// Embedding configuration
    #[serde(default)]
    pub embeddings: EmbeddingConfig,

    /// Cache configuration
    #[serde(default)]
    pub cache: CacheConfig,

    /// Index configuration
    #[serde(default)]
    pub index: IndexConfig,

    /// Ranking configuration
    #[serde(default)]
    pub ranking: RankingConfig,

    /// Named profiles (e.g., "human", "agent", "fast")
    #[serde(default, rename = "profile")]
    pub profiles: HashMap<String, ProfileConfig>,
}

impl Config {
    /// Load configuration from files
    ///
    /// Precedence (highest to lowest):
    /// 1. .cgreprc.toml in current directory
    /// 2. ~/.config/cgrep/config.toml
    pub fn load() -> Self {
        Self::load_for_dir(PathBuf::from("."))
    }

    /// Load configuration relative to a given directory.
    ///
    /// Precedence (highest to lowest):
    /// 1. <dir>/.cgreprc.toml
    /// 2. ~/.config/cgrep/config.toml
    pub fn load_for_dir(dir: impl AsRef<std::path::Path>) -> Self {
        let dir = dir.as_ref();

        // Try project-local config first
        let local_path = dir.join(".cgreprc.toml");
        if let Some(config) = Self::load_from_path(&local_path) {
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

    fn load_from_path(path: &std::path::Path) -> Option<Self> {
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
        self.default_format
            .as_ref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "json" => Some(ConfigOutputFormat::Json),
                "json2" => Some(ConfigOutputFormat::Json2),
                "text" => Some(ConfigOutputFormat::Text),
                _ => None,
            })
    }

    /// Merge CLI options with config (CLI wins)
    pub fn merge_max_results(&self, cli_value: Option<usize>) -> usize {
        cli_value.or(self.max_results).unwrap_or(10)
    }

    /// Get a profile by name, falling back to built-in presets
    pub fn profile(&self, name: &str) -> ProfileConfig {
        if let Some(profile) = self.profiles.get(name) {
            profile.clone()
        } else {
            // Built-in presets
            match name {
                "human" => ProfileConfig::human(),
                "agent" => ProfileConfig::agent(),
                "fast" => ProfileConfig::fast(),
                _ => ProfileConfig::default(),
            }
        }
    }

    /// Get the search configuration
    pub fn search(&self) -> &SearchConfig {
        &self.search
    }

    /// Get the embedding configuration
    pub fn embeddings(&self) -> &EmbeddingConfig {
        &self.embeddings
    }

    /// Get the cache configuration
    pub fn cache(&self) -> &CacheConfig {
        &self.cache
    }

    /// Get the index configuration
    pub fn index(&self) -> &IndexConfig {
        &self.index
    }

    /// Get the ranking configuration
    pub fn ranking(&self) -> &RankingConfig {
        &self.ranking
    }

    /// Check if embeddings should be enabled based on configuration and environment
    pub fn embeddings_enabled(&self) -> bool {
        match self.embeddings.enabled() {
            EmbeddingEnabled::Off => false,
            EmbeddingEnabled::On => true,
            EmbeddingEnabled::Auto => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranking_config_defaults_to_compatibility_mode() {
        let cfg = Config::default();
        assert!(!cfg.ranking().enabled());
        assert_eq!(cfg.ranking().path_weight(), 1.0);
        assert_eq!(cfg.ranking().symbol_weight(), 1.0);
        assert_eq!(cfg.ranking().language_weight(), 1.0);
        assert_eq!(cfg.ranking().changed_weight(), 1.0);
        assert_eq!(cfg.ranking().kind_weight(), 1.0);
        assert_eq!(cfg.ranking().weak_signal_penalty(), 1.0);
        assert_eq!(cfg.ranking().explain_top_k(), 5);
    }

    #[test]
    fn ranking_config_clamps_invalid_values() {
        let cfg: Config = toml::from_str(
            r#"
[ranking]
enabled = true
path_weight = -10.0
symbol_weight = 999.0
language_weight = nan
changed_weight = inf
kind_weight = 2.5
weak_signal_penalty = -0.5
explain_top_k = 0
"#,
        )
        .expect("parse config");

        assert!(cfg.ranking().enabled());
        assert_eq!(cfg.ranking().path_weight(), 0.0);
        assert_eq!(cfg.ranking().symbol_weight(), 3.0);
        assert_eq!(cfg.ranking().language_weight(), 1.0);
        assert_eq!(cfg.ranking().changed_weight(), 1.0);
        assert_eq!(cfg.ranking().kind_weight(), 2.5);
        assert_eq!(cfg.ranking().weak_signal_penalty(), 0.0);
        assert_eq!(cfg.ranking().explain_top_k(), 5);
    }
}
