//! Full-text search with BM25 ranking using tantivy

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Serialize;
use tantivy::{
    collector::TopDocs,
    query::QueryParser,
    schema::Value,
    Index, TantivyDocument,
};

use crate::cli::OutputFormat;
use crate::indexer::IndexBuilder;

const INDEX_DIR: &str = ".lgrep";

/// Search result for JSON output
#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
    pub snippet: String,
}

/// Run the search command
pub fn run(query: &str, path: Option<&str>, max_results: usize, _context: usize, format: OutputFormat) -> Result<()> {
    let root = path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let index_path = root.join(INDEX_DIR);

    // Check if index exists, create if not
    if !index_path.exists() {
        println!("{} Index not found, building...", "⚠".yellow());
        let builder = IndexBuilder::new(&root)?;
        builder.build(false)?;
    }

    let index = Index::open_in_dir(&index_path).context("Failed to open index")?;

    let reader = index.reader()?;
    let searcher = reader.searcher();

    let schema = index.schema();
    let content_field = schema.get_field("content").context("Missing content field")?;
    let path_field = schema.get_field("path").context("Missing path field")?;
    let symbols_field = schema.get_field("symbols").context("Missing symbols field")?;

    // Search in both content and symbols
    let query_parser = QueryParser::for_index(&index, vec![content_field, symbols_field]);
    let parsed_query = query_parser.parse_query(query)?;

    let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(max_results))?;

    // Collect results
    let mut results: Vec<SearchResult> = Vec::new();
    for (score, doc_address) in &top_docs {
        let doc: TantivyDocument = searcher.doc(*doc_address)?;

        let path_value = doc
            .get_first(path_field)
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let content_value = doc
            .get_first(content_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let snippet = find_snippet(content_value, query, 150);

        results.push(SearchResult {
            path: path_value.to_string(),
            score: *score,
            snippet,
        });
    }

    // Output based on format
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        OutputFormat::Text => {
            if results.is_empty() {
                println!("{} No results found for: {}", "✗".red(), query.yellow());
                return Ok(());
            }

            println!(
                "\n{} Found {} results for: {}\n",
                "✓".green(),
                results.len().to_string().cyan(),
                query.yellow()
            );

            for result in &results {
                println!(
                    "{}  {} (score: {:.2})",
                    "➜".blue(),
                    result.path.cyan(),
                    result.score
                );

                if !result.snippet.is_empty() {
                    for line in result.snippet.lines().take(3) {
                        println!("    {}", line.dimmed());
                    }
                }
                println!();
            }
        }
    }

    Ok(())
}

/// Find a relevant snippet containing the query terms
fn find_snippet(content: &str, query: &str, max_len: usize) -> String {
    let query_lower = query.to_lowercase();
    let terms: Vec<&str> = query_lower.split_whitespace().collect();

    for line in content.lines() {
        let line_lower = line.to_lowercase();
        if terms.iter().any(|term| line_lower.contains(term)) {
            let trimmed = line.trim();
            if trimmed.len() <= max_len {
                return trimmed.to_string();
            } else {
                return format!("{}...", &trimmed[..max_len]);
            }
        }
    }

    // Return first non-empty line if no match
    content
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| {
            let trimmed = l.trim();
            if trimmed.len() <= max_len {
                trimmed.to_string()
            } else {
                format!("{}...", &trimmed[..max_len])
            }
        })
        .unwrap_or_default()
}
