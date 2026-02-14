// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared helpers for scoped `path_exact` index queries.

use std::collections::HashSet;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use tantivy::query::{BooleanQuery, Occur, Query, RegexQuery, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, Term};

#[derive(Debug, Clone)]
pub(crate) enum ScopeNormalization {
    None,
    Filter(PathBuf),
    OutsideRoot,
}

/// Normalize a user-provided scope against index root.
pub(crate) fn normalize_scope(root: &Path, scope: Option<&Path>) -> ScopeNormalization {
    let Some(scope) = scope else {
        return ScopeNormalization::None;
    };

    let root = root.to_path_buf();
    let scope = if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        root.join(scope)
    };

    if scope == root {
        return ScopeNormalization::None;
    }
    if scope.starts_with(&root) {
        return ScopeNormalization::Filter(scope);
    }

    // Canonical fallback covers symlink aliases (/var vs /private/var on macOS).
    let root_canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
    let scope_canonical = scope.canonicalize().unwrap_or_else(|_| scope.clone());
    if scope_canonical == root_canonical {
        return ScopeNormalization::None;
    }
    if scope_canonical.starts_with(&root_canonical) {
        return ScopeNormalization::Filter(scope);
    }

    ScopeNormalization::OutsideRoot
}

/// Build an OR query that matches all equivalent path encodings for a scope.
/// Handles absolute paths + relative paths + canonical aliases.
pub(crate) fn build_scope_path_query(
    path_exact_field: Field,
    search_root: &Path,
    index_root: &Path,
) -> Option<Box<dyn Query>> {
    let mut search_variants = vec![search_root.to_path_buf()];
    if let Ok(canonical) = search_root.canonicalize() {
        if !search_variants.iter().any(|v| v == &canonical) {
            search_variants.push(canonical);
        }
    }
    let mut index_variants = vec![index_root.to_path_buf()];
    if let Ok(canonical) = index_root.canonicalize() {
        if !index_variants.iter().any(|v| v == &canonical) {
            index_variants.push(canonical);
        }
    }

    let is_same_root = search_variants
        .iter()
        .any(|search| index_variants.iter().any(|index| search == index));
    if is_same_root {
        return None;
    }
    let is_within_index = search_variants
        .iter()
        .any(|search| index_variants.iter().any(|index| search.starts_with(index)));
    if !is_within_index {
        return None;
    }

    let mut scope_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
    let mut seen_terms: HashSet<String> = HashSet::new();
    let mut seen_patterns: HashSet<String> = HashSet::new();

    for search_variant in &search_variants {
        if search_variant.is_file() {
            let value = search_variant.to_string_lossy().to_string();
            if seen_terms.insert(value.clone()) {
                let term = Term::from_field_text(path_exact_field, &value);
                scope_queries.push((
                    Occur::Should,
                    Box::new(TermQuery::new(term, IndexRecordOption::Basic)),
                ));
            }
        } else {
            let mut absolute_prefix = search_variant.to_string_lossy().to_string();
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

        for index_variant in &index_variants {
            let rel_scope = search_variant
                .strip_prefix(index_variant)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if rel_scope.is_empty() {
                continue;
            }

            if search_variant.is_file() {
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
