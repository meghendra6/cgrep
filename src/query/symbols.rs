//! Symbol search command

use anyhow::Result;
use colored::Colorize;
use serde::Serialize;

use crate::cli::OutputFormat;
use crate::indexer::scanner::FileScanner;
use crate::parser::symbols::SymbolExtractor;

/// Symbol result for JSON output
#[derive(Debug, Serialize)]
struct SymbolResult {
    name: String,
    kind: String,
    path: String,
    line: usize,
}

/// Run the symbols command
pub fn run(name: &str, symbol_type: Option<&str>, lang: Option<&str>, format: OutputFormat) -> Result<()> {
    let root = std::env::current_dir()?;
    let scanner = FileScanner::new(&root);
    let extractor = SymbolExtractor::new();

    let files = scanner.scan()?;
    let name_lower = name.to_lowercase();

    let mut results: Vec<SymbolResult> = Vec::new();

    for file in files {
        // Filter by language if specified
        if let Some(filter_lang) = lang {
            if file.language.as_deref() != Some(filter_lang) {
                continue;
            }
        }

        if let Some(ref file_lang) = file.language {
            if let Ok(symbols) = extractor.extract(&file.content, file_lang) {
                for symbol in symbols {
                    // Filter by name
                    if !symbol.name.to_lowercase().contains(&name_lower) {
                        continue;
                    }

                    // Filter by type if specified
                    if let Some(filter_type) = symbol_type {
                        if symbol.kind.to_string() != filter_type.to_lowercase() {
                            continue;
                        }
                    }

                    let rel_path = file
                        .path
                        .strip_prefix(&root)
                        .unwrap_or(&file.path)
                        .display()
                        .to_string();

                    results.push(SymbolResult {
                        name: symbol.name.clone(),
                        kind: symbol.kind.to_string(),
                        path: rel_path,
                        line: symbol.line,
                    });
                }
            }
        }
    }

    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        OutputFormat::Text => {
            if results.is_empty() {
                println!("{} No symbols found matching: {}", "✗".red(), name.yellow());
            } else {
                println!(
                    "\n{} Searching for symbol: {}\n",
                    "🔍".cyan(),
                    name.yellow()
                );
                for result in &results {
                    let kind_str = format!("[{}]", result.kind);
                    println!(
                        "  {} {} {}:{}",
                        kind_str.blue(),
                        result.name.green(),
                        result.path.cyan(),
                        result.line.to_string().yellow()
                    );
                }
                println!("\n{} Found {} symbols", "✓".green(), results.len().to_string().cyan());
            }
        }
    }

    Ok(())
}
