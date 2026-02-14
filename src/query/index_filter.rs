// SPDX-License-Identifier: MIT OR Apache-2.0

//! Index-backed helpers for narrowing file scans.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::MAIN_SEPARATOR;
use std::path::{Path, PathBuf};
use tantivy::{
    collector::DocSetCollector,
    query::{BooleanQuery, Occur, PhraseQuery, Query, RegexQuery, TermQuery},
    schema::{Field, FieldType, IndexRecordOption, Term, Value},
    Index, ReloadPolicy, TantivyDocument,
};

use crate::indexer::scanner::{detect_language, ScannedFile};
use cgrep::utils::INDEX_DIR;

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

    let effective_scope = match normalize_scope(root, scope)? {
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

enum ScopeNormalization {
    None,
    Filter(PathBuf),
    OutsideRoot,
}

#[derive(Clone, Copy)]
enum MatchMode {
    AllTokens,
    Phrase,
}

fn normalize_scope(root: &Path, scope: Option<&Path>) -> Result<ScopeNormalization> {
    let Some(scope) = scope else {
        return Ok(ScopeNormalization::None);
    };

    let root = root.to_path_buf();
    let scope = if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        root.join(scope)
    };

    if scope == root {
        return Ok(ScopeNormalization::None);
    }
    if scope.starts_with(&root) {
        return Ok(ScopeNormalization::Filter(scope));
    }

    // Canonical fallback covers symlink aliases (/var vs /private/var on macOS).
    let root_canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
    let scope_canonical = scope.canonicalize().unwrap_or_else(|_| scope.clone());
    if scope_canonical == root_canonical {
        return Ok(ScopeNormalization::None);
    }
    if scope_canonical.starts_with(&root_canonical) {
        return Ok(ScopeNormalization::Filter(scope));
    }

    Ok(ScopeNormalization::OutsideRoot)
}

fn build_scope_path_query(
    path_exact_field: Field,
    root: &Path,
    scope_path: &Path,
) -> Option<Box<dyn Query>> {
    let mut scope_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
    let mut seen_terms: HashSet<String> = HashSet::new();
    let mut seen_patterns: HashSet<String> = HashSet::new();

    let mut root_variants = vec![root.to_path_buf()];
    if let Ok(canonical) = root.canonicalize() {
        if !root_variants.iter().any(|v| v == &canonical) {
            root_variants.push(canonical);
        }
    }
    let mut scope_variants = vec![scope_path.to_path_buf()];
    if let Ok(canonical) = scope_path.canonicalize() {
        if !scope_variants.iter().any(|v| v == &canonical) {
            scope_variants.push(canonical);
        }
    }

    for scope_variant in &scope_variants {
        if scope_variant.is_file() {
            let value = scope_variant.to_string_lossy().to_string();
            if seen_terms.insert(value.clone()) {
                let term = Term::from_field_text(path_exact_field, &value);
                scope_queries.push((
                    Occur::Should,
                    Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
                ));
            }
        } else {
            let mut absolute_prefix = scope_variant.to_string_lossy().to_string();
            if !absolute_prefix.ends_with(MAIN_SEPARATOR) {
                absolute_prefix.push(MAIN_SEPARATOR);
            }
            let pattern = format!("{}.*", regex::escape(&absolute_prefix));
            if seen_patterns.insert(pattern.clone()) {
                if let Ok(query) = RegexQuery::from_pattern(&pattern, path_exact_field) {
                    scope_queries.push((Occur::Should, Box::new(query)));
                }
            }
        }
        for root_variant in &root_variants {
            let rel_scope = scope_variant
                .strip_prefix(root_variant)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if rel_scope.is_empty() {
                continue;
            }

            if scope_variant.is_file() {
                for candidate in [rel_scope.clone(), format!("./{rel_scope}")] {
                    if seen_terms.insert(candidate.clone()) {
                        let term = Term::from_field_text(path_exact_field, &candidate);
                        scope_queries.push((
                            Occur::Should,
                            Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
                        ));
                    }
                }
            } else {
                let mut rel_prefix = rel_scope.clone();
                if !rel_prefix.ends_with('/') {
                    rel_prefix.push('/');
                }
                for prefix in [rel_prefix.clone(), format!("./{rel_prefix}")] {
                    let pattern = format!("{}.*", regex::escape(&prefix));
                    if seen_patterns.insert(pattern.clone()) {
                        if let Ok(query) = RegexQuery::from_pattern(&pattern, path_exact_field) {
                            scope_queries.push((Occur::Should, Box::new(query)));
                        }
                    }
                }
            }
        }
    }

    if scope_queries.is_empty() {
        None
    } else if scope_queries.len() == 1 {
        Some(scope_queries.remove(0).1)
    } else {
        Some(Box::new(BooleanQuery::new(scope_queries)))
    }
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
}
