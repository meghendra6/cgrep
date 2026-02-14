// SPDX-License-Identifier: MIT OR Apache-2.0

//! Index-backed helpers for narrowing file scans.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tantivy::{
    collector::DocSetCollector,
    query::{BooleanQuery, Occur, PhraseQuery, Query, RegexQuery, TermQuery},
    schema::{Field, FieldType, IndexRecordOption, Term, Value},
    Index, ReloadPolicy, TantivyDocument,
};

use crate::indexer::scanner::{detect_language, ScannedFile};
use crate::query::scope_query::{build_scope_path_query, normalize_scope, ScopeNormalization};
use cgrep::utils::INDEX_DIR;

#[derive(Clone, Copy)]
pub enum SymbolNameMatch {
    Exact,
    Contains,
}

/// Find files that likely contain a symbol name using the index.
pub fn find_files_with_symbol(
    root: &Path,
    symbol_name: &str,
    scope: Option<&Path>,
) -> Result<Option<Vec<PathBuf>>> {
    let phrase_paths =
        find_files_with_field(root, "symbols", symbol_name, scope, MatchMode::Phrase)?;
    if let Some(paths) = phrase_paths.as_ref() {
        if !paths.is_empty() {
            return Ok(phrase_paths);
        }
    }
    find_files_with_field(root, "symbols", symbol_name, scope, MatchMode::AllTokens)
}

/// Find files that likely contain a text term using the index.
pub fn find_files_with_content(
    root: &Path,
    term: &str,
    scope: Option<&Path>,
) -> Result<Option<Vec<PathBuf>>> {
    find_files_with_field(root, "content", term, scope, MatchMode::AllTokens)
}

/// Find files with symbol definition docs whose stored symbol name matches.
///
/// This only searches `doc_type=symbol` docs, which is more selective than
/// generic symbol-field filtering and helps keep `definition` lookups fast.
pub fn find_files_with_symbol_definition(
    root: &Path,
    symbol_name: &str,
    scope: Option<&Path>,
    symbol_match: SymbolNameMatch,
) -> Result<Option<Vec<PathBuf>>> {
    let index_path = root.join(INDEX_DIR);
    if !index_path.exists() {
        return Ok(None);
    }

    let index = match Index::open_in_dir(&index_path) {
        Ok(index) => index,
        Err(_) => return Ok(None),
    };

    let schema = index.schema();
    let symbols_field = match schema.get_field("symbols") {
        Ok(field) => field,
        Err(_) => return Ok(None),
    };
    let path_field = match schema.get_field("path") {
        Ok(field) => field,
        Err(_) => return Ok(None),
    };
    let doc_type_field = match schema.get_field("doc_type") {
        Ok(field) => field,
        Err(_) => return Ok(None),
    };
    let path_exact_field = schema.get_field("path_exact").ok();

    let tokens = tokenize_for_field(&index, symbols_field, symbol_name)?;
    if tokens.is_empty() {
        return Ok(Some(Vec::new()));
    }

    let effective_scope = match normalize_scope(root, scope) {
        ScopeNormalization::None => None,
        ScopeNormalization::Filter(path) => Some(path),
        ScopeNormalization::OutsideRoot => return Ok(Some(Vec::new())),
    };

    let mut clauses: Vec<(Occur, Box<dyn Query>)> = vec![
        (
            Occur::Must,
            build_symbol_name_query(symbols_field, &tokens, symbol_match),
        ),
        (
            Occur::Must,
            Box::new(TermQuery::new(
                Term::from_field_text(doc_type_field, "symbol"),
                IndexRecordOption::Basic,
            )),
        ),
    ];

    if let (Some(scope_path), Some(path_exact)) = (effective_scope.as_ref(), path_exact_field) {
        if let Some(query) = build_scope_path_query(path_exact, root, scope_path) {
            clauses.push((Occur::Must, query));
        }
    }

    let query = BooleanQuery::new(clauses);
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .context("Failed to create index reader")?;
    let searcher = reader.searcher();
    let docset = searcher.search(&query, &DocSetCollector)?;

    let needle = symbol_name.to_lowercase();
    let mut unique_paths: HashSet<PathBuf> = HashSet::with_capacity(docset.len());
    for doc_address in docset {
        let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) else {
            continue;
        };

        let stored_symbol = doc
            .get_first(symbols_field)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_lowercase();
        let matched = match symbol_match {
            SymbolNameMatch::Exact => stored_symbol == needle,
            SymbolNameMatch::Contains => stored_symbol.contains(&needle),
        };
        if !matched {
            continue;
        }

        let Some(path_value) = doc.get_first(path_field).and_then(|v| v.as_str()) else {
            continue;
        };
        let full_path = if Path::new(path_value).is_absolute() {
            PathBuf::from(path_value)
        } else {
            root.join(path_value)
        };
        if let Some(scope_path) = effective_scope.as_ref() {
            if !full_path.starts_with(scope_path) {
                continue;
            }
        }
        unique_paths.insert(full_path);
    }

    let mut paths: Vec<PathBuf> = unique_paths.into_iter().collect();
    paths.sort();
    Ok(Some(paths))
}

/// Read a list of files into scanned-file structs.
pub fn read_scanned_files(paths: &[PathBuf]) -> Vec<ScannedFile> {
    let mut scanned = Vec::with_capacity(paths.len());
    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let language = path
                .extension()
                .and_then(|e| e.to_str())
                .and_then(detect_language);
            scanned.push(ScannedFile {
                path: path.clone(),
                content,
                language,
            });
        }
    }
    scanned
}

fn find_files_with_field(
    root: &Path,
    field_name: &str,
    term: &str,
    scope: Option<&Path>,
    match_mode: MatchMode,
) -> Result<Option<Vec<PathBuf>>> {
    let index_path = root.join(INDEX_DIR);
    if !index_path.exists() {
        return Ok(None);
    }

    let index = match Index::open_in_dir(&index_path) {
        Ok(index) => index,
        Err(_) => return Ok(None),
    };

    let schema = index.schema();
    let field = match schema.get_field(field_name) {
        Ok(field) => field,
        Err(_) => return Ok(None),
    };
    let path_field = match schema.get_field("path") {
        Ok(field) => field,
        Err(_) => return Ok(None),
    };
    let path_exact_field = schema.get_field("path_exact").ok();

    let tokens = tokenize_for_field(&index, field, term)?;
    if tokens.is_empty() {
        return Ok(None);
    }

    let effective_scope = match normalize_scope(root, scope) {
        ScopeNormalization::None => None,
        ScopeNormalization::Filter(path) => Some(path),
        ScopeNormalization::OutsideRoot => return Ok(Some(Vec::new())),
    };

    let mut clauses: Vec<(Occur, Box<dyn Query>)> =
        vec![(Occur::Must, build_token_query(field, &tokens, match_mode))];

    if let (Some(scope_path), Some(path_exact)) = (effective_scope.as_ref(), path_exact_field) {
        if let Some(query) = build_scope_path_query(path_exact, root, scope_path) {
            clauses.push((Occur::Must, query));
        }
    }

    let query = BooleanQuery::new(clauses);
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .context("Failed to create index reader")?;

    let searcher = reader.searcher();
    let docset = searcher.search(&query, &DocSetCollector)?;

    let mut unique_paths: HashSet<PathBuf> = HashSet::with_capacity(docset.len());
    for doc_address in docset {
        if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_address) {
            if let Some(path_value) = doc.get_first(path_field).and_then(|v| v.as_str()) {
                let full_path = if Path::new(path_value).is_absolute() {
                    PathBuf::from(path_value)
                } else {
                    root.join(path_value)
                };
                if let Some(scope_path) = effective_scope.as_ref() {
                    if !full_path.starts_with(scope_path) {
                        continue;
                    }
                }
                unique_paths.insert(full_path);
            }
        }
    }

    let mut paths: Vec<PathBuf> = unique_paths.into_iter().collect();
    paths.sort();
    Ok(Some(paths))
}

fn build_token_query(field: Field, tokens: &[String], match_mode: MatchMode) -> Box<dyn Query> {
    match match_mode {
        MatchMode::Phrase if tokens.len() > 1 => {
            let terms: Vec<Term> = tokens
                .iter()
                .map(|token| Term::from_field_text(field, token))
                .collect();
            Box::new(PhraseQuery::new(terms))
        }
        MatchMode::Phrase | MatchMode::AllTokens => {
            let subqueries = tokens
                .iter()
                .map(|token| {
                    let term = Term::from_field_text(field, token);
                    let query = TermQuery::new(term, IndexRecordOption::Basic);
                    (Occur::Must, Box::new(query) as Box<dyn Query>)
                })
                .collect();
            Box::new(BooleanQuery::new(subqueries))
        }
    }
}

fn build_symbol_name_query(
    field: Field,
    tokens: &[String],
    symbol_match: SymbolNameMatch,
) -> Box<dyn Query> {
    match symbol_match {
        SymbolNameMatch::Exact => build_token_query(field, tokens, MatchMode::AllTokens),
        SymbolNameMatch::Contains if tokens.len() == 1 => {
            let pattern = format!(".*{}.*", regex::escape(&tokens[0]));
            if let Ok(query) = RegexQuery::from_pattern(&pattern, field) {
                Box::new(query)
            } else {
                build_token_query(field, tokens, MatchMode::AllTokens)
            }
        }
        SymbolNameMatch::Contains => build_token_query(field, tokens, MatchMode::AllTokens),
    }
}

fn tokenize_for_field(index: &Index, field: Field, text: &str) -> Result<Vec<String>> {
    let schema = index.schema();
    let field_entry = schema.get_field_entry(field);

    let tokenizer_name = match field_entry.field_type() {
        FieldType::Str(options) => options
            .get_indexing_options()
            .map(|indexing| indexing.tokenizer().to_string()),
        _ => None,
    };

    let Some(tokenizer_name) = tokenizer_name else {
        return Ok(Vec::new());
    };

    let mut analyzer = index
        .tokenizers()
        .get(&tokenizer_name)
        .ok_or_else(|| anyhow::anyhow!("Tokenizer not found: {}", tokenizer_name))?;

    let mut token_stream = analyzer.token_stream(text);
    let mut tokens = Vec::new();
    token_stream.process(&mut |token| tokens.push(token.text.to_string()));
    tokens.sort();
    tokens.dedup();
    Ok(tokens)
}

#[derive(Clone, Copy)]
enum MatchMode {
    AllTokens,
    Phrase,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::index::DEFAULT_WRITER_BUDGET_BYTES;
    use crate::indexer::IndexBuilder;
    use tempfile::TempDir;

    #[test]
    fn find_files_with_content_respects_scope() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();
        let scoped = root.join("scoped");
        std::fs::create_dir_all(&scoped).expect("mkdir");

        let outside = root.join("outside.py");
        let inside = scoped.join("inside.py");
        std::fs::write(&outside, "cpu_fallback_path(0)\n").expect("write outside");
        std::fs::write(&inside, "cpu_fallback_path(1)\n").expect("write inside");

        let builder = IndexBuilder::new(root).expect("builder");
        builder
            .build(false, DEFAULT_WRITER_BUDGET_BYTES)
            .expect("build");

        let paths = find_files_with_content(root, "cpu_fallback_path", Some(&scoped))
            .expect("query")
            .expect("index-backed");

        assert!(paths.iter().any(|p| p.ends_with("inside.py")));
        assert!(!paths.iter().any(|p| p.ends_with("outside.py")));
    }

    #[test]
    fn find_symbol_definition_exact_excludes_partial_names() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();

        let run_file = root.join("run.rs");
        let runner_file = root.join("runner.rs");
        std::fs::write(&run_file, "pub fn run() {}\n").expect("write run");
        std::fs::write(&runner_file, "pub fn runner() {}\n").expect("write runner");

        let builder = IndexBuilder::new(root).expect("builder");
        builder
            .build(false, DEFAULT_WRITER_BUDGET_BYTES)
            .expect("build");

        let paths =
            find_files_with_symbol_definition(root, "run", Some(root), SymbolNameMatch::Exact)
                .expect("query")
                .expect("index-backed");

        assert!(paths.iter().any(|p| p.ends_with("run.rs")));
        assert!(!paths.iter().any(|p| p.ends_with("runner.rs")));
    }

    #[test]
    fn find_symbol_definition_contains_includes_partial_names() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();

        let run_file = root.join("run.rs");
        let runner_file = root.join("runner.rs");
        std::fs::write(&run_file, "pub fn run() {}\n").expect("write run");
        std::fs::write(&runner_file, "pub fn runner() {}\n").expect("write runner");

        let builder = IndexBuilder::new(root).expect("builder");
        builder
            .build(false, DEFAULT_WRITER_BUDGET_BYTES)
            .expect("build");

        let paths =
            find_files_with_symbol_definition(root, "run", Some(root), SymbolNameMatch::Contains)
                .expect("query")
                .expect("index-backed");

        assert!(paths.iter().any(|p| p.ends_with("run.rs")));
        assert!(paths.iter().any(|p| p.ends_with("runner.rs")));
    }
}
