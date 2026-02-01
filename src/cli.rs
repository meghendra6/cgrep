//! CLI argument parsing using clap

use clap::{Parser, Subcommand};

/// lgrep - Local semantic code search tool
///
/// A high-performance search tool combining AST analysis with BM25 ranking.
/// Supports symbol search, dependency tracking, and full-text search.
#[derive(Parser, Debug)]
#[command(name = "lgrep")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Output format (text or json)
    #[arg(long, default_value = "text", global = true)]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

/// Output format for results
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Full-text search with BM25 ranking
    #[command(alias = "s")]
    Search {
        /// Search query (natural language or keywords)
        query: String,

        /// Path to search in (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Maximum number of results
        #[arg(short, long, default_value = "20")]
        max_results: usize,

        /// Include surrounding context lines
        #[arg(short, long, default_value = "3")]
        context: usize,
    },

    /// Search for symbols (functions, classes, etc.)
    Symbols {
        /// Symbol name to search for
        name: String,

        /// Filter by symbol type (function, class, variable, etc.)
        #[arg(short = 't', long = "type")]
        symbol_type: Option<String>,

        /// Filter by language (typescript, python, rust, etc.)
        #[arg(short, long)]
        lang: Option<String>,
    },

    /// Find symbol definition location
    #[command(alias = "def")]
    Definition {
        /// Symbol name to find definition for
        name: String,
    },

    /// Find all callers of a function
    Callers {
        /// Function name to find callers for
        function: String,
    },

    /// Find files that depend on a given file
    #[command(alias = "deps")]
    Dependents {
        /// File path to find dependents for
        file: String,
    },

    /// Build or rebuild the search index
    Index {
        /// Path to index (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Force full reindex
        #[arg(short, long)]
        force: bool,
    },

    /// Watch for file changes and update index
    Watch {
        /// Path to watch (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Install lgrep for Claude Code
    #[command(name = "install-claude-code")]
    InstallClaudeCode,

    /// Uninstall lgrep from Claude Code
    #[command(name = "uninstall-claude-code")]
    UninstallClaudeCode,

    /// Install lgrep for Codex
    #[command(name = "install-codex")]
    InstallCodex,

    /// Uninstall lgrep from Codex
    #[command(name = "uninstall-codex")]
    UninstallCodex,

    /// Install lgrep for GitHub Copilot
    #[command(name = "install-copilot")]
    InstallCopilot,

    /// Uninstall lgrep from GitHub Copilot
    #[command(name = "uninstall-copilot")]
    UninstallCopilot,

    /// Install lgrep for OpenCode
    #[command(name = "install-opencode")]
    InstallOpencode,

    /// Uninstall lgrep from OpenCode
    #[command(name = "uninstall-opencode")]
    UninstallOpencode,
}