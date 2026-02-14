// SPDX-License-Identifier: MIT OR Apache-2.0

//! Codebase structure map command.

use anyhow::{bail, Context, Result};
use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::OutputFormat;
use crate::indexer::scanner::detect_language;
use crate::parser::symbols::SymbolExtractor;
use cgrep::output::print_json;

const MAX_SYMBOLS_PER_FILE: usize = 6;
const MAX_SYMBOL_FILE_SIZE: u64 = 500_000;

#[derive(Debug, Clone)]
struct MapEntryData {
    rel_path: PathBuf,
    tokens_estimate: u64,
    symbols: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MapEntry {
    path: String,
    tokens_estimate: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    symbols: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MapPayload<'a> {
    root: &'a str,
    depth: usize,
    entries: Vec<MapEntry>,
}

#[derive(Debug, Serialize)]
struct MapJson2Meta<'a> {
    schema_version: &'static str,
    command: &'static str,
    root: &'a str,
    depth: usize,
}

#[derive(Debug, Serialize)]
struct MapJson2Payload<'a> {
    meta: MapJson2Meta<'a>,
    entries: Vec<MapEntry>,
}

/// Run the map command.
pub fn run(path: Option<&str>, depth: usize, format: OutputFormat, compact: bool) -> Result<()> {
    let cwd = std::env::current_dir().context("Cannot determine current directory")?;
    let root = resolve_root(&cwd, path);
    if !root.exists() {
        bail!("Path not found: {}", root.display());
    }
    if !root.is_dir() {
        bail!("Map requires a directory path: {}", root.display());
    }

    let entries = collect_entries(&root, depth)?;
    let root_display = display_root(&cwd, &root);

    match format {
        OutputFormat::Text => {
            let rendered = render_text_map(&root_display, depth, &entries);
            println!("{rendered}");
        }
        OutputFormat::Json => {
            let payload = MapPayload {
                root: &root_display,
                depth,
                entries: to_json_entries(&entries),
            };
            print_json(&payload, compact)?;
        }
        OutputFormat::Json2 => {
            let payload = MapJson2Payload {
                meta: MapJson2Meta {
                    schema_version: "1",
                    command: "map",
                    root: &root_display,
                    depth,
                },
                entries: to_json_entries(&entries),
            };
            print_json(&payload, compact)?;
        }
    }

    Ok(())
}

fn resolve_root(cwd: &Path, raw_path: Option<&str>) -> PathBuf {
    let raw = raw_path.unwrap_or(".");
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn display_root(cwd: &Path, root: &Path) -> String {
    root.strip_prefix(cwd).unwrap_or(root).display().to_string()
}

fn to_json_entries(entries: &[MapEntryData]) -> Vec<MapEntry> {
    entries
        .iter()
        .map(|entry| MapEntry {
            path: entry.rel_path.display().to_string(),
            tokens_estimate: entry.tokens_estimate,
            symbols: entry.symbols.clone(),
        })
        .collect()
}

fn collect_entries(root: &Path, depth: usize) -> Result<Vec<MapEntryData>> {
    let mut entries = Vec::new();
    let extractor = SymbolExtractor::new();

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_str().unwrap_or("");
            name != ".git" && name != ".cgrep" && name != "node_modules" && name != "target"
        })
        .max_depth(Some(depth + 1))
        .build();

    for entry in walker {
        let Ok(entry) = entry else {
            continue;
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        let file_depth = rel.components().count().saturating_sub(1);
        if file_depth > depth {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let size = metadata.len();
        let tokens = estimate_tokens(size);
        let symbols = collect_symbols(path, size, &extractor);

        entries.push(MapEntryData {
            rel_path: rel,
            tokens_estimate: tokens,
            symbols,
        });
    }

    entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(entries)
}

fn collect_symbols(path: &Path, size: u64, extractor: &SymbolExtractor) -> Vec<String> {
    if size > MAX_SYMBOL_FILE_SIZE {
        return Vec::new();
    }

    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    let Some(language) = detect_language(extension) else {
        return Vec::new();
    };

    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };
    let symbols = match extractor.extract(&content, &language) {
        Ok(symbols) => symbols,
        Err(_) => return Vec::new(),
    };

    let mut unique = HashSet::new();
    let mut names = Vec::new();
    for symbol in symbols {
        if !unique.insert(symbol.name.clone()) {
            continue;
        }
        names.push(symbol.name);
        if names.len() >= MAX_SYMBOLS_PER_FILE {
            break;
        }
    }
    names
}

fn render_text_map(root_display: &str, depth: usize, entries: &[MapEntryData]) -> String {
    let mut by_dir: BTreeMap<PathBuf, Vec<&MapEntryData>> = BTreeMap::new();
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    dirs.insert(PathBuf::new());

    for entry in entries {
        let parent = entry
            .rel_path
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        by_dir.entry(parent.clone()).or_default().push(entry);

        let mut cur = parent;
        loop {
            dirs.insert(cur.clone());
            let Some(next) = cur.parent() else {
                break;
            };
            cur = next.to_path_buf();
            if cur.as_os_str().is_empty() {
                dirs.insert(PathBuf::new());
                break;
            }
        }
    }

    let mut out = String::new();
    out.push_str(&format!("# Map: {} (depth {})\n", root_display, depth));
    format_directory(&mut out, &PathBuf::new(), 0, &by_dir, &dirs);
    out
}

fn format_directory(
    out: &mut String,
    dir: &PathBuf,
    indent: usize,
    by_dir: &BTreeMap<PathBuf, Vec<&MapEntryData>>,
    dirs: &BTreeSet<PathBuf>,
) {
    if let Some(files) = by_dir.get(dir) {
        for entry in files {
            let Some(name) = entry.rel_path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if entry.symbols.is_empty() {
                out.push_str(&format!(
                    "{}{} (~{} tokens)\n",
                    "  ".repeat(indent),
                    name,
                    entry.tokens_estimate
                ));
            } else {
                out.push_str(&format!(
                    "{}{}: {}\n",
                    "  ".repeat(indent),
                    name,
                    entry.symbols.join(", ")
                ));
            }
        }
    }

    let mut subdirs: Vec<PathBuf> = dirs
        .iter()
        .filter(|candidate| {
            if **candidate == *dir || candidate.as_os_str().is_empty() {
                return false;
            }
            candidate.parent().unwrap_or(Path::new("")) == dir.as_path()
        })
        .cloned()
        .collect();
    subdirs.sort();

    for subdir in subdirs {
        let Some(name) = subdir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        out.push_str(&format!("{}{}{}\n", "  ".repeat(indent), name, "/"));
        format_directory(out, &subdir, indent + 1, by_dir, dirs);
    }
}

fn estimate_tokens(bytes: u64) -> u64 {
    bytes.div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_rounds_up() {
        assert_eq!(estimate_tokens(0), 0);
        assert_eq!(estimate_tokens(1), 1);
        assert_eq!(estimate_tokens(4), 1);
        assert_eq!(estimate_tokens(5), 2);
    }
}
