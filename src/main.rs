//! lgrep - Local semantic code search tool
//!
//! A high-performance, AST-aware search tool combining tree-sitter
//! for code structure analysis and tantivy for BM25 text ranking.

mod cli;
mod indexer;
mod install;
mod parser;
mod query;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let format = cli.format;

    match cli.command {
        Commands::Search { query, path, max_results, context } => {
            query::search::run(&query, path.as_deref(), max_results, context, format)?;
        }
        Commands::Symbols { name, symbol_type, lang } => {
            query::symbols::run(&name, symbol_type.as_deref(), lang.as_deref(), format)?;
        }
        Commands::Definition { name } => {
            query::definition::run(&name, format)?;
        }
        Commands::Callers { function } => {
            query::callers::run(&function, format)?;
        }
        Commands::Dependents { file } => {
            query::dependents::run(&file, format)?;
        }
        Commands::Index { path, force } => {
            indexer::index::run(path.as_deref(), force)?;
        }
        Commands::Watch { path } => {
            indexer::watch::run(path.as_deref())?;
        }

        // Agent installation commands
        Commands::InstallClaudeCode => {
            install::claude_code::install()?;
        }
        Commands::UninstallClaudeCode => {
            install::claude_code::uninstall()?;
        }
        Commands::InstallCodex => {
            install::codex::install()?;
        }
        Commands::UninstallCodex => {
            install::codex::uninstall()?;
        }
        Commands::InstallCopilot => {
            install::copilot::install()?;
        }
        Commands::UninstallCopilot => {
            install::copilot::uninstall()?;
        }
        Commands::InstallOpencode => {
            install::opencode::install()?;
        }
        Commands::UninstallOpencode => {
            install::opencode::uninstall()?;
        }
    }

    Ok(())
}
