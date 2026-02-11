// SPDX-License-Identifier: MIT OR Apache-2.0

//! Full-text search with BM25 ranking using tantivy

use anyhow::{Context, Result};
use colored::Colorize;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;
use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, FuzzyTermQuery, Occur, QueryParser, RegexQuery, TermQuery},
    schema::{Term, Value},
    Index, TantivyDocument,
};

use crate::cli::OutputFormat;
use crate::indexer::scanner::FileScanner;
use crate::query::changed_files::ChangedFiles;
use cgrep::cache::{CacheKey, SearchCache};
use cgrep::config::{Config, EmbeddingProviderType};
use cgrep::embedding::{
    CommandProvider, DummyProvider, EmbeddingProvider, EmbeddingProviderConfig, EmbeddingStorage,
    FastEmbedder, DEFAULT_EMBEDDING_DIM,
};
use cgrep::errors::IndexNotFoundError;
use cgrep::filters::{
    matches_file_type, matches_glob_compiled, should_exclude_compiled, CompiledGlob,
};
use cgrep::hybrid::{
    BM25Result, HybridConfig, HybridResult, HybridSearcher, SearchMode as HybridSearchMode,
};
use cgrep::output::{
    colorize_context, colorize_line_num, colorize_match, colorize_path, print_json, use_colors,
};
use cgrep::utils::INDEX_DIR;
const DEFAULT_CACHE_TTL_MS: u64 = 600_000; // 10 minutes

/// Search result for internal use and text output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub path: String,
    pub score: f32,
    pub snippet: String,
    pub line: Option<usize>,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
    /// BM25/text score for hybrid search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_score: Option<f32>,
    /// Vector/embedding score for hybrid search
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_score: Option<f32>,
    /// Combined hybrid score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hybrid_score: Option<f32>,
    /// Unique result identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_id: Option<String>,
    /// Symbol start line (for semantic/hybrid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_start: Option<u32>,
    /// Symbol end line (for semantic/hybrid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_end: Option<u32>,
}

/// Minimal search result for JSON output
#[derive(Debug, Serialize)]
struct SearchResultJson<'a> {
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    snippet: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_before: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_after: Option<&'a [String]>,
}

impl<'a> SearchResultJson<'a> {
    fn from_result(result: &'a SearchResult) -> Self {
        Self {
            path: result.path.as_str(),
            line: result.line,
            snippet: result.snippet.as_str(),
            context_before: if result.context_before.is_empty() {
                None
            } else {
                Some(result.context_before.as_slice())
            },
            context_after: if result.context_after.is_empty() {
                None
            } else {
                Some(result.context_after.as_slice())
            },
        }
    }
}

/// Ultra-minimal search result for compact JSON output (AI agent optimized)
#[derive(Debug, Serialize)]
struct SearchResultCompactJson<'a> {
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    snippet: &'a str,
}

impl<'a> SearchResultCompactJson<'a> {
    fn from_result(result: &'a SearchResult) -> Self {
        Self {
            path: result.path.as_str(),
            line: result.line,
            snippet: result.snippet.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexMode {
    Index,
    Scan,
}

struct SearchOutcome {
    results: Vec<SearchResult>,
    files_with_matches: usize,
    total_matches: usize,
    mode: IndexMode,
    cache_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeywordCachePayload {
    results: Vec<SearchResult>,
    files_with_matches: usize,
    total_matches: usize,
    mode: String,
}

#[derive(Debug, Serialize)]
struct SearchJson2Meta<'a> {
    schema_version: &'a str,
    query: &'a str,
    search_mode: String,
    index_mode: &'static str,
    elapsed_ms: f64,
    files_with_matches: usize,
    total_matches: usize,
    cache_hit: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_pack: Option<usize>,
    truncated: bool,
    dropped_results: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_total_chars: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_chars_per_snippet: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_context_chars: Option<usize>,
    dedupe_context: bool,
    path_alias: bool,
    suppress_boilerplate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed_rev: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_aliases: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Serialize)]
struct SearchJson2Result {
    id: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<usize>,
    snippet: String,
    score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vector_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hybrid_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_before: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_after: Option<Vec<String>>,
}

impl SearchJson2Result {
    fn from_result(result: &SearchResult, include_context: bool, path_value: Option<&str>) -> Self {
        let (start_line, end_line) = if let Some(line) = result.line {
            let start = line.saturating_sub(result.context_before.len());
            let end = line + result.context_after.len();
            (Some(start), Some(end))
        } else {
            (None, None)
        };

        let id = result
            .result_id
            .clone()
            .unwrap_or_else(|| stable_result_id(result));

        Self {
            id,
            path: path_value.unwrap_or(result.path.as_str()).to_string(),
            line: result.line,
            start_line,
            end_line,
            snippet: result.snippet.clone(),
            score: result.score,
            text_score: result.text_score,
            vector_score: result.vector_score,
            hybrid_score: result.hybrid_score,
            context_before: if include_context && !result.context_before.is_empty() {
                Some(result.context_before.clone())
            } else {
                None
            },
            context_after: if include_context && !result.context_after.is_empty() {
                Some(result.context_after.clone())
            } else {
                None
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct SearchJson2Payload<'a> {
    meta: SearchJson2Meta<'a>,
    results: Vec<SearchJson2Result>,
}

#[derive(Debug, Clone, Copy)]
struct SearchOutputBudget {
    max_chars_per_snippet: Option<usize>,
    max_total_chars: Option<usize>,
    max_context_chars: Option<usize>,
    dedupe_context: bool,
    suppress_boilerplate: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct BudgetApplyStats {
    truncated: bool,
    dropped_results: usize,
}

/// Run the search command
#[allow(clippy::too_many_arguments)]
pub fn run(
    query: &str,
    path: Option<&str>,
    max_results: usize,
    context: usize,
    file_type: Option<&str>,
    glob_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
    changed: Option<&str>,
    quiet: bool,
    fuzzy: bool,
    no_index: bool,
    regex: bool,
    case_sensitive: bool,
    format: OutputFormat,
    compact: bool,
    search_mode: Option<HybridSearchMode>,
    context_pack: Option<usize>,
    use_cache: bool,
    cache_ttl: Option<u64>,
    max_chars_per_snippet: Option<usize>,
    max_total_chars: Option<usize>,
    max_context_chars: Option<usize>,
    dedupe_context: bool,
    path_alias: bool,
    suppress_boilerplate: bool,
) -> Result<()> {
    let start_time = Instant::now();
    let use_color = use_colors() && format == OutputFormat::Text;

    // Precompile glob patterns for efficient repeated matching
    let compiled_glob = glob_pattern.and_then(CompiledGlob::new);
    let compiled_exclude = exclude_pattern.and_then(CompiledGlob::new);

    let search_root = resolve_search_root(path)?;

    // Find index root (may be in parent directory)
    let (index_root, index_path, using_parent) = match cgrep::utils::find_index_root(&search_root) {
        Some(index_root) => (
            index_root.root.clone(),
            index_root.index_path,
            index_root.is_parent,
        ),
        None => (search_root.clone(), search_root.join(INDEX_DIR), false),
    };

    // Load config relative to the index root so running from subdirectories works.
    let config = Config::load_for_dir(&index_root);
    let effective_max_results = max_results;
    let config_exclude_patterns: Vec<CompiledGlob> = config
        .exclude_patterns
        .iter()
        .filter_map(|p| CompiledGlob::new(p.as_str()))
        .collect();
    let changed_filter = changed
        .map(|rev| ChangedFiles::from_scope(&search_root, rev))
        .transpose()?;

    if using_parent {
        eprintln!("Using index from: {}", index_root.display());
    }

    let requested_mode = if no_index || regex {
        IndexMode::Scan
    } else {
        IndexMode::Index
    };

    if requested_mode == IndexMode::Scan && fuzzy {
        eprintln!("Warning: --fuzzy is only supported with index search; ignoring.");
    }

    let compiled_regex = if regex {
        Some(
            RegexBuilder::new(query)
                .case_insensitive(!case_sensitive)
                .build()
                .context("Invalid regex pattern")?,
        )
    } else {
        None
    };

    // Check for hybrid search mode
    let effective_search_mode = search_mode.unwrap_or(HybridSearchMode::Keyword);
    let effective_cache_ttl = cache_ttl.unwrap_or(DEFAULT_CACHE_TTL_MS);

    let mut outcome = match effective_search_mode {
        HybridSearchMode::Semantic | HybridSearchMode::Hybrid => {
            // Use hybrid search
            hybrid_search(
                query,
                &index_root,
                &search_root,
                &config,
                effective_max_results,
                context,
                file_type,
                glob_pattern,
                exclude_pattern,
                compiled_glob.as_ref(),
                compiled_exclude.as_ref(),
                &config_exclude_patterns,
                changed_filter.as_ref(),
                effective_search_mode,
                use_cache,
                effective_cache_ttl,
            )?
        }
        HybridSearchMode::Keyword => keyword_search(
            query,
            &index_root,
            &search_root,
            &index_path,
            effective_max_results,
            context,
            file_type,
            glob_pattern,
            exclude_pattern,
            compiled_glob.as_ref(),
            compiled_exclude.as_ref(),
            &config_exclude_patterns,
            changed_filter.as_ref(),
            requested_mode,
            fuzzy,
            compiled_regex.as_ref(),
            case_sensitive,
            use_cache,
            effective_cache_ttl,
        )?,
    };

    let effective_context_pack = context_pack.filter(|v| *v > 0);
    if let Some(pack_gap) = effective_context_pack {
        apply_context_pack(&mut outcome.results, pack_gap);
    }

    let budget = SearchOutputBudget {
        max_chars_per_snippet,
        max_total_chars,
        max_context_chars,
        dedupe_context: dedupe_context || format == OutputFormat::Json2,
        suppress_boilerplate: suppress_boilerplate || format == OutputFormat::Json2,
    };
    let budget_stats = apply_output_budget(&mut outcome.results, budget);
    let (path_alias_lookup, path_aliases_meta) = if format == OutputFormat::Json2 && path_alias {
        let (lookup, aliases) = build_path_aliases(&outcome.results);
        (Some(lookup), Some(aliases))
    } else {
        (None, None)
    };

    let elapsed = start_time.elapsed();

    // Output based on format
    match format {
        OutputFormat::Json => {
            if compact {
                let json_results: Vec<SearchResultCompactJson<'_>> = outcome
                    .results
                    .iter()
                    .map(SearchResultCompactJson::from_result)
                    .collect();
                print_json(&json_results, compact)?;
            } else {
                let json_results: Vec<SearchResultJson<'_>> = outcome
                    .results
                    .iter()
                    .map(SearchResultJson::from_result)
                    .collect();
                print_json(&json_results, compact)?;
            }
        }
        OutputFormat::Json2 => {
            let json2_results: Vec<SearchJson2Result> = outcome
                .results
                .iter()
                .map(|result| {
                    let alias = path_alias_lookup
                        .as_ref()
                        .and_then(|lookup| lookup.get(&result.path))
                        .map(|s| s.as_str());
                    SearchJson2Result::from_result(result, !compact, alias)
                })
                .collect();

            let payload = SearchJson2Payload {
                meta: SearchJson2Meta {
                    schema_version: "1",
                    query,
                    search_mode: effective_search_mode.to_string(),
                    index_mode: match outcome.mode {
                        IndexMode::Index => "index",
                        IndexMode::Scan => "scan",
                    },
                    elapsed_ms: elapsed.as_secs_f64() * 1000.0,
                    files_with_matches: outcome.files_with_matches,
                    total_matches: outcome.total_matches,
                    cache_hit: outcome.cache_hit,
                    context_pack: effective_context_pack,
                    truncated: budget_stats.truncated,
                    dropped_results: budget_stats.dropped_results,
                    max_total_chars: budget.max_total_chars,
                    max_chars_per_snippet: budget.max_chars_per_snippet,
                    max_context_chars: budget.max_context_chars,
                    dedupe_context: budget.dedupe_context,
                    path_alias,
                    suppress_boilerplate: budget.suppress_boilerplate,
                    changed_rev: changed_filter.as_ref().map(|f| f.rev()),
                    path_aliases: path_aliases_meta,
                },
                results: json2_results,
            };

            print_json(&payload, compact)?;
        }
        OutputFormat::Text => {
            if outcome.results.is_empty() {
                if use_color {
                    println!("{} No results found for: {}", "✗".red(), query.yellow());
                } else {
                    println!("No results found for: {}", query);
                }
            } else {
                if use_color {
                    println!(
                        "\n{} Found {} results for: {}\n",
                        "✓".green(),
                        outcome.results.len().to_string().cyan(),
                        query.yellow()
                    );
                } else {
                    println!("\nFound {} results for: {}\n", outcome.results.len(), query);
                }

                let highlight_snippet = |snippet: &str| {
                    if outcome.mode == IndexMode::Scan {
                        if let Some(re) = compiled_regex.as_ref() {
                            highlight_matches_regex(snippet, re, use_color)
                        } else {
                            highlight_matches(snippet, query, use_color)
                        }
                    } else {
                        highlight_matches(snippet, query, use_color)
                    }
                };

                let format_line_prefix = |marker: &str, line_num: usize, width: usize| {
                    let padded = format!("{:>width$}", line_num, width = width);
                    let num = if use_color {
                        padded.yellow().to_string()
                    } else {
                        padded
                    };
                    let marker = if use_color && marker == ">" {
                        marker.blue().to_string()
                    } else {
                        marker.to_string()
                    };
                    format!("{} {} | ", marker, num)
                };

                let mut prev_had_context = false;
                for (idx, result) in outcome.results.iter().enumerate() {
                    let has_context =
                        !result.context_before.is_empty() || !result.context_after.is_empty();

                    // Print separator between context groups
                    if idx > 0 && (prev_had_context || has_context) {
                        println!(
                            "{}",
                            if use_color {
                                "--".dimmed().to_string()
                            } else {
                                "--".to_string()
                            }
                        );
                    }

                    // Print match header
                    let line_info = result
                        .line
                        .map(|l| format!(":{}", colorize_line_num(l, use_color)))
                        .unwrap_or_default();

                    if use_color {
                        println!("{}{}", colorize_path(&result.path, use_color), line_info);
                    } else {
                        println!("{}{}", result.path, line_info);
                    }

                    if has_context {
                        if let Some(match_line) = result.line {
                            let max_line = match_line + result.context_after.len();
                            let min_line = match_line.saturating_sub(result.context_before.len());
                            let width = std::cmp::max(max_line, min_line).to_string().len();

                            // Print context before
                            for (i, line) in result.context_before.iter().enumerate() {
                                let ctx_line_num =
                                    match_line.saturating_sub(result.context_before.len() - i);
                                let prefix = format_line_prefix(" ", ctx_line_num, width);
                                println!("{}{}", prefix, colorize_context(line, use_color));
                            }

                            // Print match line (single-line snippet)
                            if !result.snippet.is_empty() {
                                let highlighted = highlight_snippet(&result.snippet);
                                let match_text = highlighted.lines().next().unwrap_or("");
                                let prefix = format_line_prefix(">", match_line, width);
                                println!("{}{}", prefix, match_text);
                            }

                            // Print context after
                            for (i, line) in result.context_after.iter().enumerate() {
                                let ctx_line_num = match_line + i + 1;
                                let prefix = format_line_prefix(" ", ctx_line_num, width);
                                println!("{}{}", prefix, colorize_context(line, use_color));
                            }
                        }
                    } else if !result.snippet.is_empty() {
                        let highlighted = highlight_snippet(&result.snippet);
                        for line in highlighted.lines().take(3) {
                            println!("    {}", line);
                        }
                    }

                    prev_had_context = has_context;

                    if !has_context {
                        println!();
                    }
                }
            }

            // Print stats unless quiet
            if !quiet {
                eprintln!(
                    "\n{} files | {} matches | {:.2}ms",
                    outcome.files_with_matches,
                    outcome.total_matches,
                    elapsed.as_secs_f64() * 1000.0
                );
            }
        }
    }

    Ok(())
}

fn stable_result_id(result: &SearchResult) -> String {
    let payload = format!(
        "{}:{}:{}",
        result.path,
        result.line.unwrap_or(0),
        result.snippet
    );
    let hash = blake3::hash(payload.as_bytes());
    hash.to_hex()[..16].to_string()
}

fn apply_context_pack(results: &mut [SearchResult], pack_gap: usize) {
    let mut last_end_by_path: HashMap<String, usize> = HashMap::new();

    for result in results.iter_mut() {
        let Some(line) = result.line else {
            continue;
        };

        if result.context_before.is_empty() && result.context_after.is_empty() {
            continue;
        }

        let start = line.saturating_sub(result.context_before.len());
        let end = line + result.context_after.len();

        if let Some(last_end) = last_end_by_path.get_mut(&result.path) {
            if start <= last_end.saturating_add(pack_gap) {
                if start <= *last_end {
                    let overlap = *last_end - start + 1;
                    let trim_before = overlap.min(result.context_before.len());
                    if trim_before > 0 {
                        result.context_before.drain(0..trim_before);
                    }
                    if end <= *last_end {
                        result.context_after.clear();
                    }
                }
                *last_end = (*last_end).max(end);
            } else {
                *last_end = end;
            }
        } else {
            last_end_by_path.insert(result.path.clone(), end);
        }
    }
}

fn apply_output_budget(
    results: &mut Vec<SearchResult>,
    budget: SearchOutputBudget,
) -> BudgetApplyStats {
    let mut stats = BudgetApplyStats::default();

    if budget.suppress_boilerplate && suppress_repeated_boilerplate(results) {
        stats.truncated = true;
    }

    if budget.dedupe_context {
        dedupe_context_lines(results);
    }

    if let Some(max_chars) = budget.max_chars_per_snippet {
        for result in results.iter_mut() {
            let original = result.snippet.clone();
            result.snippet = truncate_with_ellipsis(&result.snippet, max_chars);
            if result.snippet != original {
                stats.truncated = true;
            }
        }
    }

    if let Some(max_context_chars) = budget.max_context_chars {
        for result in results.iter_mut() {
            let trimmed = trim_result_context_chars(result, max_context_chars);
            if trimmed {
                stats.truncated = true;
            }
        }
    }

    if let Some(max_total_chars) = budget.max_total_chars {
        let total_stats = enforce_total_chars_budget(results, max_total_chars);
        stats.truncated |= total_stats.truncated;
        stats.dropped_results = total_stats.dropped_results;
    }

    stats
}

fn dedupe_context_lines(results: &mut [SearchResult]) {
    let mut seen_by_path: HashMap<String, HashSet<String>> = HashMap::new();

    for result in results.iter_mut() {
        let seen = seen_by_path.entry(result.path.clone()).or_default();
        result
            .context_before
            .retain(|line| seen.insert(line.to_string()));
        result
            .context_after
            .retain(|line| seen.insert(line.to_string()));
    }
}

fn suppress_repeated_boilerplate(results: &mut [SearchResult]) -> bool {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for result in results.iter() {
        let mut lines =
            Vec::with_capacity(1 + result.context_before.len() + result.context_after.len());
        lines.push(result.snippet.as_str());
        lines.extend(result.context_before.iter().map(String::as_str));
        lines.extend(result.context_after.iter().map(String::as_str));

        for line in lines {
            if is_boilerplate_line(line) {
                *counts.entry(normalize_boilerplate_line(line)).or_insert(0) += 1;
            }
        }
    }

    let mut changed = false;
    for result in results.iter_mut() {
        if is_repeated_boilerplate(&result.snippet, &counts) {
            result.snippet = "[boilerplate suppressed]".to_string();
            changed = true;
        }

        let before_len = result.context_before.len();
        result
            .context_before
            .retain(|line| !is_repeated_boilerplate(line, &counts));
        changed |= before_len != result.context_before.len();

        let after_len = result.context_after.len();
        result
            .context_after
            .retain(|line| !is_repeated_boilerplate(line, &counts));
        changed |= after_len != result.context_after.len();
    }

    changed
}

fn is_repeated_boilerplate(line: &str, counts: &HashMap<String, usize>) -> bool {
    if !is_boilerplate_line(line) {
        return false;
    }
    let key = normalize_boilerplate_line(line);
    counts.get(&key).copied().unwrap_or(0) > 1
}

fn normalize_boilerplate_line(line: &str) -> String {
    line.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn is_boilerplate_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_lowercase();
    lower.starts_with("use ")
        || lower.starts_with("pub use ")
        || lower.starts_with("import ")
        || lower.starts_with("from ")
        || lower.starts_with("#include")
        || lower.starts_with("package ")
        || lower.starts_with("namespace ")
        || lower.starts_with("module ")
        || lower.starts_with("export ")
        || lower.starts_with("//")
        || lower.starts_with("/*")
        || lower.starts_with('*')
        || matches!(trimmed, "{" | "}" | "(" | ")" | "[" | "]")
}

fn trim_result_context_chars(result: &mut SearchResult, max_context_chars: usize) -> bool {
    let mut remaining = max_context_chars;
    let mut trimmed = false;

    for line in result.context_before.iter_mut() {
        let original = line.clone();
        *line = truncate_with_ellipsis(line, remaining);
        if *line != original {
            trimmed = true;
        }
        remaining = remaining.saturating_sub(char_count(line));
    }
    result.context_before.retain(|line| !line.is_empty());

    for line in result.context_after.iter_mut() {
        let original = line.clone();
        *line = truncate_with_ellipsis(line, remaining);
        if *line != original {
            trimmed = true;
        }
        remaining = remaining.saturating_sub(char_count(line));
    }
    result.context_after.retain(|line| !line.is_empty());

    trimmed
}

fn enforce_total_chars_budget(
    results: &mut Vec<SearchResult>,
    max_total_chars: usize,
) -> BudgetApplyStats {
    if max_total_chars == 0 {
        let dropped_results = results.len();
        results.clear();
        return BudgetApplyStats {
            truncated: dropped_results > 0,
            dropped_results,
        };
    }

    let mut used = 0usize;
    let mut keep = 0usize;
    let mut truncated = false;

    for result in results.iter_mut() {
        let mandatory = mandatory_chars(result);
        if used + mandatory > max_total_chars {
            truncated = true;
            break;
        }
        used += mandatory;

        let optional_budget = max_total_chars.saturating_sub(used);
        if optional_budget == 0 {
            if !result.snippet.is_empty()
                || !result.context_before.is_empty()
                || !result.context_after.is_empty()
            {
                truncated = true;
            }
            result.snippet.clear();
            result.context_before.clear();
            result.context_after.clear();
            keep += 1;
            continue;
        }

        let snippet_chars = char_count(&result.snippet);
        let mut remaining_for_context = optional_budget;
        if snippet_chars > optional_budget {
            truncated = true;
            result.snippet = truncate_with_ellipsis(&result.snippet, optional_budget);
            result.context_before.clear();
            result.context_after.clear();
            keep += 1;
            used += optional_chars(result);
            continue;
        }
        remaining_for_context = remaining_for_context.saturating_sub(snippet_chars);

        if context_chars(result) > remaining_for_context
            && trim_result_context_chars(result, remaining_for_context)
        {
            truncated = true;
        }

        used += optional_chars(result);
        keep += 1;
    }

    let dropped_results = results.len().saturating_sub(keep);
    results.truncate(keep);
    if dropped_results > 0 {
        truncated = true;
    }

    BudgetApplyStats {
        truncated,
        dropped_results,
    }
}

fn mandatory_chars(result: &SearchResult) -> usize {
    let line_chars = result
        .line
        .map(|line| line.to_string().len())
        .unwrap_or_default();
    result.path.len() + line_chars
}

fn optional_chars(result: &SearchResult) -> usize {
    char_count(&result.snippet) + context_chars(result)
}

fn context_chars(result: &SearchResult) -> usize {
    result
        .context_before
        .iter()
        .chain(result.context_after.iter())
        .map(|line| char_count(line))
        .sum()
}

fn truncate_with_ellipsis(input: &str, max_chars: usize) -> String {
    let total = char_count(input);
    if total <= max_chars {
        return input.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    if max_chars <= 3 {
        return input.chars().take(max_chars).collect();
    }

    let keep = max_chars - 3;
    let mut out: String = input.chars().take(keep).collect();
    out.push_str("...");
    out
}

fn char_count(input: &str) -> usize {
    input.chars().count()
}

fn build_path_aliases(
    results: &[SearchResult],
) -> (HashMap<String, String>, BTreeMap<String, String>) {
    let mut unique_paths = BTreeSet::new();
    for result in results {
        unique_paths.insert(result.path.clone());
    }

    let mut lookup = HashMap::new();
    let mut aliases = BTreeMap::new();

    for (idx, path) in unique_paths.into_iter().enumerate() {
        let alias = format!("p{}", idx + 1);
        lookup.insert(path.clone(), alias.clone());
        aliases.insert(alias, path);
    }

    (lookup, aliases)
}

fn normalize_query(query: &str, lowercase: bool, collapse_spaces: bool) -> String {
    let normalized = if collapse_spaces {
        query.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        query.to_string()
    };
    if lowercase {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

fn index_fingerprint(index_root: &Path) -> Option<String> {
    let metadata_path = index_root.join(INDEX_DIR).join("metadata.json");
    let bytes = fs::read(&metadata_path).ok()?;
    Some(blake3::hash(&bytes).to_hex()[..16].to_string())
}

fn context_for_line_cached(
    file_path: &Path,
    line_num: Option<usize>,
    context: usize,
    cache: &mut HashMap<PathBuf, Vec<String>>,
) -> (Vec<String>, Vec<String>) {
    if context == 0 {
        return (vec![], vec![]);
    }

    let Some(line) = line_num else {
        return (vec![], vec![]);
    };

    let lines = cache
        .entry(file_path.to_path_buf())
        .or_insert_with(|| read_file_lines(file_path).unwrap_or_default());
    get_context_from_string_lines(lines, line, context)
}

fn read_file_lines(file_path: &Path) -> Option<Vec<String>> {
    let file = fs::File::open(file_path).ok()?;
    let reader = BufReader::new(file);
    Some(reader.lines().map_while(|line| line.ok()).collect())
}

fn get_context_from_string_lines(
    lines: &[String],
    line_num: usize,
    context: usize,
) -> (Vec<String>, Vec<String>) {
    if lines.is_empty() || context == 0 {
        return (vec![], vec![]);
    }

    let idx = line_num.saturating_sub(1);
    let start = idx.saturating_sub(context);
    let end = (idx + context + 1).min(lines.len());

    let before = if idx > start {
        lines[start..idx].to_vec()
    } else {
        vec![]
    };
    let after = if idx + 1 < end {
        lines[idx + 1..end].to_vec()
    } else {
        vec![]
    };

    (before, after)
}

struct IndexCandidate {
    stored_path: String,
    full_path: PathBuf,
    display_path: String,
    score: f32,
    snippet: String,
    line: Option<usize>,
    symbol_id: Option<String>,
    symbol_start: Option<u32>,
    symbol_end: Option<u32>,
}

#[allow(clippy::too_many_arguments)]
fn collect_index_candidates(
    query: &str,
    index_root: &Path,
    search_root: &Path,
    max_candidates: usize,
    doc_type: &str,
    file_type: Option<&str>,
    compiled_glob: Option<&CompiledGlob>,
    compiled_exclude: Option<&CompiledGlob>,
    config_exclude_patterns: &[CompiledGlob],
    changed_filter: Option<&ChangedFiles>,
    fuzzy: bool,
) -> Result<Vec<IndexCandidate>> {
    let index_path = index_root.join(INDEX_DIR);
    if !index_path.exists() {
        return Err(IndexNotFoundError {
            index_path: index_path.display().to_string(),
        }
        .into());
    }

    let index = Index::open_in_dir(&index_path).context("Failed to open index")?;
    let reader = index.reader()?;
    let searcher = reader.searcher();

    let schema = index.schema();
    let content_field = schema
        .get_field("content")
        .context("Missing content field")?;
    let path_field = schema.get_field("path").context("Missing path field")?;
    let symbols_field = schema
        .get_field("symbols")
        .context("Missing symbols field")?;
    let doc_type_field = schema
        .get_field("doc_type")
        .context("Missing doc_type field")?;
    let symbol_id_field = schema
        .get_field("symbol_id")
        .context("Missing symbol_id field")?;
    let symbol_end_line_field = schema
        .get_field("symbol_end_line")
        .context("Missing symbol_end_line field")?;
    let line_offset_field = schema
        .get_field("line_number")
        .context("Missing line_number field")?;
    let path_exact_field = schema.get_field("path_exact").ok();

    let text_query: Box<dyn tantivy::query::Query> = if fuzzy {
        let terms: Vec<&str> = query.split_whitespace().collect();
        if terms.is_empty() {
            anyhow::bail!("Fuzzy search requires at least one search term");
        }
        let mut fuzzy_queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();

        for term in terms {
            let distance = if term.len() <= 4 { 1 } else { 2 };

            let content_term = Term::from_field_text(content_field, term);
            let content_fuzzy = FuzzyTermQuery::new(content_term, distance, true);
            fuzzy_queries.push((Occur::Should, Box::new(content_fuzzy)));

            let symbols_term = Term::from_field_text(symbols_field, term);
            let symbols_fuzzy = FuzzyTermQuery::new(symbols_term, distance, true);
            fuzzy_queries.push((Occur::Should, Box::new(symbols_fuzzy)));
        }

        Box::new(BooleanQuery::new(fuzzy_queries))
    } else {
        let mut query_parser =
            QueryParser::for_index(&index, vec![content_field, symbols_field, path_field]);
        query_parser.set_field_boost(symbols_field, 2.5);
        query_parser.set_field_boost(path_field, 0.3);
        let (parsed_query, _errors) = query_parser.parse_query_lenient(query);
        parsed_query
    };

    let doc_type_term = Term::from_field_text(doc_type_field, doc_type);
    let doc_type_query = TermQuery::new(doc_type_term, tantivy::schema::IndexRecordOption::Basic);
    let mut clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> = vec![
        (Occur::Must, text_query),
        (Occur::Must, Box::new(doc_type_query)),
    ];
    if let Some(scope_query) =
        path_exact_field.and_then(|f| build_search_scope_query(f, search_root, index_root))
    {
        clauses.push((Occur::Must, scope_query));
    }
    let parsed_query: Box<dyn tantivy::query::Query> = Box::new(BooleanQuery::new(clauses));

    let fetch_limit = max_candidates.saturating_mul(5).max(1);
    let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(fetch_limit))?;

    let mut candidates: Vec<IndexCandidate> = Vec::new();

    for (score, doc_address) in &top_docs {
        if candidates.len() >= max_candidates {
            break;
        }

        let doc: TantivyDocument = searcher.doc(*doc_address)?;
        let path_value = doc
            .get_first(path_field)
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let full_path = resolve_full_path(path_value, index_root);
        let Some(display_path) = scoped_display_path(&full_path, search_root) else {
            continue;
        };
        if let Some(filter) = changed_filter {
            if !filter.matches_rel_path(&display_path) {
                continue;
            }
        }

        if !matches_file_type(&display_path, file_type) {
            continue;
        }
        if !matches_glob_compiled(&display_path, compiled_glob) {
            continue;
        }
        if should_exclude_compiled(&display_path, compiled_exclude) {
            continue;
        }
        if config_exclude_patterns
            .iter()
            .any(|p| should_exclude_compiled(&display_path, Some(p)))
        {
            continue;
        }

        let content_value = doc
            .get_first(content_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let line_offset = doc
            .get_first(line_offset_field)
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let doc_type_value = doc
            .get_first(doc_type_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let symbol_id = if doc_type_value == "symbol" {
            doc.get_first(symbol_id_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        let symbol_end = if doc_type_value == "symbol" {
            doc.get_first(symbol_end_line_field)
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
        } else {
            None
        };

        if doc_type_value == "file" {
            let matches = find_snippets_with_lines(content_value, query, 150);
            if !matches.is_empty() {
                for (snippet, rel_line) in matches {
                    if candidates.len() >= max_candidates {
                        break;
                    }

                    candidates.push(IndexCandidate {
                        stored_path: path_value.to_string(),
                        full_path: full_path.clone(),
                        display_path: display_path.clone(),
                        score: *score,
                        snippet,
                        line: Some(line_offset + rel_line.saturating_sub(1)),
                        symbol_id: None,
                        symbol_start: None,
                        symbol_end: None,
                    });
                }
                continue;
            }
        }

        let (snippet, line_num) = find_snippet_with_line(content_value, query, 150);
        let mut line_num = line_num.map(|l| l + line_offset.saturating_sub(1));
        if line_num.is_none() && doc_type_value == "symbol" {
            line_num = Some(line_offset);
        }

        candidates.push(IndexCandidate {
            stored_path: path_value.to_string(),
            full_path,
            display_path,
            score: *score,
            snippet,
            line: line_num,
            symbol_id,
            symbol_start: if doc_type_value == "symbol" {
                Some(line_offset as u32)
            } else {
                None
            },
            symbol_end,
        });
    }

    Ok(candidates)
}

#[allow(clippy::too_many_arguments)]
fn keyword_search(
    query: &str,
    index_root: &Path,
    search_root: &Path,
    index_path: &Path,
    max_results: usize,
    context: usize,
    file_type: Option<&str>,
    glob_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
    compiled_glob: Option<&CompiledGlob>,
    compiled_exclude: Option<&CompiledGlob>,
    config_exclude_patterns: &[CompiledGlob],
    changed_filter: Option<&ChangedFiles>,
    requested_mode: IndexMode,
    fuzzy: bool,
    regex: Option<&Regex>,
    case_sensitive: bool,
    use_cache: bool,
    cache_ttl_ms: u64,
) -> Result<SearchOutcome> {
    let use_index = requested_mode == IndexMode::Index && index_path.exists();
    if requested_mode == IndexMode::Index && !index_path.exists() {
        eprintln!(
            "Index not found at {}. Falling back to scan mode.",
            index_path.display()
        );
    }
    let effective_mode = if use_index {
        IndexMode::Index
    } else {
        IndexMode::Scan
    };

    let normalized_query = if regex.is_some() {
        query.to_string()
    } else {
        normalize_query(
            query,
            effective_mode == IndexMode::Index || !case_sensitive,
            effective_mode == IndexMode::Index,
        )
    };
    let changed_component = changed_filter
        .map(|f| format!("{}:{}", f.rev(), f.signature()))
        .filter(|s| !s.is_empty());
    let cache_key = CacheKey {
        query: normalized_query,
        mode: if effective_mode == IndexMode::Index {
            "keyword:index".to_string()
        } else {
            "keyword:scan".to_string()
        },
        max_results,
        context,
        file_type: file_type.map(str::to_string),
        glob: glob_pattern.map(str::to_string),
        exclude: exclude_pattern.map(str::to_string),
        profile: None,
        index_hash: index_fingerprint(index_root),
        embedding_model: None,
        search_root: Some(search_root.to_string_lossy().to_string()),
        changed: changed_component,
    };

    if use_cache {
        if let Ok(cache) = SearchCache::new(index_root, cache_ttl_ms) {
            if let Ok(Some(entry)) = cache.get::<KeywordCachePayload>(&cache_key) {
                return Ok(SearchOutcome {
                    results: entry.data.results,
                    files_with_matches: entry.data.files_with_matches,
                    total_matches: entry.data.total_matches,
                    mode: parse_index_mode(&entry.data.mode),
                    cache_hit: true,
                });
            }
        }
    }

    let outcome = if use_index {
        index_search(
            query,
            index_root,
            search_root,
            max_results,
            context,
            file_type,
            compiled_glob,
            compiled_exclude,
            config_exclude_patterns,
            changed_filter,
            fuzzy,
        )?
    } else {
        scan_search(
            query,
            search_root,
            max_results,
            context,
            file_type,
            compiled_glob,
            compiled_exclude,
            config_exclude_patterns,
            changed_filter,
            regex,
            case_sensitive,
        )?
    };

    if use_cache {
        if let Ok(cache) = SearchCache::new(index_root, cache_ttl_ms) {
            let payload = KeywordCachePayload {
                results: outcome.results.clone(),
                files_with_matches: outcome.files_with_matches,
                total_matches: outcome.total_matches,
                mode: match outcome.mode {
                    IndexMode::Index => "index".to_string(),
                    IndexMode::Scan => "scan".to_string(),
                },
            };
            let _ = cache.put(&cache_key, payload);
        }
    }

    Ok(outcome)
}

fn parse_index_mode(mode: &str) -> IndexMode {
    if mode.eq_ignore_ascii_case("scan") {
        IndexMode::Scan
    } else {
        IndexMode::Index
    }
}

fn normalized_hybrid_weights(weight_text: f32, weight_vector: f32) -> (f32, f32) {
    let text = if weight_text.is_finite() {
        weight_text.max(0.0)
    } else {
        0.0
    };
    let vector = if weight_vector.is_finite() {
        weight_vector.max(0.0)
    } else {
        0.0
    };
    let total = text + vector;
    if total <= f32::EPSILON {
        return (0.7, 0.3);
    }
    (text / total, vector / total)
}

fn fallback_hybrid_results(bm25_results: &[BM25Result]) -> Vec<HybridResult> {
    let max_text_score = bm25_results
        .iter()
        .map(|r| r.score)
        .fold(f32::NEG_INFINITY, f32::max);

    bm25_results
        .iter()
        .map(|r| {
            let text_norm = if max_text_score > 0.0 {
                r.score / max_text_score
            } else {
                0.0
            };
            HybridResult {
                path: r.path.clone(),
                score: text_norm,
                text_score: r.score,
                vector_score: 0.0,
                text_norm,
                vector_norm: 0.0,
                snippet: r.snippet.clone(),
                line: r.line,
                chunk_start: r.chunk_start,
                chunk_end: r.chunk_end,
                result_id: r.symbol_id.clone(),
            }
        })
        .collect()
}

fn semantic_backfill_results(
    storage: &EmbeddingStorage,
    query_embedding: &[f32],
    top_k: usize,
) -> Vec<HybridResult> {
    if top_k == 0 {
        return Vec::new();
    }

    storage
        .search_similar(query_embedding, top_k)
        .unwrap_or_default()
        .into_iter()
        .map(|result| {
            let vector_score = result.score;
            let vector_norm = (vector_score + 1.0) / 2.0;
            HybridResult {
                path: result.symbol.path,
                score: vector_norm,
                text_score: 0.0,
                vector_score,
                text_norm: 0.0,
                vector_norm,
                snippet: format!(
                    "{} {}",
                    result.symbol.symbol_name, result.symbol.symbol_kind
                ),
                line: Some(result.symbol.start_line as usize),
                chunk_start: Some(result.symbol.start_line),
                chunk_end: Some(result.symbol.end_line),
                result_id: Some(result.symbol.symbol_id),
            }
        })
        .collect()
}

fn hybrid_result_key(result: &HybridResult) -> String {
    result.result_id.clone().unwrap_or_else(|| {
        format!(
            "{}:{}:{}",
            result.path,
            result.line.unwrap_or(0),
            result.snippet
        )
    })
}

fn sort_hybrid_results(results: &mut [HybridResult]) {
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.vector_norm
                    .partial_cmp(&a.vector_norm)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                b.text_norm
                    .partial_cmp(&a.text_norm)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
    });
}

#[allow(clippy::too_many_arguments)]
fn index_search(
    query: &str,
    index_root: &Path,
    search_root: &Path,
    max_results: usize,
    context: usize,
    file_type: Option<&str>,
    compiled_glob: Option<&CompiledGlob>,
    compiled_exclude: Option<&CompiledGlob>,
    config_exclude_patterns: &[CompiledGlob],
    changed_filter: Option<&ChangedFiles>,
    fuzzy: bool,
) -> Result<SearchOutcome> {
    let candidates = collect_index_candidates(
        query,
        index_root,
        search_root,
        max_results,
        "file",
        file_type,
        compiled_glob,
        compiled_exclude,
        config_exclude_patterns,
        changed_filter,
        fuzzy,
    )?;

    let mut files_with_matches: HashSet<String> = HashSet::new();
    let mut results: Vec<SearchResult> = Vec::new();
    let mut context_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();

    for candidate in candidates {
        let (context_before, context_after) = context_for_line_cached(
            &candidate.full_path,
            candidate.line,
            context,
            &mut context_cache,
        );

        let display_path = candidate.display_path;
        files_with_matches.insert(display_path.clone());

        results.push(SearchResult {
            path: display_path,
            score: candidate.score,
            snippet: candidate.snippet,
            line: candidate.line,
            context_before,
            context_after,
            text_score: None,
            vector_score: None,
            hybrid_score: None,
            result_id: None,
            chunk_start: None,
            chunk_end: None,
        });
    }

    let total_matches = results.len();

    Ok(SearchOutcome {
        results,
        files_with_matches: files_with_matches.len(),
        total_matches,
        mode: IndexMode::Index,
        cache_hit: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn scan_search(
    query: &str,
    root: &std::path::Path,
    max_results: usize,
    context: usize,
    file_type: Option<&str>,
    compiled_glob: Option<&CompiledGlob>,
    compiled_exclude: Option<&CompiledGlob>,
    config_exclude_patterns: &[CompiledGlob],
    changed_filter: Option<&ChangedFiles>,
    regex: Option<&Regex>,
    case_sensitive: bool,
) -> Result<SearchOutcome> {
    if regex.is_none() && query.is_empty() {
        anyhow::bail!("Search query cannot be empty");
    }

    let query_lower = if !case_sensitive {
        query.to_lowercase()
    } else {
        String::new()
    };
    let scanner = FileScanner::new(root);
    let files = scanner.scan()?;

    let mut results: Vec<SearchResult> = Vec::new();
    let mut files_with_matches: HashSet<String> = HashSet::new();
    let mut total_matches = 0;

    'files: for file in files {
        let rel_path = file
            .path
            .strip_prefix(root)
            .unwrap_or(&file.path)
            .display()
            .to_string();
        if let Some(filter) = changed_filter {
            if !filter.matches_rel_path(&rel_path) {
                continue;
            }
        }

        if !matches_file_type(&rel_path, file_type) {
            continue;
        }
        if !matches_glob_compiled(&rel_path, compiled_glob) {
            continue;
        }
        if should_exclude_compiled(&rel_path, compiled_exclude) {
            continue;
        }
        if config_exclude_patterns
            .iter()
            .any(|p| should_exclude_compiled(&rel_path, Some(p)))
        {
            continue;
        }

        if context == 0 {
            for (idx, line) in file.content.lines().enumerate() {
                if results.len() >= max_results {
                    break 'files;
                }

                let matched = if let Some(re) = regex {
                    re.is_match(line)
                } else if case_sensitive {
                    line.contains(query)
                } else {
                    line.to_lowercase().contains(&query_lower)
                };

                if !matched {
                    continue;
                }

                files_with_matches.insert(rel_path.clone());
                total_matches += 1;

                let trimmed = line.trim();
                let snippet = if trimmed.len() <= 150 {
                    trimmed.to_string()
                } else {
                    format!("{}...", &trimmed[..150])
                };

                results.push(SearchResult {
                    path: rel_path.clone(),
                    score: 1.0,
                    snippet,
                    line: Some(idx + 1),
                    context_before: vec![],
                    context_after: vec![],
                    text_score: None,
                    vector_score: None,
                    hybrid_score: None,
                    result_id: None,
                    chunk_start: None,
                    chunk_end: None,
                });
            }
        } else {
            let lines: Vec<&str> = file.content.lines().collect();
            for (idx, line) in lines.iter().enumerate() {
                if results.len() >= max_results {
                    break 'files;
                }

                let matched = if let Some(re) = regex {
                    re.is_match(line)
                } else if case_sensitive {
                    line.contains(query)
                } else {
                    line.to_lowercase().contains(&query_lower)
                };

                if !matched {
                    continue;
                }

                files_with_matches.insert(rel_path.clone());
                total_matches += 1;

                let trimmed = line.trim();
                let snippet = if trimmed.len() <= 150 {
                    trimmed.to_string()
                } else {
                    format!("{}...", &trimmed[..150])
                };

                let (context_before, context_after) =
                    get_context_from_lines(&lines, idx + 1, context);

                results.push(SearchResult {
                    path: rel_path.clone(),
                    score: 1.0,
                    snippet,
                    line: Some(idx + 1),
                    context_before,
                    context_after,
                    text_score: None,
                    vector_score: None,
                    hybrid_score: None,
                    result_id: None,
                    chunk_start: None,
                    chunk_end: None,
                });
            }
        }
    }

    Ok(SearchOutcome {
        results,
        files_with_matches: files_with_matches.len(),
        total_matches,
        mode: IndexMode::Scan,
        cache_hit: false,
    })
}

/// Hybrid search combining BM25 with vector embeddings
#[allow(clippy::too_many_arguments)]
fn hybrid_search(
    query: &str,
    index_root: &Path,
    search_root: &Path,
    config: &Config,
    max_results: usize,
    context: usize,
    file_type: Option<&str>,
    glob_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
    compiled_glob: Option<&CompiledGlob>,
    compiled_exclude: Option<&CompiledGlob>,
    config_exclude_patterns: &[CompiledGlob],
    changed_filter: Option<&ChangedFiles>,
    mode: HybridSearchMode,
    use_cache: bool,
    cache_ttl_ms: u64,
) -> Result<SearchOutcome> {
    let index_path = index_root.join(INDEX_DIR);
    let embedding_db_path = index_root.join(".cgrep").join("embeddings.sqlite");
    let changed_component = changed_filter
        .map(|f| format!("{}:{}", f.rev(), f.signature()))
        .filter(|s| !s.is_empty());
    let candidate_k = config.search().candidate_k().max(max_results).max(1);
    let (weight_text, weight_vector) = normalized_hybrid_weights(
        config.search().weight_text(),
        config.search().weight_vector(),
    );
    let weight_text_milli = (weight_text * 1000.0).round() as i32;
    let weight_vector_milli = (weight_vector * 1000.0).round() as i32;
    let cache_mode = format!(
        "{}:k{}:wt{}:wv{}",
        mode, candidate_k, weight_text_milli, weight_vector_milli
    );

    // Build cache key
    let cache_key = CacheKey {
        query: normalize_query(query, true, true),
        mode: cache_mode,
        max_results,
        context,
        file_type: file_type.map(str::to_string),
        glob: glob_pattern.map(str::to_string),
        exclude: exclude_pattern.map(str::to_string),
        profile: None,
        index_hash: index_fingerprint(index_root),
        embedding_model: Some(config.embeddings.model().to_string()),
        search_root: Some(search_root.to_string_lossy().to_string()),
        changed: changed_component,
    };

    // Try cache
    if use_cache {
        if let Ok(cache) = SearchCache::new(index_root, cache_ttl_ms) {
            if let Ok(Some(entry)) = cache.get::<Vec<HybridResult>>(&cache_key) {
                // Return cached results
                let results: Vec<SearchResult> = entry
                    .data
                    .iter()
                    .filter_map(|hr| {
                        let full_path = resolve_full_path(&hr.path, index_root);
                        let display_path = scoped_display_path(&full_path, search_root)?;
                        Some(SearchResult {
                            path: display_path,
                            score: hr.score,
                            snippet: hr.snippet.clone(),
                            line: hr.line,
                            context_before: vec![],
                            context_after: vec![],
                            text_score: Some(hr.text_score),
                            vector_score: Some(hr.vector_score),
                            hybrid_score: Some(hr.score),
                            result_id: hr.result_id.clone(),
                            chunk_start: hr.chunk_start,
                            chunk_end: hr.chunk_end,
                        })
                    })
                    .collect();

                let files_with_matches = results
                    .iter()
                    .map(|r| r.path.clone())
                    .collect::<HashSet<_>>()
                    .len();
                let total_matches = results.len();

                return Ok(SearchOutcome {
                    results,
                    files_with_matches,
                    total_matches,
                    mode: IndexMode::Index,
                    cache_hit: true,
                });
            }
        }
    }

    // Open embedding storage if available
    let embedding_storage = if embedding_db_path.exists() {
        match EmbeddingStorage::open(&embedding_db_path) {
            Ok(storage) => match storage.is_symbol_unit() {
                Ok(true) => Some(storage),
                Ok(false) => {
                    eprintln!(
                        "Warning: embeddings DB schema mismatch (expected symbol-level). Using BM25 only."
                    );
                    None
                }
                Err(err) => {
                    eprintln!("Warning: failed to read embeddings metadata: {}", err);
                    None
                }
            },
            Err(err) => {
                eprintln!("Warning: failed to open embeddings DB: {}", err);
                None
            }
        }
    } else {
        None
    };

    // Get BM25 results first
    if !index_path.exists() {
        return Err(anyhow::anyhow!(
            "Index required for hybrid search. Run: cgrep index"
        ));
    }

    let bm25_candidates = collect_index_candidates(
        query,
        index_root,
        search_root,
        candidate_k,
        "symbol",
        file_type,
        compiled_glob,
        compiled_exclude,
        config_exclude_patterns,
        changed_filter,
        false,
    )?;

    // Convert to BM25Result format
    let bm25_results: Vec<BM25Result> = bm25_candidates
        .into_iter()
        .map(|candidate| BM25Result {
            path: candidate.stored_path,
            score: candidate.score,
            snippet: candidate.snippet,
            line: candidate.line,
            chunk_start: candidate
                .symbol_start
                .or_else(|| candidate.line.map(|l| l as u32)),
            chunk_end: candidate
                .symbol_end
                .or_else(|| candidate.line.map(|l| l as u32)),
            symbol_id: candidate.symbol_id,
        })
        .collect();

    // Create hybrid searcher
    let hybrid_config = HybridConfig::new(weight_text, weight_vector)
        .with_candidate_k(candidate_k)
        .with_max_results(candidate_k);
    let hybrid_searcher = HybridSearcher::new(hybrid_config);

    // Perform hybrid search based on mode
    let hybrid_results: Vec<HybridResult> = match mode {
        HybridSearchMode::Semantic | HybridSearchMode::Hybrid => {
            if let Some(ref storage) = embedding_storage {
                let provider_type = config.embeddings.provider();
                let provider_result: Result<Box<dyn EmbeddingProvider>> = match provider_type {
                    EmbeddingProviderType::Builtin => EmbeddingProviderConfig::from_env()
                        .and_then(FastEmbedder::new)
                        .map(|provider| Box::new(provider) as Box<dyn EmbeddingProvider>),
                    EmbeddingProviderType::Dummy => {
                        Ok(Box::new(DummyProvider::new(DEFAULT_EMBEDDING_DIM)))
                    }
                    EmbeddingProviderType::Command => Ok(Box::new(CommandProvider::new(
                        config.embeddings.command().to_string(),
                        config.embeddings.model().to_string(),
                    ))),
                };

                let query_embedding = match provider_result {
                    Ok(mut provider) => match provider.embed_one(query) {
                        Ok(query_embedding) => Some(query_embedding),
                        Err(err) => {
                            eprintln!("Warning: embedding query failed (using BM25 only): {}", err);
                            None
                        }
                    },
                    Err(err) => {
                        eprintln!("Warning: embedding provider unavailable: {}", err);
                        None
                    }
                };

                if let Some(query_embedding) = query_embedding {
                    match mode {
                        HybridSearchMode::Semantic => {
                            let mut semantic_results = hybrid_searcher
                                .semantic_search(bm25_results.clone(), &query_embedding, storage)
                                .unwrap_or_default();

                            if semantic_results.len() < max_results {
                                let mut seen: HashSet<String> =
                                    semantic_results.iter().map(hybrid_result_key).collect();
                                for extra in semantic_backfill_results(
                                    storage,
                                    &query_embedding,
                                    candidate_k,
                                ) {
                                    let key = hybrid_result_key(&extra);
                                    if seen.insert(key) {
                                        semantic_results.push(extra);
                                    }
                                    if semantic_results.len() >= candidate_k {
                                        break;
                                    }
                                }
                                sort_hybrid_results(&mut semantic_results);
                                semantic_results.truncate(candidate_k);
                            }

                            semantic_results
                        }
                        HybridSearchMode::Hybrid => hybrid_searcher
                            .rerank_with_embeddings(bm25_results, &query_embedding, storage)
                            .unwrap_or_default(),
                        HybridSearchMode::Keyword => Vec::new(),
                    }
                } else {
                    fallback_hybrid_results(&bm25_results)
                }
            } else {
                eprintln!("Warning: No embedding storage found. Using BM25 only.");
                fallback_hybrid_results(&bm25_results)
            }
        }
        HybridSearchMode::Keyword => {
            // Should not reach here
            fallback_hybrid_results(&bm25_results)
        }
    };

    // Convert to SearchResult with context
    let mut results: Vec<SearchResult> = Vec::with_capacity(max_results.min(hybrid_results.len()));
    let mut filtered_hybrid_results: Vec<HybridResult> = Vec::with_capacity(max_results);
    let mut files_with_matches: HashSet<String> = HashSet::new();
    let mut context_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();

    for hr in hybrid_results.iter() {
        if results.len() >= max_results {
            break;
        }

        let full_path = resolve_full_path(&hr.path, index_root);
        let Some(display_path) = scoped_display_path(&full_path, search_root) else {
            continue;
        };

        // Apply filters
        if !matches_file_type(&display_path, file_type) {
            continue;
        }
        if !matches_glob_compiled(&display_path, compiled_glob) {
            continue;
        }
        if should_exclude_compiled(&display_path, compiled_exclude) {
            continue;
        }
        if config_exclude_patterns
            .iter()
            .any(|p| should_exclude_compiled(&display_path, Some(p)))
        {
            continue;
        }

        files_with_matches.insert(display_path.clone());
        filtered_hybrid_results.push(hr.clone());

        // Get context lines if needed
        let (context_before, context_after) =
            context_for_line_cached(&full_path, hr.line, context, &mut context_cache);

        results.push(SearchResult {
            path: display_path,
            score: hr.score,
            snippet: hr.snippet.clone(),
            line: hr.line,
            context_before,
            context_after,
            text_score: Some(hr.text_score),
            vector_score: Some(hr.vector_score),
            hybrid_score: Some(hr.score),
            result_id: hr.result_id.clone(),
            chunk_start: hr.chunk_start,
            chunk_end: hr.chunk_end,
        });
    }

    // Store in cache
    if use_cache {
        if let Ok(cache) = SearchCache::new(index_root, cache_ttl_ms) {
            let _ = cache.put(&cache_key, filtered_hybrid_results);
        }
    }

    let total_matches = results.len();
    let files_count = files_with_matches.len();

    Ok(SearchOutcome {
        results,
        files_with_matches: files_count,
        total_matches,
        mode: IndexMode::Index,
        cache_hit: false,
    })
}

fn get_context_from_lines(
    lines: &[&str],
    line_num: usize,
    context: usize,
) -> (Vec<String>, Vec<String>) {
    if lines.is_empty() {
        return (vec![], vec![]);
    }
    let idx = line_num.saturating_sub(1);
    let start = idx.saturating_sub(context);
    let end = (idx + context + 1).min(lines.len());

    let before = lines[start..idx].iter().map(|l| (*l).to_string()).collect();
    let after = if idx + 1 < end {
        lines[idx + 1..end]
            .iter()
            .map(|l| (*l).to_string())
            .collect()
    } else {
        vec![]
    };

    (before, after)
}

fn highlight_matches_regex(text: &str, re: &Regex, use_color: bool) -> String {
    if !use_color {
        return text.to_string();
    }
    re.replace_all(text, |caps: &regex::Captures| {
        colorize_match(&caps[0], true)
    })
    .to_string()
}

/// Highlight query matches in text
fn highlight_matches(text: &str, query: &str, use_color: bool) -> String {
    if !use_color {
        return text.to_string();
    }

    let terms: Vec<&str> = query.split_whitespace().collect();
    let mut result = text.to_string();

    for term in terms {
        let re = regex::RegexBuilder::new(&regex::escape(term))
            .case_insensitive(true)
            .build();

        if let Ok(re) = re {
            result = re
                .replace_all(&result, |caps: &regex::Captures| {
                    colorize_match(&caps[0], true)
                })
                .to_string();
        }
    }

    result
}

/// Find a relevant snippet containing the query terms, also returning line number
fn find_snippet_with_line(content: &str, query: &str, max_len: usize) -> (String, Option<usize>) {
    let query_lower = query.to_lowercase();
    let mut terms: Vec<&str> = query_lower.split_whitespace().collect();
    terms.sort_unstable();
    terms.dedup();

    let mut best_match: Option<(usize, usize, usize, usize, String)> = None;
    for (line_idx, line) in content.lines().enumerate() {
        if terms.is_empty() {
            break;
        }
        let line_lower = line.to_lowercase();
        let mut matched_terms = 0usize;
        let mut hit_count = 0usize;
        for term in &terms {
            if line_lower.contains(term) {
                matched_terms += 1;
                hit_count += line_lower.match_indices(term).count();
            }
        }
        if matched_terms == 0 {
            continue;
        }

        let trimmed = line.trim();
        let line_len = char_count(trimmed);
        let line_num = line_idx + 1;
        let should_replace = match best_match.as_ref() {
            None => true,
            Some((best_terms, best_hits, best_len, best_line, _)) => {
                matched_terms > *best_terms
                    || (matched_terms == *best_terms && hit_count > *best_hits)
                    || (matched_terms == *best_terms
                        && hit_count == *best_hits
                        && line_len < *best_len)
                    || (matched_terms == *best_terms
                        && hit_count == *best_hits
                        && line_len == *best_len
                        && line_num < *best_line)
            }
        };
        if should_replace {
            best_match = Some((
                matched_terms,
                hit_count,
                line_len,
                line_num,
                trimmed.to_string(),
            ));
        }
    }

    if let Some((_, _, _, line_num, line_text)) = best_match {
        return (truncate_with_ellipsis(&line_text, max_len), Some(line_num));
    }

    // Return first non-empty line if no match
    let snippet = content
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| truncate_with_ellipsis(l.trim(), max_len))
        .unwrap_or_default();

    (snippet, None)
}

fn find_snippets_with_lines(content: &str, query: &str, max_len: usize) -> Vec<(String, usize)> {
    let query_lower = query.to_lowercase();
    let mut terms: Vec<&str> = query_lower.split_whitespace().collect();
    terms.sort_unstable();
    terms.dedup();

    if terms.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let line_lower = line.to_lowercase();
        if !terms.iter().any(|term| line_lower.contains(term)) {
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        matches.push((truncate_with_ellipsis(trimmed, max_len), line_idx + 1));
    }

    matches
}

fn build_search_scope_query(
    path_exact_field: tantivy::schema::Field,
    search_root: &Path,
    index_root: &Path,
) -> Option<Box<dyn tantivy::query::Query>> {
    let search_root = search_root
        .canonicalize()
        .unwrap_or_else(|_| search_root.to_path_buf());
    let index_root = index_root
        .canonicalize()
        .unwrap_or_else(|_| index_root.to_path_buf());
    if search_root == index_root || !search_root.starts_with(&index_root) {
        return None;
    }

    let mut scope_queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();

    if search_root.is_file() {
        let absolute = search_root.to_string_lossy().to_string();
        let term = Term::from_field_text(path_exact_field, &absolute);
        scope_queries.push((
            Occur::Should,
            Box::new(TermQuery::new(
                term,
                tantivy::schema::IndexRecordOption::Basic,
            )),
        ));
    } else {
        let mut absolute_prefix = search_root.to_string_lossy().to_string();
        if !absolute_prefix.ends_with(std::path::MAIN_SEPARATOR) {
            absolute_prefix.push(std::path::MAIN_SEPARATOR);
        }
        let pattern = format!("{}.*", regex::escape(&absolute_prefix));
        if let Ok(query) = RegexQuery::from_pattern(&pattern, path_exact_field) {
            scope_queries.push((Occur::Should, Box::new(query)));
        }
    }

    let rel_scope = search_root
        .strip_prefix(&index_root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    if !rel_scope.is_empty() {
        if search_root.is_file() {
            for candidate in [rel_scope.clone(), format!("./{rel_scope}")] {
                let term = Term::from_field_text(path_exact_field, &candidate);
                scope_queries.push((
                    Occur::Should,
                    Box::new(TermQuery::new(
                        term,
                        tantivy::schema::IndexRecordOption::Basic,
                    )),
                ));
            }
        } else {
            let mut rel_prefix = rel_scope.clone();
            if !rel_prefix.ends_with('/') {
                rel_prefix.push('/');
            }
            for prefix in [rel_prefix.clone(), format!("./{rel_prefix}")] {
                let pattern = format!("{}.*", regex::escape(&prefix));
                if let Ok(query) = RegexQuery::from_pattern(&pattern, path_exact_field) {
                    scope_queries.push((Occur::Should, Box::new(query)));
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

fn resolve_search_root(path: Option<&str>) -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("Cannot determine current directory")?;
    let requested = path.map(PathBuf::from).unwrap_or_else(|| cwd.clone());
    let absolute = if requested.is_absolute() {
        requested
    } else {
        cwd.join(requested)
    };
    Ok(normalize_path(&absolute))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut cleaned = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                cleaned.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                cleaned.push(component.as_os_str());
            }
        }
    }

    if cleaned.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        cleaned
    }
}

fn resolve_full_path(path_value: &str, index_root: &Path) -> PathBuf {
    let path = Path::new(path_value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        index_root.join(path)
    }
}

fn scoped_display_path(full_path: &Path, search_root: &Path) -> Option<String> {
    full_path
        .strip_prefix(search_root)
        .ok()
        .map(|rel| rel.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer::index::DEFAULT_WRITER_BUDGET_BYTES;
    use crate::indexer::IndexBuilder;
    use cgrep::embedding::SymbolEmbeddingInput;
    use tempfile::TempDir;

    #[test]
    fn scan_search_plain_text_case_insensitive() {
        let dir = TempDir::new().expect("tempdir");
        let file_path = dir.path().join("sample.txt");
        std::fs::write(&file_path, "Hello World\nSecond line").expect("write");

        let outcome = scan_search(
            "world",
            dir.path(),
            10,
            0,
            None,
            None,
            None,
            &[],
            None,
            None,
            false,
        )
        .expect("scan");

        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "sample.txt");
        assert_eq!(outcome.results[0].line, Some(1));
    }

    #[test]
    fn scan_search_regex_match() {
        let dir = TempDir::new().expect("tempdir");
        let file_path = dir.path().join("numbers.txt");
        std::fs::write(&file_path, "abc123\nnope\nxyz456").expect("write");

        let re = Regex::new(r"\d{3}").expect("regex");
        let outcome = scan_search(
            r"\d{3}",
            dir.path(),
            10,
            0,
            None,
            None,
            None,
            &[],
            None,
            Some(&re),
            true,
        )
        .expect("scan");

        assert_eq!(outcome.results.len(), 2);
        assert_eq!(outcome.results[0].path, "numbers.txt");
        assert_eq!(outcome.results[0].line, Some(1));
        assert_eq!(outcome.results[1].line, Some(3));
    }

    #[test]
    fn find_snippet_with_line_prefers_high_term_coverage() {
        let content = "foo only\nfoo bar matched\nbar only\n";
        let (snippet, line) = find_snippet_with_line(content, "foo bar", 120);
        assert_eq!(line, Some(2));
        assert_eq!(snippet, "foo bar matched");
    }

    #[test]
    fn find_snippet_with_line_truncates_on_char_boundaries() {
        let content = "한글테스트라인";
        let (snippet, _) = find_snippet_with_line(content, "없음", 5);
        assert_eq!(snippet.chars().count(), 5);
    }

    #[test]
    fn index_search_scopes_to_search_root_and_relativizes_paths() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();
        let root_file = root.join("root.rs");
        let subdir = root.join("src");
        let sub_file = subdir.join("sub.rs");

        std::fs::create_dir_all(&subdir).expect("create subdir");
        std::fs::write(&root_file, "needle in root").expect("write root");
        std::fs::write(&sub_file, "needle in sub").expect("write sub");

        let builder = IndexBuilder::new(root).expect("builder");
        builder
            .build(false, DEFAULT_WRITER_BUDGET_BYTES)
            .expect("build");

        let outcome = index_search(
            "needle",
            root,
            &subdir,
            10,
            0,
            None,
            None,
            None,
            &[],
            None,
            false,
        )
        .expect("index search");

        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "sub.rs");
    }

    #[test]
    fn index_search_scope_filter_applies_before_top_docs() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();
        let scoped = root.join("scoped");
        std::fs::create_dir_all(&scoped).expect("create scoped");

        for i in 0..20 {
            let outside = root.join(format!("outside_{i}.txt"));
            std::fs::write(&outside, format!("{}\n", "needle ".repeat(200)))
                .expect("write outside");
        }
        let scoped_file = scoped.join("target.txt");
        std::fs::write(&scoped_file, "needle only in scoped file\n").expect("write scoped");

        let builder = IndexBuilder::new(root).expect("builder");
        builder
            .build(false, DEFAULT_WRITER_BUDGET_BYTES)
            .expect("build");

        let outcome = index_search(
            "needle",
            root,
            &scoped,
            1,
            0,
            None,
            None,
            None,
            &[],
            None,
            false,
        )
        .expect("index search");

        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "target.txt");
    }

    #[test]
    fn index_search_returns_multiple_matches_from_single_chunk() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();
        let file = root.join("sample.py");
        std::fs::write(
            &file,
            "def cpu_fallback_path(target):\n    return target\ncpu_fallback_path(1)\n",
        )
        .expect("write sample");

        let builder = IndexBuilder::new(root).expect("builder");
        builder
            .build(false, DEFAULT_WRITER_BUDGET_BYTES)
            .expect("build");

        let outcome = index_search(
            "cpu_fallback_path",
            root,
            root,
            10,
            0,
            None,
            None,
            None,
            &[],
            None,
            false,
        )
        .expect("index search");

        let lines: Vec<usize> = outcome.results.iter().filter_map(|r| r.line).collect();
        assert!(lines.contains(&1));
        assert!(lines.contains(&3));
    }

    #[test]
    fn context_pack_trims_overlapping_context() {
        let mut results = vec![
            SearchResult {
                path: "src/lib.rs".to_string(),
                score: 1.0,
                snippet: "fn alpha() {}".to_string(),
                line: Some(10),
                context_before: vec!["line 8".to_string(), "line 9".to_string()],
                context_after: vec!["line 11".to_string(), "line 12".to_string()],
                text_score: None,
                vector_score: None,
                hybrid_score: None,
                result_id: None,
                chunk_start: None,
                chunk_end: None,
            },
            SearchResult {
                path: "src/lib.rs".to_string(),
                score: 0.9,
                snippet: "fn beta() {}".to_string(),
                line: Some(11),
                context_before: vec!["line 9".to_string(), "line 10".to_string()],
                context_after: vec!["line 12".to_string(), "line 13".to_string()],
                text_score: None,
                vector_score: None,
                hybrid_score: None,
                result_id: None,
                chunk_start: None,
                chunk_end: None,
            },
        ];

        apply_context_pack(&mut results, 0);

        assert!(results[1].context_before.is_empty());
        assert_eq!(results[1].context_after, vec!["line 12", "line 13"]);
    }

    #[test]
    fn stable_result_id_is_deterministic() {
        let result = SearchResult {
            path: "src/lib.rs".to_string(),
            score: 1.0,
            snippet: "fn alpha() {}".to_string(),
            line: Some(10),
            context_before: vec![],
            context_after: vec![],
            text_score: None,
            vector_score: None,
            hybrid_score: None,
            result_id: None,
            chunk_start: None,
            chunk_end: None,
        };

        let a = stable_result_id(&result);
        let b = stable_result_id(&result);
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    fn sample_result(path: &str, line: usize, snippet: &str) -> SearchResult {
        SearchResult {
            path: path.to_string(),
            score: 1.0,
            snippet: snippet.to_string(),
            line: Some(line),
            context_before: vec!["before one".to_string(), "before two".to_string()],
            context_after: vec!["after one".to_string(), "after two".to_string()],
            text_score: None,
            vector_score: None,
            hybrid_score: None,
            result_id: None,
            chunk_start: None,
            chunk_end: None,
        }
    }

    #[test]
    fn budget_truncates_snippet_chars() {
        let mut results = vec![sample_result("a.rs", 1, "0123456789abcdef")];
        let stats = apply_output_budget(
            &mut results,
            SearchOutputBudget {
                max_chars_per_snippet: Some(8),
                max_total_chars: None,
                max_context_chars: None,
                dedupe_context: false,
                suppress_boilerplate: false,
            },
        );

        assert!(stats.truncated);
        assert_eq!(results[0].snippet, "01234...");
    }

    #[test]
    fn budget_truncates_context_chars() {
        let mut results = vec![sample_result("a.rs", 1, "short")];
        let stats = apply_output_budget(
            &mut results,
            SearchOutputBudget {
                max_chars_per_snippet: None,
                max_total_chars: None,
                max_context_chars: Some(6),
                dedupe_context: false,
                suppress_boilerplate: false,
            },
        );

        assert!(stats.truncated);
        let context_total: usize = results[0]
            .context_before
            .iter()
            .chain(results[0].context_after.iter())
            .map(|line| line.chars().count())
            .sum();
        assert!(context_total <= 6);
    }

    #[test]
    fn budget_max_total_chars_drops_tail_results() {
        let mut results = vec![
            sample_result("a.rs", 1, "alpha"),
            sample_result("b.rs", 2, "beta"),
            sample_result("c.rs", 3, "gamma"),
        ];
        let stats = apply_output_budget(
            &mut results,
            SearchOutputBudget {
                max_chars_per_snippet: None,
                max_total_chars: Some(25),
                max_context_chars: None,
                dedupe_context: false,
                suppress_boilerplate: false,
            },
        );

        assert!(stats.truncated);
        assert!(stats.dropped_results >= 1);
        assert_eq!(results.len(), 3 - stats.dropped_results);
    }

    #[test]
    fn budget_dedupes_context_lines_per_path() {
        let mut first = sample_result("same.rs", 1, "a");
        first.context_before = vec!["shared".to_string(), "unique-1".to_string()];
        first.context_after = vec!["shared-after".to_string()];
        let mut second = sample_result("same.rs", 2, "b");
        second.context_before = vec!["shared".to_string(), "unique-2".to_string()];
        second.context_after = vec!["shared-after".to_string(), "tail".to_string()];
        let mut results = vec![first, second];

        apply_output_budget(
            &mut results,
            SearchOutputBudget {
                max_chars_per_snippet: None,
                max_total_chars: None,
                max_context_chars: None,
                dedupe_context: true,
                suppress_boilerplate: false,
            },
        );

        assert_eq!(results[1].context_before, vec!["unique-2"]);
        assert_eq!(results[1].context_after, vec!["tail"]);
    }

    #[test]
    fn normalized_hybrid_weights_handle_invalid_inputs() {
        let (wt, wv) = normalized_hybrid_weights(2.0, 1.0);
        assert!((wt - (2.0 / 3.0)).abs() < 0.001);
        assert!((wv - (1.0 / 3.0)).abs() < 0.001);

        let (wt, wv) = normalized_hybrid_weights(-10.0, -5.0);
        assert!((wt - 0.7).abs() < 0.001);
        assert!((wv - 0.3).abs() < 0.001);
    }

    #[test]
    fn fallback_hybrid_results_normalize_bm25_scores() {
        let bm25 = vec![
            BM25Result {
                path: "a.rs".to_string(),
                score: 10.0,
                snippet: "a".to_string(),
                line: Some(1),
                chunk_start: Some(1),
                chunk_end: Some(1),
                symbol_id: Some("a".to_string()),
            },
            BM25Result {
                path: "b.rs".to_string(),
                score: 5.0,
                snippet: "b".to_string(),
                line: Some(2),
                chunk_start: Some(2),
                chunk_end: Some(2),
                symbol_id: Some("b".to_string()),
            },
        ];

        let results = fallback_hybrid_results(&bm25);
        assert_eq!(results.len(), 2);
        assert!((results[0].score - 1.0).abs() < 0.001);
        assert!((results[1].score - 0.5).abs() < 0.001);
    }

    #[test]
    fn semantic_backfill_results_uses_vector_similarity() {
        let dir = TempDir::new().expect("tempdir");
        let db_path = dir.path().join("embeddings.sqlite");
        let mut storage = EmbeddingStorage::open(&db_path).expect("open storage");

        let emb_a = vec![1.0, 0.0, 0.0];
        let emb_b = vec![0.0, 1.0, 0.0];
        let input_a = SymbolEmbeddingInput {
            symbol_id: "sym_a",
            lang: "rust",
            symbol_kind: "function",
            symbol_name: "alpha",
            start_line: 5,
            end_line: 8,
            content_hash: "h1",
            embedding: &emb_a,
        };
        let input_b = SymbolEmbeddingInput {
            symbol_id: "sym_b",
            lang: "rust",
            symbol_kind: "function",
            symbol_name: "beta",
            start_line: 15,
            end_line: 18,
            content_hash: "h2",
            embedding: &emb_b,
        };

        storage
            .replace_file_symbols("src/lib.rs", "hash", 0, &[input_a, input_b])
            .expect("insert");

        let query_embedding = vec![1.0, 0.0, 0.0];
        let backfill = semantic_backfill_results(&storage, &query_embedding, 2);
        assert_eq!(backfill.len(), 2);
        assert_eq!(backfill[0].result_id.as_deref(), Some("sym_a"));
        assert!(backfill[0].score >= backfill[1].score);
    }
}
