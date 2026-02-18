// SPDX-License-Identifier: MIT OR Apache-2.0

//! CLI argument parsing using clap

use clap::{Parser, Subcommand};
use clap_complete::Shell;

/// cgrep - Local semantic code search tool
///
/// A high-performance search tool combining AST analysis with BM25 ranking.
/// Supports symbol search, dependency tracking, and full-text search.
#[derive(Parser, Debug)]
#[command(name = "cgrep")]
#[command(
    author,
    version,
    about,
    long_about = None,
    override_usage = "cgrep [OPTIONS] <COMMAND>",
    after_help = "Search quickstart:\n  cgrep s \"token refresh\" src/\n  cgrep search -r --include '**/*.rs' needle src/\n\nLiteral query tips:\n  cgrep search -- --literal\n  cgrep s read"
)]
pub struct Cli {
    /// Output format (text or json)
    #[arg(long, global = true)]
    pub format: Option<OutputFormat>,

    /// Compact JSON output (no pretty formatting)
    #[arg(long, global = true)]
    pub compact: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Output format for results
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
    /// Structured JSON for AI agents (`meta` + `results`)
    Json2,
}

/// Search mode for queries
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum CliSearchMode {
    /// BM25 keyword search only
    #[default]
    Keyword,
    /// Embedding-based semantic search only
    Semantic,
    /// Combined BM25 + embedding search
    Hybrid,
}

/// Output budget preset for token-efficient responses
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CliBudgetPreset {
    Tight,
    Balanced,
    Full,
    Off,
}

/// Usage lookup strategy for callers/references
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum UsageSearchMode {
    /// Prefer AST when language parser is available, otherwise regex fallback
    Auto,
    /// Regex-only text matching
    Regex,
    /// AST-only matching for supported languages
    Ast,
}

/// Agent provider for install/uninstall commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum AgentProvider {
    ClaudeCode,
    Codex,
    Copilot,
    Cursor,
    Opencode,
}

#[derive(Subcommand, Debug)]
pub enum AgentCommands {
    /// Stage 1: locate candidate code regions with minimal payload
    #[command(visible_aliases = ["l", "loc"])]
    Locate {
        /// Search query (natural language or keywords)
        query: String,

        /// Path to search in (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Limit search to files changed since revision (default: HEAD)
        #[arg(short = 'u', long, num_args = 0..=1, default_missing_value = "HEAD")]
        changed: Option<String>,

        /// Maximum number of results to return
        #[arg(short = 'm', long = "limit")]
        limit: Option<usize>,

        /// Search mode: keyword, semantic, or hybrid
        #[arg(short = 'M', long, value_enum)]
        mode: Option<CliSearchMode>,

        /// Output budget preset (default: balanced)
        #[arg(short = 'B', long, value_enum)]
        budget: Option<CliBudgetPreset>,
    },

    /// Stage 2: expand selected locate result IDs into richer context
    #[command(visible_aliases = ["x", "ex"])]
    Expand {
        /// Result ID from `agent locate` (repeatable)
        #[arg(long = "id", required = true)]
        ids: Vec<String>,

        /// Path to search in (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Context lines to return around each ID match
        #[arg(short = 'C', long)]
        context: Option<usize>,
    },

    /// Install cgrep instructions for an AI agent provider
    #[command(visible_aliases = ["add"])]
    Install {
        #[arg(value_enum)]
        provider: AgentProvider,
    },

    /// Uninstall cgrep instructions for an AI agent provider
    #[command(visible_aliases = ["rm"])]
    Uninstall {
        #[arg(value_enum)]
        provider: AgentProvider,
    },
}

#[derive(Subcommand, Debug)]
pub enum DaemonCommands {
    /// Start background watch daemon
    #[command(visible_aliases = ["up"])]
    Start {
        /// Path to watch (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Debounce interval in seconds (default: 15)
        #[arg(short = 'd', long, default_value = "15")]
        debounce: u64,

        /// Minimum time between reindex operations in seconds (default: 180)
        #[arg(short = 'i', long = "min-interval", default_value = "180")]
        min_interval: u64,

        /// Force reindex if events keep arriving for this many seconds (default: 180)
        #[arg(short = 'b', long = "max-batch-delay", default_value = "180")]
        max_batch_delay: u64,

        /// Disable adaptive backoff (adaptive is on by default)
        #[arg(long = "no-adaptive")]
        no_adaptive: bool,
    },

    /// Stop background watch daemon
    #[command(visible_aliases = ["down"])]
    Stop {
        /// Path containing daemon state (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Print daemon status
    #[command(visible_aliases = ["st"])]
    Status {
        /// Path containing daemon state (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
}

/// MCP host target for automatic config install
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum McpHost {
    ClaudeCode,
    Cursor,
    Windsurf,
    Vscode,
    ClaudeDesktop,
}

#[derive(Subcommand, Debug)]
pub enum McpCommands {
    /// Run cgrep as an MCP stdio server
    #[command(visible_aliases = ["run"])]
    Serve,

    /// Install cgrep MCP server config for a host
    #[command(visible_aliases = ["add"])]
    Install {
        #[arg(value_enum)]
        host: McpHost,
    },

    /// Remove cgrep MCP server config from a host
    #[command(visible_aliases = ["rm"])]
    Uninstall {
        #[arg(value_enum)]
        host: McpHost,
    },
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Full-text search with BM25 ranking
    #[command(
        visible_aliases = ["s", "find", "q"],
        after_help = "Examples:\n  cgrep s \"token refresh\" src/\n  cgrep search -r --no-ignore \"auth flow\" src/\n  cgrep search \"retry\" -p src/ -C 2"
    )]
    Search {
        /// Search query (natural language or keywords)
        #[arg(required_unless_present = "help_advanced")]
        query: Option<String>,

        /// Optional path (grep-style positional form)
        #[arg(value_name = "PATH")]
        path_positional: Option<String>,

        /// Path to search in (defaults to current directory)
        #[arg(short, long, help_heading = "Core")]
        path: Option<String>,

        /// Search subdirectories recursively (grep -r, default)
        #[arg(short = 'r', long, help_heading = "Scope")]
        recursive: bool,

        /// Search only the top-level directory in the scope
        #[arg(long, conflicts_with = "recursive", help_heading = "Scope")]
        no_recursive: bool,

        /// Do not respect .gitignore/.ignore rules (forces scan mode)
        #[arg(long, help_heading = "Scope")]
        no_ignore: bool,

        /// Maximum number of results
        #[arg(
            short = 'm',
            long = "limit",
            visible_alias = "max-results",
            help_heading = "Core"
        )]
        limit: Option<usize>,

        /// Show N lines before and after each match (like grep -C)
        #[arg(short = 'C', long, help_heading = "Core")]
        context: Option<usize>,

        /// Filter by file type/language (e.g., rust, ts, python)
        #[arg(short = 't', long = "type", help_heading = "Core")]
        file_type: Option<String>,

        /// Filter files matching glob pattern (e.g., "*.rs", "src/**/*.ts")
        #[arg(short = 'g', long, visible_alias = "include", help_heading = "Core")]
        glob: Option<String>,

        /// Exclude files matching pattern
        #[arg(
            short = 'x',
            long,
            visible_alias = "exclude-dir",
            help_heading = "Core"
        )]
        exclude: Option<String>,

        /// Limit search to files changed since revision (default: HEAD)
        #[arg(
            short = 'u',
            long,
            num_args = 0..=1,
            default_missing_value = "HEAD",
            help_heading = "Core"
        )]
        changed: Option<String>,

        /// Output budget preset (tight, balanced, full, off)
        #[arg(short = 'B', long, value_enum, help_heading = "Core")]
        budget: Option<CliBudgetPreset>,

        /// Use a preset profile (human, agent, fast)
        #[arg(short = 'P', long, help_heading = "Core")]
        profile: Option<String>,

        /// Suppress statistics output
        #[arg(short = 'q', long, help_heading = "Core")]
        quiet: bool,

        /// Treat query as a regular expression (scan mode)
        #[arg(long, help_heading = "Mode")]
        regex: bool,

        /// Grep compatibility flag (ignore case, default behavior)
        #[arg(
            short = 'i',
            long = "ignore-case",
            conflicts_with = "case_sensitive",
            help_heading = "Mode"
        )]
        ignore_case: bool,

        /// Case-sensitive search (scan mode)
        #[arg(long, conflicts_with = "ignore_case", help_heading = "Mode")]
        case_sensitive: bool,

        /// Search mode: keyword, semantic, or hybrid
        #[arg(short = 'M', long, value_enum, help_heading = "Mode")]
        mode: Option<CliSearchMode>,

        /// Deprecated: use `--mode keyword`
        #[arg(
            long,
            hide = true,
            conflicts_with = "semantic",
            conflicts_with = "hybrid"
        )]
        keyword: bool,

        /// Deprecated: use `--mode semantic`
        #[arg(
            long,
            hide = true,
            conflicts_with = "keyword",
            conflicts_with = "hybrid"
        )]
        semantic: bool,

        /// Deprecated: use `--mode hybrid`
        #[arg(
            long,
            hide = true,
            conflicts_with = "keyword",
            conflicts_with = "semantic"
        )]
        hybrid: bool,

        /// Print advanced options for search and exit
        #[arg(long, help_heading = "Help")]
        help_advanced: bool,

        /// Context pack size for agent mode (merges overlapping context)
        #[arg(long, hide = true)]
        context_pack: Option<usize>,

        /// Enable agent session caching
        #[arg(long, hide = true)]
        agent_cache: bool,

        /// Cache TTL in milliseconds (default: 600000 = 10 minutes)
        #[arg(long, hide = true)]
        cache_ttl: Option<u64>,

        /// Maximum characters per snippet in output
        #[arg(long, hide = true)]
        max_chars_per_snippet: Option<usize>,

        /// Maximum total characters across returned results
        #[arg(long, hide = true)]
        max_total_chars: Option<usize>,

        /// Maximum context characters per result (before+after)
        #[arg(long, hide = true)]
        max_context_chars: Option<usize>,

        /// Remove duplicated context lines across results
        #[arg(long, hide = true)]
        dedupe_context: bool,

        /// Use short path aliases (p1, p2, ...) in json2 output with lookup table in meta
        #[arg(long, hide = true)]
        path_alias: bool,

        /// Suppress repeated boilerplate lines (imports/headers) in snippets and context
        #[arg(long, hide = true)]
        suppress_boilerplate: bool,

        /// Enable fuzzy matching (allows 1-2 character differences)
        #[arg(short = 'f', long, hide = true)]
        fuzzy: bool,

        /// Do not use the index; scan files directly
        #[arg(long, hide = true)]
        no_index: bool,

        /// Internal flag for metadata when MCP bootstrapped an index before search
        #[arg(long, hide = true)]
        bootstrap_index: bool,
    },

    /// Read a file with smart full/outline output
    #[command(visible_aliases = ["rd", "cat", "view"])]
    Read {
        /// File path to read
        path: String,

        /// Read only a specific section (line range `start-end` or markdown heading)
        #[arg(short = 's', long)]
        section: Option<String>,

        /// Force full content output (disable smart outline mode)
        #[arg(long)]
        full: bool,
    },

    /// Print a structural codebase map
    #[command(visible_aliases = ["mp", "tree"])]
    Map {
        /// Root path to map (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Maximum directory depth (default: 3)
        #[arg(short = 'd', long, default_value = "3")]
        depth: usize,
    },

    /// Agent-optimized workflow: locate/expand/install/uninstall
    #[command(visible_aliases = ["a"])]
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },

    /// Manage background watch daemon
    #[command(visible_aliases = ["bg"])]
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },

    /// MCP server and host config integration
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },

    /// Search for symbols (functions, classes, etc.)
    #[command(visible_aliases = ["sym", "sy"])]
    Symbols {
        /// Symbol name to search for
        name: String,

        /// Filter by symbol type (function, class, variable, etc.)
        #[arg(short = 'T', long = "type")]
        symbol_type: Option<String>,

        /// Filter by language (typescript, python, rust, etc.)
        #[arg(short, long)]
        lang: Option<String>,

        /// Filter by file type/language (e.g., rust, ts, python)
        #[arg(short = 't', long = "file-type")]
        file_type: Option<String>,

        /// Filter files matching glob pattern (e.g., "*.rs", "src/**/*.ts")
        #[arg(short = 'g', long, visible_alias = "include")]
        glob: Option<String>,

        /// Exclude files matching pattern
        #[arg(short = 'x', long, visible_alias = "exclude-dir")]
        exclude: Option<String>,

        /// Limit symbol search to files changed since revision (default: HEAD)
        #[arg(short = 'u', long, num_args = 0..=1, default_missing_value = "HEAD")]
        changed: Option<String>,

        /// Suppress statistics output
        #[arg(short = 'q', long)]
        quiet: bool,
    },

    /// Find symbol definition location
    #[command(visible_aliases = ["def", "d"])]
    Definition {
        /// Symbol name to find definition for
        name: String,

        /// Path to search in (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Maximum number of results
        #[arg(
            short = 'm',
            long = "limit",
            visible_alias = "max-results",
            default_value = "20"
        )]
        max_results: usize,
    },

    /// Find all callers of a function
    #[command(visible_aliases = ["calls", "c"])]
    Callers {
        /// Function name to find callers for
        function: String,

        /// Matching strategy (auto, regex, ast)
        #[arg(short = 'M', long, value_enum, default_value = "auto")]
        mode: UsageSearchMode,
    },

    /// Find all references to a symbol
    #[command(visible_aliases = ["refs", "r"])]
    References {
        /// Symbol name to find references for
        name: String,

        /// Path to search in (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Maximum number of results
        #[arg(
            short = 'm',
            long = "limit",
            visible_alias = "max-results",
            default_value = "50"
        )]
        max_results: usize,

        /// Limit references to files changed since revision (default: HEAD)
        #[arg(short = 'u', long, num_args = 0..=1, default_missing_value = "HEAD")]
        changed: Option<String>,

        /// Matching strategy (auto, regex, ast)
        #[arg(short = 'M', long, value_enum, default_value = "auto")]
        mode: UsageSearchMode,
    },

    /// Find files that depend on a given file
    #[command(visible_aliases = ["deps", "dep"])]
    Dependents {
        /// File path to find dependents for
        file: String,
    },

    /// Build or rebuild the search index
    #[command(visible_aliases = ["ix", "i"])]
    Index {
        /// Path to index (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Force full reindex
        #[arg(short, long)]
        force: bool,

        /// Embedding generation mode: auto, precompute, or off
        #[arg(short = 'E', long, default_value = "off")]
        embeddings: String,

        /// Force regeneration of all embeddings
        #[arg(short = 'F', long)]
        embeddings_force: bool,

        /// Use a high-memory index writer (1GiB budget)
        #[arg(short = 'H', long)]
        high_memory: bool,

        /// Include files ignored by .gitignore/.ignore (opt-out of default ignore-respecting index)
        #[arg(long)]
        include_ignored: bool,

        /// Paths/patterns to exclude (can be specified multiple times)
        #[arg(long = "exclude", short = 'e')]
        exclude_paths: Vec<String>,
    },

    /// Watch for file changes and update index
    #[command(visible_aliases = ["wt", "w"])]
    Watch {
        /// Path to watch (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Debounce interval in seconds (default: 15)
        #[arg(short = 'd', long, default_value = "15")]
        debounce: u64,

        /// Minimum time between reindex operations in seconds (default: 180)
        #[arg(short = 'i', long = "min-interval", default_value = "180")]
        min_interval: u64,

        /// Force reindex if events keep arriving for this many seconds (default: 180)
        #[arg(short = 'b', long = "max-batch-delay", default_value = "180")]
        max_batch_delay: u64,

        /// Disable adaptive backoff (adaptive is on by default)
        #[arg(long = "no-adaptive")]
        no_adaptive: bool,
    },

    /// Install cgrep for Claude Code
    #[command(name = "install-claude-code", hide = true)]
    InstallClaudeCode,

    /// Uninstall cgrep from Claude Code
    #[command(name = "uninstall-claude-code", hide = true)]
    UninstallClaudeCode,

    /// Install cgrep for Codex
    #[command(name = "install-codex", hide = true)]
    InstallCodex,

    /// Uninstall cgrep from Codex
    #[command(name = "uninstall-codex", hide = true)]
    UninstallCodex,

    /// Install cgrep for GitHub Copilot
    #[command(name = "install-copilot", hide = true)]
    InstallCopilot,

    /// Uninstall cgrep from GitHub Copilot
    #[command(name = "uninstall-copilot", hide = true)]
    UninstallCopilot,

    /// Install cgrep for OpenCode
    #[command(name = "install-opencode", hide = true)]
    InstallOpencode,

    /// Uninstall cgrep from OpenCode
    #[command(name = "uninstall-opencode", hide = true)]
    UninstallOpencode,

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn search_alias_and_short_flags_parse() {
        let cli = Cli::try_parse_from([
            "cgrep",
            "s",
            "auth flow",
            "-M",
            "keyword",
            "-B",
            "tight",
            "-P",
            "agent",
            "-x",
            "target/**",
            "-u",
        ])
        .expect("parse search alias");

        match cli.command {
            Commands::Search {
                query,
                mode,
                budget,
                profile,
                exclude,
                changed,
                ..
            } => {
                assert_eq!(query.as_deref(), Some("auth flow"));
                assert_eq!(mode, Some(CliSearchMode::Keyword));
                assert_eq!(budget, Some(CliBudgetPreset::Tight));
                assert_eq!(profile.as_deref(), Some("agent"));
                assert_eq!(exclude.as_deref(), Some("target/**"));
                assert_eq!(changed.as_deref(), Some("HEAD"));
            }
            other => panic!("expected search command, got {other:?}"),
        }
    }

    #[test]
    fn search_with_positional_path_parses() {
        let cli = Cli::try_parse_from(["cgrep", "search", "auth flow", "src"])
            .expect("parse search with positional path");

        match cli.command {
            Commands::Search {
                query,
                path_positional,
                ..
            } => {
                assert_eq!(query.as_deref(), Some("auth flow"));
                assert_eq!(path_positional.as_deref(), Some("src"));
            }
            other => panic!("expected search command, got {other:?}"),
        }
    }

    #[test]
    fn search_scope_flags_parse() {
        let cli = Cli::try_parse_from(["cgrep", "search", "-r", "--no-ignore", "needle", "src"])
            .expect("parse search scope flags");

        match cli.command {
            Commands::Search {
                query,
                path_positional,
                recursive,
                no_ignore,
                ..
            } => {
                assert_eq!(query.as_deref(), Some("needle"));
                assert_eq!(path_positional.as_deref(), Some("src"));
                assert!(recursive);
                assert!(no_ignore);
            }
            other => panic!("expected search command, got {other:?}"),
        }
    }

    #[test]
    fn definition_short_alias_parses() {
        let cli = Cli::try_parse_from(["cgrep", "d", "handle_auth", "-p", "src", "-m", "7"])
            .expect("parse definition");
        match cli.command {
            Commands::Definition {
                name,
                path,
                max_results,
            } => {
                assert_eq!(name, "handle_auth");
                assert_eq!(path.as_deref(), Some("src"));
                assert_eq!(max_results, 7);
            }
            other => panic!("expected definition command, got {other:?}"),
        }
    }

    #[test]
    fn agent_alias_and_short_flags_parse() {
        let cli = Cli::try_parse_from([
            "cgrep",
            "a",
            "l",
            "token validation",
            "-u",
            "-M",
            "keyword",
            "-B",
            "balanced",
        ])
        .expect("parse agent locate alias");

        match cli.command {
            Commands::Agent {
                command:
                    AgentCommands::Locate {
                        query,
                        changed,
                        mode,
                        budget,
                        ..
                    },
            } => {
                assert_eq!(query, "token validation");
                assert_eq!(changed.as_deref(), Some("HEAD"));
                assert_eq!(mode, Some(CliSearchMode::Keyword));
                assert_eq!(budget, Some(CliBudgetPreset::Balanced));
            }
            other => panic!("expected agent locate command, got {other:?}"),
        }
    }

    #[test]
    fn references_short_alias_and_mode_parse() {
        let cli = Cli::try_parse_from(["cgrep", "r", "UserService", "-M", "ast"])
            .expect("parse references alias");

        match cli.command {
            Commands::References { name, mode, .. } => {
                assert_eq!(name, "UserService");
                assert_eq!(mode, UsageSearchMode::Ast);
            }
            other => panic!("expected references command, got {other:?}"),
        }
    }
}
