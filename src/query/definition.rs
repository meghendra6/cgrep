// SPDX-License-Identifier: MIT OR Apache-2.0

//! Find symbol definition location

use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::cli::OutputFormat;
use crate::indexer::scanner::{FileScanner, ScannedFile};
use crate::parser::symbols::{Symbol, SymbolExtractor, SymbolKind};
use crate::query::index_filter::{
    find_files_with_symbol, find_files_with_symbol_definition, read_scanned_files, SymbolNameMatch,
};
use cgrep::output::print_json;
use cgrep::utils::get_root_with_index;

/// Definition result for JSON output
#[derive(Debug, Serialize)]
struct DefinitionResult {
    name: String,
    kind: String,
    path: String,
    line: usize,
    column: usize,
}

/// Run the definition command
pub fn run(
    name: &str,
    path: Option<&str>,
    max_results: usize,
    format: OutputFormat,
    compact: bool,
) -> Result<()> {
    let search_root = match path {
        Some(p) => PathBuf::from(p).canonicalize()?,
        None => std::env::current_dir()?.canonicalize()?,
    };
    let index_root = get_root_with_index(&search_root);
    let extractor = SymbolExtractor::new();
    let files = load_definition_candidate_files(name, &search_root, &index_root)?;
    let content_by_path: HashMap<&std::path::PathBuf, &str> = files
        .iter()
        .map(|file| (&file.path, file.content.as_str()))
        .collect();
    let name_lower = name.to_lowercase();

    // Priority: exact match > contains
    let mut exact_matches = Vec::new();
    let mut partial_matches = Vec::new();
    let mut parser_cache = HashMap::new();

    for file in &files {
        if let Some(ref file_lang) = file.language {
            let is_cpp_like = is_cpp_like_language(file_lang);
            let lines: Vec<&str> = file.content.lines().collect();
            if let Ok(symbols) =
                extractor.extract_with_cache(&file.content, file_lang, &mut parser_cache)
            {
                let mut file_type_like_names: HashSet<String> = HashSet::new();
                for symbol in &symbols {
                    if !is_type_like_kind(&symbol.kind) {
                        continue;
                    }
                    let line_text = lines
                        .get(symbol.line.saturating_sub(1))
                        .copied()
                        .unwrap_or_default();
                    if is_forward_declaration(line_text, &symbol.kind) {
                        continue;
                    }
                    file_type_like_names.insert(symbol.name.to_lowercase());
                }

                for symbol in symbols {
                    // Skip variable/property references, focus on definitions
                    if !is_definition_kind(&symbol.kind) {
                        continue;
                    }
                    let line_text = lines
                        .get(symbol.line.saturating_sub(1))
                        .copied()
                        .unwrap_or_default();
                    if is_forward_declaration(line_text, &symbol.kind) {
                        continue;
                    }
                    if is_cpp_like && is_cpp_declaration_without_body(&symbol.kind, line_text) {
                        continue;
                    }
                    if is_cpp_like
                        && matches!(symbol.kind, SymbolKind::Function)
                        && symbol.name.to_lowercase() == name_lower
                        && file_type_like_names.contains(&name_lower)
                    {
                        // Constructor-like overloads with the same name as a type are redundant
                        // when locating a type definition and add significant token noise.
                        continue;
                    }

                    if symbol.name.to_lowercase() == name_lower {
                        exact_matches.push((file.path.clone(), symbol));
                    } else if symbol.name.to_lowercase().contains(&name_lower) {
                        partial_matches.push((file.path.clone(), symbol));
                    }
                }
            }
        }
    }

    let exact_matches = dedupe_matches(exact_matches);
    let partial_matches = dedupe_matches(partial_matches);
    let mut matches = if !exact_matches.is_empty() {
        exact_matches
    } else {
        partial_matches
    };
    sort_matches(&mut matches, &name_lower);

    let results_to_show = matches.len().min(max_results);
    let shown_matches = &matches[..results_to_show];

    // Collect results
    let results: Vec<DefinitionResult> = shown_matches
        .iter()
        .map(|(path, symbol)| {
            let rel_path = path
                .strip_prefix(&search_root)
                .unwrap_or(path)
                .display()
                .to_string();
            DefinitionResult {
                name: symbol.name.clone(),
                kind: symbol.kind.to_string(),
                path: rel_path,
                line: symbol.line,
                column: symbol.column,
            }
        })
        .collect();

    match format {
        OutputFormat::Json | OutputFormat::Json2 => {
            print_json(&results, compact)?;
        }
        OutputFormat::Text => {
            if results.is_empty() {
                println!("{} No definition found for: {}", "✗".red(), name.yellow());
                return Ok(());
            }

            println!(
                "\n{} Finding definition of: {}\n",
                "🔍".cyan(),
                name.yellow()
            );

            for (path, symbol) in shown_matches {
                let rel_path = path.strip_prefix(&search_root).unwrap_or(path).display();
                let kind_str = format!("[{}]", symbol.kind);

                println!(
                    "  {} {} {}:{}:{}",
                    kind_str.blue(),
                    symbol.name.green(),
                    rel_path.to_string().cyan(),
                    symbol.line.to_string().yellow(),
                    symbol.column.to_string().yellow()
                );

                // Show context from file
                if let Some(content) = content_by_path.get(path).copied() {
                    let lines: Vec<&str> = content.lines().collect();
                    let start = symbol.line.saturating_sub(1);
                    let end = (start + 3).min(lines.len());

                    println!();
                    for (i, line) in lines.iter().enumerate().take(end).skip(start) {
                        let line_num = format!("{:4}", i + 1);
                        let prefix = if i + 1 == symbol.line {
                            format!("{} ", "➜".green())
                        } else {
                            "  ".to_string()
                        };
                        println!("    {} {} {}", prefix, line_num.dimmed(), line);
                    }
                    println!();
                }
            }

            if matches.len() > shown_matches.len() {
                println!(
                    "{} Showing {} of {} matches (use `-m` to increase)",
                    "ℹ".cyan(),
                    shown_matches.len().to_string().cyan(),
                    matches.len().to_string().cyan()
                );
            }
        }
    }

    Ok(())
}

fn is_definition_kind(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Function
            | SymbolKind::Class
            | SymbolKind::Interface
            | SymbolKind::Type
            | SymbolKind::Struct
            | SymbolKind::Enum
            | SymbolKind::Trait
    )
}

fn is_type_like_kind(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Class
            | SymbolKind::Interface
            | SymbolKind::Type
            | SymbolKind::Struct
            | SymbolKind::Enum
            | SymbolKind::Trait
    )
}

fn is_cpp_like_language(language: &str) -> bool {
    matches!(language, "c" | "cpp")
}

fn is_forward_declaration(line_text: &str, kind: &SymbolKind) -> bool {
    if !matches!(
        kind,
        SymbolKind::Class | SymbolKind::Struct | SymbolKind::Interface | SymbolKind::Enum
    ) {
        return false;
    }
    let trimmed = line_text.trim();
    if trimmed.is_empty() || !trimmed.ends_with(';') || trimmed.contains('{') {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("class ")
        || lower.starts_with("struct ")
        || lower.starts_with("interface ")
        || lower.starts_with("enum ")
}

fn is_cpp_declaration_without_body(kind: &SymbolKind, line_text: &str) -> bool {
    if !matches!(kind, SymbolKind::Function) {
        return false;
    }
    let trimmed = line_text.trim();
    if trimmed.is_empty() || !trimmed.ends_with(';') || trimmed.contains('{') {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    !lower.starts_with("typedef ") && !lower.starts_with("using ")
}

fn dedupe_matches(matches: Vec<(PathBuf, Symbol)>) -> Vec<(PathBuf, Symbol)> {
    let mut best_by_key: HashMap<(PathBuf, String, String), Symbol> = HashMap::new();
    for (path, symbol) in matches {
        let key = (
            path.clone(),
            symbol.kind.to_string(),
            symbol.name.to_lowercase(),
        );
        best_by_key
            .entry(key)
            .and_modify(|existing| {
                if symbol.line < existing.line {
                    *existing = symbol.clone();
                }
            })
            .or_insert(symbol);
    }

    best_by_key
        .into_iter()
        .map(|((path, _, _), symbol)| (path, symbol))
        .collect()
}

fn sort_matches(matches: &mut [(PathBuf, Symbol)], name_lower: &str) {
    matches.sort_by(|(path_a, symbol_a), (path_b, symbol_b)| {
        rank_match(name_lower, path_a, symbol_a).cmp(&rank_match(name_lower, path_b, symbol_b))
    });
}

fn rank_match(name_lower: &str, path: &Path, symbol: &Symbol) -> (u8, u8, usize, String) {
    let kind_rank = match symbol.kind {
        SymbolKind::Class => 0,
        SymbolKind::Struct => 1,
        SymbolKind::Interface => 2,
        SymbolKind::Type => 3,
        SymbolKind::Trait => 4,
        SymbolKind::Enum => 5,
        SymbolKind::Function => 6,
        _ => 7,
    };
    let file_name = path
        .file_name()
        .and_then(|f| f.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let stem_name = path
        .file_stem()
        .and_then(|f| f.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let path_text = path.to_string_lossy().to_ascii_lowercase();
    let name_rank = if stem_name == name_lower {
        0
    } else if file_name.contains(name_lower) {
        1
    } else if path_text.contains(name_lower) {
        2
    } else {
        3
    };
    (kind_rank, name_rank, symbol.line, path_text)
}

fn load_definition_candidate_files(
    name: &str,
    search_root: &Path,
    index_root: &Path,
) -> Result<Vec<ScannedFile>> {
    let exact = find_files_with_symbol_definition(
        index_root,
        name,
        Some(search_root),
        SymbolNameMatch::Exact,
    )?;
    if let Some(paths) = exact {
        if !paths.is_empty() {
            return Ok(read_scanned_files(&paths));
        }

        let partial = find_files_with_symbol_definition(
            index_root,
            name,
            Some(search_root),
            SymbolNameMatch::Contains,
        )?;
        if let Some(paths) = partial {
            if !paths.is_empty() {
                return Ok(read_scanned_files(&paths));
            }
        }
    }

    match find_files_with_symbol(index_root, name, Some(search_root))? {
        Some(indexed_paths) => Ok(read_scanned_files(&indexed_paths)),
        None => {
            let scanner = FileScanner::new(search_root);
            scanner.scan()
        }
    }
}
