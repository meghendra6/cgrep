// SPDX-License-Identifier: MIT OR Apache-2.0

//! Find all references to a symbol

use anyhow::Result;
use colored::Colorize;
use regex::Regex;
use serde::Serialize;
use std::path::Path;

use crate::cli::{OutputFormat, UsageSearchMode};
use crate::indexer::scanner::FileScanner;
use crate::query::ast_usage::AstUsageExtractor;
use crate::query::changed_files::ChangedFiles;
use crate::query::index_filter::{find_files_with_content, read_scanned_files};
use cgrep::output::print_json;
use cgrep::utils::get_root_with_index;

/// Reference result for JSON output
#[derive(Debug, Serialize)]
struct ReferenceResult {
    path: String,
    line: usize,
    column: usize,
    code: String,
}

/// Run the references command
pub fn run(
    name: &str,
    path: Option<&str>,
    max_results: usize,
    changed: Option<&str>,
    mode: UsageSearchMode,
    format: OutputFormat,
    compact: bool,
) -> Result<()> {
    let search_root = match path {
        Some(p) => std::path::PathBuf::from(p).canonicalize()?,
        None => std::env::current_dir()?.canonicalize()?,
    };
    let workspace_root = std::env::current_dir()?.canonicalize()?;
    let index_root = get_root_with_index(&search_root);
    let files = match find_files_with_content(&index_root, name, Some(&search_root))? {
        Some(indexed_paths) => read_scanned_files(&indexed_paths),
        None => {
            let scanner = FileScanner::new(&search_root);
            scanner.scan()?
        }
    };
    let changed_filter = changed
        .map(|rev| ChangedFiles::from_scope(&search_root, rev))
        .transpose()?;

    // Pattern to match symbol with word boundaries
    let pattern = format!(r"\b{}\b", regex::escape(name));
    let re = Regex::new(&pattern)?;

    let mut results: Vec<ReferenceResult> = Vec::new();
    let mut ast = AstUsageExtractor::new();

    for file in &files {
        let scope_path = scope_relative_path(&file.path, &search_root);
        let rel_path = workspace_display_path(&file.path, &workspace_root);
        if let Some(filter) = changed_filter.as_ref() {
            if !filter.matches_rel_path(&scope_path) {
                continue;
            }
        }

        let ast_matches = if mode == UsageSearchMode::Regex {
            None
        } else {
            file.language.as_deref().and_then(|lang| {
                ast.references(
                    &file.content,
                    lang,
                    name,
                    max_results.saturating_sub(results.len()),
                )
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
                results.push(ReferenceResult {
                    path: rel_path.clone(),
                    line: m.line,
                    column: m.column,
                    code,
                });
                if results.len() >= max_results {
                    break;
                }
            }
            if results.len() >= max_results {
                break;
            }
            continue;
        }

        if mode == UsageSearchMode::Ast {
            continue;
        }

        for (line_num, line) in file.content.lines().enumerate() {
            if let Some(mat) = re.find(line) {
                results.push(ReferenceResult {
                    path: rel_path.clone(),
                    line: line_num + 1,
                    column: mat.start() + 1,
                    code: line.trim().to_string(),
                });

                if results.len() >= max_results {
                    break;
                }
            }
        }

        if results.len() >= max_results {
            break;
        }
    }

    match format {
        OutputFormat::Json | OutputFormat::Json2 => {
            print_json(&results, compact)?;
        }
        OutputFormat::Text => {
            if results.is_empty() {
                println!("{} No references found for: {}", "✗".red(), name.yellow());
            } else {
                println!(
                    "\n{} Finding references of: {}\n",
                    "🔍".cyan(),
                    name.yellow()
                );
                for result in &results {
                    println!(
                        "  {}:{}:{} {}",
                        result.path.cyan(),
                        result.line.to_string().yellow(),
                        result.column.to_string().dimmed(),
                        result.code.dimmed()
                    );
                }
                println!(
                    "\n{} Found {} references",
                    "✓".green(),
                    results.len().to_string().cyan()
                );
            }
        }
    }

    Ok(())
}

fn scope_relative_path(full_path: &Path, search_root: &Path) -> String {
    if let Ok(rel) = full_path.strip_prefix(search_root) {
        let rendered = rel.display().to_string();
        if !rendered.is_empty() {
            return rendered;
        }
    }

    full_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| full_path.display().to_string())
}

fn workspace_display_path(full_path: &Path, workspace_root: &Path) -> String {
    if workspace_root != Path::new("/") {
        if let Ok(rel) = full_path.strip_prefix(workspace_root) {
            let rendered = rel.display().to_string();
            if !rendered.is_empty() {
                return rendered;
            }
            return ".".to_string();
        }
    }

    full_path.display().to_string()
}
