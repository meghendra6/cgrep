// SPDX-License-Identifier: MIT OR Apache-2.0

//! Find all callers of a function

use anyhow::Result;
use colored::Colorize;
use regex::Regex;
use serde::Serialize;

use crate::cli::{OutputFormat, UsageSearchMode};
use crate::indexer::scanner::FileScanner;
use crate::query::ast_usage::AstUsageExtractor;
use crate::query::index_filter::{find_files_with_content, read_scanned_files};
use cgrep::output::print_json;
use cgrep::utils::get_root_with_index;

/// Caller result for JSON output
#[derive(Debug, Serialize)]
struct CallerResult {
    path: String,
    line: usize,
    code: String,
}

/// Run the callers command
pub fn run(
    function: &str,
    mode: UsageSearchMode,
    format: OutputFormat,
    compact: bool,
) -> Result<()> {
    let search_root = std::env::current_dir()?.canonicalize()?;
    let index_root = get_root_with_index(&search_root);
    let files = match find_files_with_content(&index_root, function, Some(&search_root))? {
        Some(indexed_paths) => read_scanned_files(&indexed_paths),
        None => {
            let scanner = FileScanner::new(&search_root);
            scanner.scan()?
        }
    };
    let mut ast = AstUsageExtractor::new();

    // Pattern to match function calls
    // Matches: functionName( or object.functionName( or object?.functionName(
    let pattern = format!(r"\b{}\s*\(", regex::escape(function));
    let re = Regex::new(&pattern)?;

    let mut results: Vec<CallerResult> = Vec::new();

    for file in &files {
        let rel_path = file
            .path
            .strip_prefix(&search_root)
            .unwrap_or(&file.path)
            .display()
            .to_string();

        let ast_matches = if mode == UsageSearchMode::Regex {
            None
        } else {
            file.language.as_deref().and_then(|lang| {
                ast.callers(&file.content, lang, function, usize::MAX)
                    .filter(|matches| !matches.is_empty())
            })
        };

        if let Some(matches) = ast_matches {
            for m in matches {
                let code = file
                    .content
                    .lines()
                    .nth(m.line.saturating_sub(1))
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if code.is_empty() {
                    continue;
                }
                results.push(CallerResult {
                    path: rel_path.clone(),
                    line: m.line,
                    code,
                });
            }
            continue;
        }

        if mode == UsageSearchMode::Ast {
            continue;
        }

        for (line_num, line) in file.content.lines().enumerate() {
            if !re.is_match(line) {
                continue;
            }
            // Skip definition lines (function declarations)
            let line_lower = line.to_lowercase();
            if line_lower.contains("function ")
                || line_lower.contains("fn ")
                || line_lower.contains("def ")
                || line_lower.contains("func ")
            {
                continue;
            }

            results.push(CallerResult {
                path: rel_path.clone(),
                line: line_num + 1,
                code: line.trim().to_string(),
            });
        }
    }

    match format {
        OutputFormat::Json | OutputFormat::Json2 => {
            print_json(&results, compact)?;
        }
        OutputFormat::Text => {
            if results.is_empty() {
                println!("{} No callers found for: {}", "✗".red(), function.yellow());
            } else {
                println!(
                    "\n{} Finding callers of: {}\n",
                    "🔍".cyan(),
                    function.yellow()
                );
                for result in &results {
                    println!(
                        "  {}:{} {}",
                        result.path.cyan(),
                        result.line.to_string().yellow(),
                        result.code.dimmed()
                    );
                }
                println!(
                    "\n{} Found {} call sites",
                    "✓".green(),
                    results.len().to_string().cyan()
                );
            }
        }
    }

    Ok(())
}
