// SPDX-License-Identifier: MIT OR Apache-2.0

//! Full-text search with BM25 ranking using tantivy

use anyhow::{Context, Result};
use colored::Colorize;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;
use tantivy::{
    collector::TopDocs,
    query::{BooleanQuery, FuzzyTermQuery, Occur, QueryParser, TermQuery},
    schema::{Term, Value},
    Index, TantivyDocument,
};

use crate::cli::OutputFormat;
use crate::indexer::reuse;
use crate::indexer::scanner::FileScanner;
use crate::query::changed_files::ChangedFiles;
use crate::query::scope_query::build_scope_path_query;
use cgrep::cache::{CacheKey, SearchCache};
use cgrep::config::{Config, EmbeddingProviderType, RankingConfig};
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
    /// Keyword ranking component breakdown (only with --explain)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explain: Option<ScoreExplain>,
}

/// Deterministic keyword ranking breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreExplain {
    pub bm25: f32,
    pub path_boost: f32,
    pub symbol_boost: f32,
    pub changed_boost: f32,
    pub kind_boost: f32,
    pub penalties: f32,
    pub final_score: f32,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryClass {
    IdentifierLike,
    PhraseLike,
}

#[derive(Debug, Clone, Copy)]
struct RankingWeights {
    path_weight: f32,
    symbol_weight: f32,
    language_weight: f32,
    changed_weight: f32,
    kind_weight: f32,
    weak_signal_penalty: f32,
}

#[derive(Debug, Clone)]
struct RankingStrategy {
    enabled: bool,
    explain: bool,
    explain_top_k: usize,
    query_class: QueryClass,
    query_tokens: Vec<String>,
    identifier_query: Option<String>,
    language_filter: Option<String>,
    changed_requested: bool,
    weights: RankingWeights,
}

impl RankingStrategy {
    fn from_config(
        config: &RankingConfig,
        query: &str,
        file_type: Option<&str>,
        changed_filter: Option<&ChangedFiles>,
        explain: bool,
    ) -> Self {
        Self {
            enabled: config.enabled(),
            explain,
            explain_top_k: config.explain_top_k(),
            query_class: classify_query(query),
            query_tokens: query_tokens_for_ranking(query),
            identifier_query: single_identifier_query(query),
            language_filter: file_type.map(|value| value.to_ascii_lowercase()),
            changed_requested: changed_filter.is_some(),
            weights: RankingWeights {
                path_weight: config.path_weight(),
                symbol_weight: config.symbol_weight(),
                language_weight: config.language_weight(),
                changed_weight: config.changed_weight(),
                kind_weight: config.kind_weight(),
                weak_signal_penalty: config.weak_signal_penalty(),
            },
        }
    }

    fn cache_mode_suffix(&self) -> String {
        format!(
            "rk{}:qc{}:ex{}",
            usize::from(self.enabled),
            match self.query_class {
                QueryClass::IdentifierLike => "id",
                QueryClass::PhraseLike => "ph",
            },
            usize::from(self.explain)
        )
    }
}

fn legacy_ranking_strategy(
    query: &str,
    file_type: Option<&str>,
    changed_filter: Option<&ChangedFiles>,
) -> RankingStrategy {
    RankingStrategy::from_config(
        &RankingConfig::default(),
        query,
        file_type,
        changed_filter,
        false,
    )
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
    confidence: f32,
    fallback_chain: Vec<String>,
    bootstrap_index: bool,
    payload_chars: usize,
    payload_tokens_estimate: usize,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    explain: Option<ScoreExplain>,
}

impl SearchJson2Result {
    fn from_result(
        result: &SearchResult,
        include_context: bool,
        include_explain: bool,
        path_value: Option<&str>,
    ) -> Self {
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
            explain: if include_explain {
                result.explain.clone()
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
    recursive: bool,
    no_ignore: bool,
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
    persist_agent_hints: bool,
    explicit_mode: bool,
    bootstrap_index: bool,
    explain: bool,
) -> Result<()> {
    let start_time = Instant::now();
    let use_color = use_colors() && format == OutputFormat::Text;

    if query.trim().is_empty() {
        anyhow::bail!("Search query cannot be empty");
    }

    // Precompile glob patterns for efficient repeated matching
    let compiled_glob = glob_pattern.and_then(CompiledGlob::new);
    let compiled_exclude = exclude_pattern.and_then(CompiledGlob::new);

    let workspace_root =
        normalize_path(&std::env::current_dir().context("Cannot determine current directory")?);
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

    let requested_mode = if no_index || regex || no_ignore {
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
    let mut effective_search_mode = search_mode.unwrap_or(HybridSearchMode::Keyword);
    if no_ignore
        && matches!(
            effective_search_mode,
            HybridSearchMode::Semantic | HybridSearchMode::Hybrid
        )
    {
        eprintln!(
            "Warning: --no-ignore is only supported for keyword search; falling back to --mode keyword."
        );
        effective_search_mode = HybridSearchMode::Keyword;
    }
    let effective_cache_ttl = cache_ttl.unwrap_or(DEFAULT_CACHE_TTL_MS);

    let explain_keyword = explain && effective_search_mode == HybridSearchMode::Keyword;
    if explain && !explain_keyword {
        eprintln!("Warning: --explain is currently supported for --mode keyword only; ignoring.");
    }
    let ranking_strategy = RankingStrategy::from_config(
        config.ranking(),
        query,
        file_type,
        changed_filter.as_ref(),
        explain_keyword,
    );

    let mut outcome = match effective_search_mode {
        HybridSearchMode::Semantic | HybridSearchMode::Hybrid => {
            // Use hybrid search
            hybrid_search(
                query,
                &index_root,
                &search_root,
                &workspace_root,
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
                recursive,
                use_cache,
                effective_cache_ttl,
            )?
        }
        HybridSearchMode::Keyword => keyword_search(
            query,
            &index_root,
            &search_root,
            &workspace_root,
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
            recursive,
            no_ignore,
            use_cache,
            effective_cache_ttl,
            &ranking_strategy,
        )?,
    };
    let mut confidence = estimate_confidence(&outcome.results, effective_search_mode);
    let mut fallback_chain = vec![format!(
        "{}:{}",
        effective_search_mode,
        match outcome.mode {
            IndexMode::Index => "index",
            IndexMode::Scan => "scan",
        }
    )];

    let fallback_policy = KeywordFallbackPolicy {
        mode: effective_search_mode,
        explicit_mode,
        requested_mode,
        no_ignore,
        fuzzy,
        has_regex: compiled_regex.is_some(),
        confidence,
        results: &outcome.results,
    };
    if should_attempt_keyword_fallback(&fallback_policy) {
        match hybrid_search(
            query,
            &index_root,
            &search_root,
            &workspace_root,
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
            HybridSearchMode::Hybrid,
            recursive,
            use_cache,
            effective_cache_ttl,
        ) {
            Ok(hybrid_outcome) => {
                let hybrid_confidence =
                    estimate_confidence(&hybrid_outcome.results, HybridSearchMode::Hybrid);
                let should_replace = hybrid_outcome.results.len() > outcome.results.len()
                    || hybrid_confidence > confidence + 0.08;
                fallback_chain.push("hybrid:attempted".to_string());
                if should_replace {
                    outcome = hybrid_outcome;
                    effective_search_mode = HybridSearchMode::Hybrid;
                    confidence = hybrid_confidence;
                    fallback_chain.push("hybrid:selected".to_string());
                } else {
                    fallback_chain.push("hybrid:discarded".to_string());
                }
            }
            Err(_) => {
                fallback_chain.push("hybrid:unavailable".to_string());
            }
        }
    }

    if using_parent && outcome.mode == IndexMode::Index {
        eprintln!("Using index from: {}", index_root.display());
    }

    let effective_context_pack = context_pack.filter(|v| *v > 0);
    if let Some(pack_gap) = effective_context_pack {
        apply_context_pack(&mut outcome.results, pack_gap);
    }

    ensure_result_ids(&mut outcome.results);

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
            if persist_agent_hints {
                let hint_inputs: Vec<crate::query::agent::AgentHintInput> = outcome
                    .results
                    .iter()
                    .filter_map(|result| {
                        result.line.map(|line| crate::query::agent::AgentHintInput {
                            id: result.result_id.clone(),
                            path: normalize_hint_path(&result.path, &search_root, &workspace_root),
                            id_path: Some(result.path.clone()),
                            line,
                            snippet: result.snippet.clone(),
                        })
                    })
                    .collect();
                if !hint_inputs.is_empty() {
                    let _ = crate::query::agent::persist_expand_hints(&search_root, hint_inputs);
                }
            }

            let json2_results: Vec<SearchJson2Result> = outcome
                .results
                .iter()
                .map(|result| {
                    let alias = path_alias_lookup
                        .as_ref()
                        .and_then(|lookup| lookup.get(&result.path))
                        .map(|s| s.as_str());
                    SearchJson2Result::from_result(result, !compact, explain_keyword, alias)
                })
                .collect();
            let payload_chars = estimate_json2_payload_chars(&json2_results);
            let payload_tokens_estimate = estimate_tokens_from_chars(payload_chars);

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
                    confidence,
                    fallback_chain: fallback_chain.clone(),
                    bootstrap_index,
                    payload_chars,
                    payload_tokens_estimate,
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

                    if explain_keyword {
                        if let Some(explain) = &result.explain {
                            println!(
                                "    [score] bm25={:.4} path={:.4} symbol={:.4} changed={:.4} kind={:.4} penalties={:.4} final={:.4}",
                                explain.bm25,
                                explain.path_boost,
                                explain.symbol_boost,
                                explain.changed_boost,
                                explain.kind_boost,
                                explain.penalties,
                                explain.final_score
                            );
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

fn ensure_result_ids(results: &mut [SearchResult]) {
    for result in results.iter_mut() {
        if result.result_id.is_none() {
            result.result_id = Some(stable_result_id(result));
        }
    }
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

#[derive(Debug, Deserialize, Default)]
struct ReuseIndexMetadata {
    #[serde(default)]
    files: HashMap<String, ReuseFileMetadata>,
}

#[derive(Debug, Deserialize, Default)]
struct ReuseFileMetadata {
    #[serde(default)]
    hash: String,
}

#[derive(Debug, Clone)]
struct ReuseStaleFilter {
    hashes: HashMap<String, String>,
}

fn reuse_stale_filter_active(index_root: &Path) -> bool {
    reuse::load_runtime_state(index_root)
        .map(|state| state.active)
        .unwrap_or(false)
}

fn load_reuse_stale_filter(index_root: &Path) -> Option<ReuseStaleFilter> {
    if !reuse_stale_filter_active(index_root) {
        return None;
    }

    let metadata_path = index_root.join(INDEX_DIR).join("metadata.json");
    let raw = fs::read_to_string(metadata_path).ok()?;
    let metadata: ReuseIndexMetadata = serde_json::from_str(&raw).ok()?;
    let hashes = metadata
        .files
        .into_iter()
        .filter_map(|(path, file)| {
            if file.hash.is_empty() {
                None
            } else {
                Some((path, file.hash))
            }
        })
        .collect::<HashMap<_, _>>();

    Some(ReuseStaleFilter { hashes })
}

fn hash_file_streaming(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buf).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Some(hasher.finalize().to_hex().to_string())
}

fn candidate_is_fresh(
    candidate: &IndexCandidate,
    stale_filter: &ReuseStaleFilter,
    hash_cache: &mut HashMap<PathBuf, Option<String>>,
) -> bool {
    if !candidate.full_path.is_file() {
        return false;
    }
    let Some(expected_hash) = stale_filter.hashes.get(&candidate.stored_path) else {
        return false;
    };
    let actual_hash = hash_cache
        .entry(candidate.full_path.clone())
        .or_insert_with(|| hash_file_streaming(&candidate.full_path));
    actual_hash
        .as_ref()
        .map(|hash| hash == expected_hash)
        .unwrap_or(false)
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
    explain: Option<ScoreExplain>,
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
    workspace_root: &Path,
    max_candidates: usize,
    doc_type: &str,
    file_type: Option<&str>,
    compiled_glob: Option<&CompiledGlob>,
    compiled_exclude: Option<&CompiledGlob>,
    config_exclude_patterns: &[CompiledGlob],
    changed_filter: Option<&ChangedFiles>,
    recursive: bool,
    fuzzy: bool,
    case_sensitive: bool,
    ranking_strategy: &RankingStrategy,
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
    let language_field = schema
        .get_field("language")
        .context("Missing language field")?;
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

    let literal_query = !fuzzy && query_requires_literal_handling(query);
    let query_for_parser = if literal_query {
        escape_as_query_phrase(query)
    } else {
        query.to_string()
    };

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
        let (parsed_query, _errors) = query_parser.parse_query_lenient(&query_for_parser);
        parsed_query
    };

    let doc_type_term = Term::from_field_text(doc_type_field, doc_type);
    let doc_type_query = TermQuery::new(doc_type_term, tantivy::schema::IndexRecordOption::Basic);
    let mut clauses: Vec<(Occur, Box<dyn tantivy::query::Query>)> = vec![
        (Occur::Must, text_query),
        (Occur::Must, Box::new(doc_type_query)),
    ];
    if let Some(scope_query) =
        path_exact_field.and_then(|f| build_scope_path_query(f, search_root, index_root))
    {
        clauses.push((Occur::Must, scope_query));
    }
    let parsed_query: Box<dyn tantivy::query::Query> = Box::new(BooleanQuery::new(clauses));

    let fetch_limit = max_candidates.saturating_mul(5).max(1);
    let top_docs = searcher.search(&parsed_query, &TopDocs::with_limit(fetch_limit))?;

    let mut candidates: Vec<IndexCandidate> = Vec::new();
    let mut per_path_counts: HashMap<String, usize> = HashMap::new();

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
        let Some(scope_path) = scope_relative_path(&full_path, search_root) else {
            continue;
        };
        let current_path_count = per_path_counts.get(&scope_path).copied().unwrap_or(0);
        if current_path_count >= MAX_INITIAL_RESULTS_PER_PATH {
            continue;
        }
        let display_path = workspace_display_path(&full_path, workspace_root);
        if !recursive && Path::new(&scope_path).components().count() > 1 {
            continue;
        }
        if let Some(filter) = changed_filter {
            if !filter.matches_rel_path(&scope_path) {
                continue;
            }
        }

        if !matches_file_type(&scope_path, file_type) {
            continue;
        }
        if !matches_glob_compiled(&scope_path, compiled_glob) {
            continue;
        }
        if should_exclude_compiled(&scope_path, compiled_exclude) {
            continue;
        }
        if config_exclude_patterns
            .iter()
            .any(|p| should_exclude_compiled(&scope_path, Some(p)))
        {
            continue;
        }

        let content_value = doc
            .get_first(content_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let symbols_value = doc
            .get_first(symbols_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let enforce_literal_filter = literal_query || (case_sensitive && !fuzzy);
        if enforce_literal_filter
            && !matches_literal_query(
                content_value,
                symbols_value,
                path_value,
                query,
                case_sensitive,
            )
        {
            continue;
        }

        let line_offset = doc
            .get_first(line_offset_field)
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let doc_type_value = doc
            .get_first(doc_type_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let language_value = doc
            .get_first(language_field)
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let symbol_kind = if doc_type_value == "symbol" {
            infer_symbol_kind_from_content(content_value)
        } else {
            None
        };
        let score_components = compute_keyword_score_components(
            *score,
            &scope_path,
            doc_type_value,
            symbols_value,
            language_value,
            symbol_kind.as_deref(),
            ranking_strategy,
        );
        let adjusted_score = score_components.final_score;
        let explain = if ranking_strategy.explain {
            Some(score_components.to_explain())
        } else {
            None
        };

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
                    let used = per_path_counts.get(&scope_path).copied().unwrap_or(0);
                    if used >= MAX_INITIAL_RESULTS_PER_PATH {
                        break;
                    }

                    candidates.push(IndexCandidate {
                        stored_path: path_value.to_string(),
                        full_path: full_path.clone(),
                        display_path: display_path.clone(),
                        score: adjusted_score,
                        explain: explain.clone(),
                        snippet,
                        line: Some(line_offset + rel_line.saturating_sub(1)),
                        symbol_id: None,
                        symbol_start: None,
                        symbol_end: None,
                    });
                    *per_path_counts.entry(scope_path.clone()).or_insert(0) += 1;
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
            score: adjusted_score,
            explain,
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
        *per_path_counts.entry(scope_path).or_insert(0) += 1;
    }

    Ok(candidates)
}

#[allow(clippy::too_many_arguments)]
fn keyword_search(
    query: &str,
    index_root: &Path,
    search_root: &Path,
    workspace_root: &Path,
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
    recursive: bool,
    no_ignore: bool,
    use_cache: bool,
    cache_ttl_ms: u64,
    ranking_strategy: &RankingStrategy,
) -> Result<SearchOutcome> {
    let force_scan_for_literal_query = requested_mode == IndexMode::Index
        && regex.is_none()
        && !fuzzy
        && should_force_scan_for_literal_query(query);
    let full_index_available = has_full_index(index_path);
    let mut use_index =
        requested_mode == IndexMode::Index && full_index_available && !force_scan_for_literal_query;
    let reuse_active = reuse_stale_filter_active(index_root);
    if use_index && reuse_active && !index_root.join(INDEX_DIR).join("metadata.json").is_file() {
        eprintln!(
            "Reuse stale-filter metadata missing at {}. Falling back to scan mode.",
            index_root.join(INDEX_DIR).join("metadata.json").display()
        );
        use_index = false;
    }
    if requested_mode == IndexMode::Index && !full_index_available {
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
        normalize_query(query, !case_sensitive, effective_mode == IndexMode::Index)
    };
    let changed_component = changed_filter
        .map(|f| format!("{}:{}", f.rev(), f.signature()))
        .filter(|s| !s.is_empty());
    let cache_key = CacheKey {
        query: normalized_query,
        mode: format!(
            "keyword:{}:r{}:ni{}:{}:pv3",
            if effective_mode == IndexMode::Index {
                "index"
            } else {
                "scan"
            },
            usize::from(recursive),
            usize::from(no_ignore),
            ranking_strategy.cache_mode_suffix(),
        ),
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
    let effective_use_cache = use_cache && !ranking_strategy.explain && !reuse_active;

    if effective_use_cache {
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
            workspace_root,
            max_results,
            context,
            file_type,
            compiled_glob,
            compiled_exclude,
            config_exclude_patterns,
            changed_filter,
            fuzzy,
            case_sensitive,
            recursive,
            ranking_strategy,
        )?
    } else {
        scan_search(
            query,
            search_root,
            workspace_root,
            max_results,
            context,
            file_type,
            compiled_glob,
            compiled_exclude,
            config_exclude_patterns,
            changed_filter,
            regex,
            case_sensitive,
            recursive,
            no_ignore,
            ranking_strategy,
        )?
    };

    if effective_use_cache {
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

const KEYWORD_FALLBACK_CONFIDENCE_THRESHOLD: f32 = 0.45;
const MAX_INITIAL_RESULTS_PER_PATH: usize = 2;
const NOISY_PATH_SEGMENTS: &[&str] = &["target/", "dist/", "build/", "node_modules/", ".venv/"];

struct KeywordFallbackPolicy<'a> {
    mode: HybridSearchMode,
    explicit_mode: bool,
    requested_mode: IndexMode,
    no_ignore: bool,
    fuzzy: bool,
    has_regex: bool,
    confidence: f32,
    results: &'a [SearchResult],
}

fn should_attempt_keyword_fallback(policy: &KeywordFallbackPolicy<'_>) -> bool {
    policy.mode == HybridSearchMode::Keyword
        && !policy.explicit_mode
        && policy.requested_mode == IndexMode::Index
        && !policy.no_ignore
        && !policy.fuzzy
        && !policy.has_regex
        && (policy.results.is_empty() || policy.confidence < KEYWORD_FALLBACK_CONFIDENCE_THRESHOLD)
}

fn estimate_confidence(results: &[SearchResult], mode: HybridSearchMode) -> f32 {
    if results.is_empty() {
        return 0.0;
    }
    let top_score = results.first().map(|r| r.score).unwrap_or(0.0);
    let count_factor = (results.len().min(5) as f32) / 5.0;
    let mode_bonus = match mode {
        HybridSearchMode::Keyword => 0.0,
        HybridSearchMode::Semantic => 0.05,
        HybridSearchMode::Hybrid => 0.08,
    };
    let confidence = 0.15 + (0.50 * count_factor) + (0.35 * score_to_unit(top_score)) + mode_bonus;
    confidence.clamp(0.0, 1.0)
}

fn score_to_unit(score: f32) -> f32 {
    if !score.is_finite() || score <= 0.0 {
        return 0.0;
    }
    (score / (score + 1.0)).clamp(0.0, 1.0)
}

fn estimate_json2_payload_chars(results: &[SearchJson2Result]) -> usize {
    results
        .iter()
        .map(|result| {
            let mut chars = result.id.len() + result.path.len() + result.snippet.len();
            if let Some(before) = &result.context_before {
                chars += before.iter().map(|line| line.len()).sum::<usize>();
            }
            if let Some(after) = &result.context_after {
                chars += after.iter().map(|line| line.len()).sum::<usize>();
            }
            chars
        })
        .sum()
}

fn estimate_tokens_from_chars(chars: usize) -> usize {
    chars.saturating_add(3) / 4
}

fn query_tokens_for_ranking(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != ':')
        .map(str::trim)
        .filter(|t| t.len() >= 3)
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

fn classify_query(query: &str) -> QueryClass {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return QueryClass::PhraseLike;
    }
    if trimmed.len() > 128 {
        return QueryClass::PhraseLike;
    }
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.' | '$'))
    {
        return QueryClass::IdentifierLike;
    }
    QueryClass::PhraseLike
}

fn query_requires_literal_handling(query: &str) -> bool {
    query.chars().any(is_query_parser_metachar)
}

fn should_force_scan_for_literal_query(query: &str) -> bool {
    query_requires_literal_handling(query) && !has_index_seed_token(query)
}

fn has_index_seed_token(query: &str) -> bool {
    query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .map(str::trim)
        .any(|token| token.len() >= 2)
}

fn is_query_parser_metachar(ch: char) -> bool {
    matches!(
        ch,
        '+' | '-'
            | '!'
            | '('
            | ')'
            | '{'
            | '}'
            | '['
            | ']'
            | '^'
            | '"'
            | '~'
            | '*'
            | '?'
            | ':'
            | '\\'
            | '/'
    )
}

fn escape_as_query_phrase(query: &str) -> String {
    let mut out = String::with_capacity(query.len() + 2);
    out.push('"');
    for ch in query.chars() {
        if ch == '"' || ch == '\\' {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

fn matches_literal_query(
    content: &str,
    symbols: &str,
    path: &str,
    query: &str,
    case_sensitive: bool,
) -> bool {
    literal_contains(content, query, case_sensitive)
        || literal_contains(symbols, query, case_sensitive)
        || literal_contains(path, query, case_sensitive)
}

fn literal_contains(text: &str, query: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        return text.contains(query);
    }
    text.to_ascii_lowercase()
        .contains(&query.to_ascii_lowercase())
}

fn single_identifier_query(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
        return None;
    }
    let ident = trimmed
        .rsplit("::")
        .next()
        .unwrap_or(trimmed)
        .rsplit('.')
        .next()
        .unwrap_or(trimmed);
    if ident
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    {
        Some(ident.to_ascii_lowercase())
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct ScoreComponents {
    bm25: f32,
    path_boost: f32,
    symbol_boost: f32,
    changed_boost: f32,
    kind_boost: f32,
    penalties: f32,
    final_score: f32,
}

impl ScoreComponents {
    fn to_explain(self) -> ScoreExplain {
        ScoreExplain {
            bm25: self.bm25,
            path_boost: self.path_boost,
            symbol_boost: self.symbol_boost,
            changed_boost: self.changed_boost,
            kind_boost: self.kind_boost,
            penalties: self.penalties,
            final_score: self.final_score,
        }
    }
}

fn path_signal_components(display_path: &str, query_tokens: &[String]) -> (f32, f32) {
    let path = display_path.to_ascii_lowercase();
    let mut bonus = 0.0f32;
    for token in query_tokens {
        if path.contains(token) {
            bonus += 0.03;
        }
    }
    let mut penalty = 0.0f32;
    for segment in NOISY_PATH_SEGMENTS {
        if path.contains(segment) {
            penalty += 0.08;
        }
    }
    (bonus.min(0.15), penalty.min(0.25))
}

fn path_ranking_bonus(display_path: &str, query_tokens: &[String]) -> f32 {
    let (bonus, penalty) = path_signal_components(display_path, query_tokens);
    (bonus - penalty).clamp(-0.25, 0.15)
}

fn symbol_ranking_bonus(
    doc_type: &str,
    symbols_value: &str,
    identifier_query: Option<&str>,
) -> f32 {
    let Some(identifier) = identifier_query else {
        return 0.0;
    };
    if symbols_value.is_empty() {
        return 0.0;
    }

    let symbol = symbols_value.to_ascii_lowercase();
    if doc_type == "symbol" {
        if symbol == identifier {
            return 0.35;
        }
        if symbol.starts_with(identifier) || identifier.starts_with(&symbol) {
            return 0.15;
        }
        if symbol.contains(identifier) {
            return 0.08;
        }
        return 0.0;
    }

    if symbol.split_whitespace().any(|token| token == identifier) {
        0.08
    } else if symbol.contains(identifier) {
        0.03
    } else {
        0.0
    }
}

fn infer_symbol_kind_from_content(content: &str) -> Option<String> {
    let first_line = content.lines().next()?.trim();
    let kind = first_line.split_whitespace().last()?.to_ascii_lowercase();
    if matches!(
        kind.as_str(),
        "function"
            | "method"
            | "class"
            | "struct"
            | "trait"
            | "interface"
            | "module"
            | "enum"
            | "type"
            | "property"
            | "constant"
            | "variable"
    ) {
        Some(kind)
    } else {
        None
    }
}

fn infer_kind_from_snippet(snippet: &str) -> Option<String> {
    let trimmed = snippet.trim_start().to_ascii_lowercase();
    if trimmed.starts_with("fn ") || trimmed.starts_with("def ") || trimmed.starts_with("function ")
    {
        return Some("function".to_string());
    }
    if trimmed.starts_with("class ") {
        return Some("class".to_string());
    }
    if trimmed.starts_with("module ") || trimmed.starts_with("mod ") {
        return Some("module".to_string());
    }
    None
}

fn kind_ranking_bonus(doc_type: &str, symbol_kind: Option<&str>, query_class: QueryClass) -> f32 {
    if query_class != QueryClass::IdentifierLike || doc_type != "symbol" {
        return 0.0;
    }
    match symbol_kind.unwrap_or_default() {
        "function" | "method" => 0.18,
        "class" | "struct" | "trait" | "interface" => 0.15,
        "module" | "enum" | "type" => 0.12,
        _ => 0.0,
    }
}

fn language_ranking_bonus(
    scope_path: &str,
    language_value: &str,
    language_filter: Option<&str>,
) -> f32 {
    let Some(filter) = language_filter else {
        return 0.0;
    };
    if language_value.eq_ignore_ascii_case(filter) || matches_file_type(scope_path, Some(filter)) {
        0.04
    } else {
        0.0
    }
}

fn query_class_weights(class: QueryClass) -> (f32, f32, f32, f32, f32, f32) {
    match class {
        QueryClass::IdentifierLike => (0.75, 1.35, 1.05, 0.90, 1.20, 1.15),
        QueryClass::PhraseLike => (1.10, 0.60, 1.00, 0.80, 0.30, 0.50),
    }
}

fn weak_signal_penalty(
    strategy: &RankingStrategy,
    scope_path: &str,
    symbol_boost_raw: f32,
    kind_boost_raw: f32,
) -> f32 {
    if strategy.query_class != QueryClass::IdentifierLike {
        return 0.0;
    }

    let has_identifier_in_path = strategy
        .identifier_query
        .as_deref()
        .map(|ident| scope_path.to_ascii_lowercase().contains(ident))
        .unwrap_or(false);

    if symbol_boost_raw <= 0.0 && kind_boost_raw <= 0.0 && !has_identifier_in_path {
        -0.08
    } else {
        0.0
    }
}

fn compute_keyword_score_components(
    bm25: f32,
    scope_path: &str,
    doc_type: &str,
    symbols_value: &str,
    language_value: &str,
    symbol_kind: Option<&str>,
    strategy: &RankingStrategy,
) -> ScoreComponents {
    let bm25 = if bm25.is_finite() { bm25.max(0.0) } else { 0.0 };
    let path_legacy = path_ranking_bonus(scope_path, &strategy.query_tokens);
    let symbol_legacy = symbol_ranking_bonus(
        doc_type,
        symbols_value,
        strategy.identifier_query.as_deref(),
    );

    if !strategy.enabled {
        let factor = (1.0 + path_legacy + symbol_legacy).max(0.05);
        return ScoreComponents {
            bm25,
            path_boost: path_legacy,
            symbol_boost: symbol_legacy,
            changed_boost: 0.0,
            kind_boost: 0.0,
            penalties: 0.0,
            final_score: bm25 * factor,
        };
    }

    let (path_base, noisy_penalty) = path_signal_components(scope_path, &strategy.query_tokens);
    let symbol_base = symbol_ranking_bonus(
        doc_type,
        symbols_value,
        strategy.identifier_query.as_deref(),
    );
    let language_base = language_ranking_bonus(
        scope_path,
        language_value,
        strategy.language_filter.as_deref(),
    );
    let changed_base = if strategy.changed_requested {
        0.05
    } else {
        0.0
    };
    let kind_base = kind_ranking_bonus(doc_type, symbol_kind, strategy.query_class);
    let weak_penalty_base = weak_signal_penalty(strategy, scope_path, symbol_base, kind_base);

    let (path_class_w, symbol_class_w, language_class_w, changed_class_w, kind_class_w, penalty_w) =
        query_class_weights(strategy.query_class);

    let language_boost = language_base * strategy.weights.language_weight * language_class_w;
    let path_boost = (path_base * strategy.weights.path_weight * path_class_w) + language_boost;
    let symbol_boost = symbol_base * strategy.weights.symbol_weight * symbol_class_w;
    let changed_boost = changed_base * strategy.weights.changed_weight * changed_class_w;
    let kind_boost = kind_base * strategy.weights.kind_weight * kind_class_w;
    let penalties =
        (-noisy_penalty) + (weak_penalty_base * strategy.weights.weak_signal_penalty * penalty_w);

    let factor =
        (1.0 + path_boost + symbol_boost + changed_boost + kind_boost + penalties).clamp(0.05, 5.0);
    ScoreComponents {
        bm25,
        path_boost,
        symbol_boost,
        changed_boost,
        kind_boost,
        penalties,
        final_score: bm25 * factor,
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
    workspace_root: &Path,
    max_results: usize,
    context: usize,
    file_type: Option<&str>,
    compiled_glob: Option<&CompiledGlob>,
    compiled_exclude: Option<&CompiledGlob>,
    config_exclude_patterns: &[CompiledGlob],
    changed_filter: Option<&ChangedFiles>,
    fuzzy: bool,
    case_sensitive: bool,
    recursive: bool,
    ranking_strategy: &RankingStrategy,
) -> Result<SearchOutcome> {
    let candidates = collect_index_candidates(
        query,
        index_root,
        search_root,
        workspace_root,
        max_results,
        "file",
        file_type,
        compiled_glob,
        compiled_exclude,
        config_exclude_patterns,
        changed_filter,
        recursive,
        fuzzy,
        case_sensitive,
        ranking_strategy,
    )?;

    let mut files_with_matches: HashSet<String> = HashSet::new();
    let mut results: Vec<SearchResult> = Vec::new();
    let mut context_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let stale_filter = load_reuse_stale_filter(index_root);
    let mut file_hash_cache: HashMap<PathBuf, Option<String>> = HashMap::new();

    for candidate in candidates {
        if let Some(filter) = stale_filter.as_ref() {
            if !candidate_is_fresh(&candidate, filter, &mut file_hash_cache) {
                continue;
            }
        }
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
            explain: candidate.explain,
        });
    }

    if ranking_strategy.enabled || ranking_strategy.explain {
        sort_results_deterministic(&mut results);
    }
    trim_explain_results(
        &mut results,
        ranking_strategy.explain,
        ranking_strategy.explain_top_k,
    );

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
    workspace_root: &Path,
    max_results: usize,
    context: usize,
    file_type: Option<&str>,
    compiled_glob: Option<&CompiledGlob>,
    compiled_exclude: Option<&CompiledGlob>,
    config_exclude_patterns: &[CompiledGlob],
    changed_filter: Option<&ChangedFiles>,
    regex: Option<&Regex>,
    case_sensitive: bool,
    recursive: bool,
    no_ignore: bool,
    ranking_strategy: &RankingStrategy,
) -> Result<SearchOutcome> {
    if query.trim().is_empty() {
        anyhow::bail!("Search query cannot be empty");
    }

    let query_lower = query.to_ascii_lowercase();

    let scanner = FileScanner::new(root)
        .with_recursive(recursive)
        .with_gitignore(!no_ignore);
    let mut files = scanner.scan()?;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let mut results: Vec<SearchResult> = Vec::new();
    let candidate_cap = max_results.max(1);

    'files: for file in files {
        if results.len() >= candidate_cap {
            break;
        }
        let scope_path = scope_relative_path(&file.path, root)
            .unwrap_or_else(|| file.path.display().to_string());
        let display_path = workspace_display_path(&file.path, workspace_root);
        let language_value = file
            .path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(crate::indexer::scanner::detect_language)
            .unwrap_or_default()
            .to_string();
        if let Some(filter) = changed_filter {
            if !filter.matches_rel_path(&scope_path) {
                continue;
            }
        }
        if !matches_file_type(&scope_path, file_type) {
            continue;
        }
        if !matches_glob_compiled(&scope_path, compiled_glob) {
            continue;
        }
        if should_exclude_compiled(&scope_path, compiled_exclude) {
            continue;
        }
        if config_exclude_patterns
            .iter()
            .any(|p| should_exclude_compiled(&scope_path, Some(p)))
        {
            continue;
        }

        if context == 0 {
            for (idx, line) in file.content.lines().enumerate() {
                if results.len() >= candidate_cap {
                    break 'files;
                }
                if !scan_line_matches(line, query, &query_lower, regex, case_sensitive) {
                    continue;
                }

                let snippet = truncate_with_ellipsis(line.trim(), 150);
                let symbol_kind = infer_kind_from_snippet(&snippet);
                let score_components = compute_keyword_score_components(
                    1.0,
                    &scope_path,
                    "file",
                    "",
                    language_value.as_str(),
                    symbol_kind.as_deref(),
                    ranking_strategy,
                );
                results.push(SearchResult {
                    path: display_path.clone(),
                    score: score_components.final_score,
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
                    explain: if ranking_strategy.explain {
                        Some(score_components.to_explain())
                    } else {
                        None
                    },
                });
            }
            continue;
        }

        let lines: Vec<&str> = file.content.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if results.len() >= candidate_cap {
                break 'files;
            }
            if !scan_line_matches(line, query, &query_lower, regex, case_sensitive) {
                continue;
            }

            let snippet = truncate_with_ellipsis(line.trim(), 150);
            let symbol_kind = infer_kind_from_snippet(&snippet);
            let score_components = compute_keyword_score_components(
                1.0,
                &scope_path,
                "file",
                "",
                language_value.as_str(),
                symbol_kind.as_deref(),
                ranking_strategy,
            );
            let (context_before, context_after) = get_context_from_lines(&lines, idx + 1, context);
            results.push(SearchResult {
                path: display_path.clone(),
                score: score_components.final_score,
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
                explain: if ranking_strategy.explain {
                    Some(score_components.to_explain())
                } else {
                    None
                },
            });
        }
    }

    sort_and_dedupe_scan_results(&mut results);
    if results.len() > max_results {
        results.truncate(max_results);
    }
    trim_explain_results(
        &mut results,
        ranking_strategy.explain,
        ranking_strategy.explain_top_k,
    );

    let files_with_matches_count = results
        .iter()
        .map(|result| result.path.clone())
        .collect::<HashSet<_>>()
        .len();
    let total_matches = results.len();

    Ok(SearchOutcome {
        results,
        files_with_matches: files_with_matches_count,
        total_matches,
        mode: IndexMode::Scan,
        cache_hit: false,
    })
}

fn has_full_index(index_path: &Path) -> bool {
    index_path.join("meta.json").is_file()
}

fn scan_line_matches(
    line: &str,
    query: &str,
    query_lower: &str,
    regex: Option<&Regex>,
    case_sensitive: bool,
) -> bool {
    if let Some(re) = regex {
        return re.is_match(line);
    }
    if case_sensitive {
        line.contains(query)
    } else {
        contains_ascii_case_insensitive(line, query_lower)
    }
}

fn contains_ascii_case_insensitive(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if needle_lower.len() > haystack.len() {
        return false;
    }
    if !haystack.is_ascii() || !needle_lower.is_ascii() {
        return haystack.to_ascii_lowercase().contains(needle_lower);
    }

    let needle = needle_lower.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    haystack_bytes.windows(needle.len()).any(|window| {
        window
            .iter()
            .zip(needle.iter())
            .all(|(a, b)| a.to_ascii_lowercase() == *b)
    })
}

fn sort_results_deterministic(results: &mut [SearchResult]) {
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
            .then_with(|| a.snippet.cmp(&b.snippet))
    });
}

fn trim_explain_results(results: &mut [SearchResult], explain_enabled: bool, top_k: usize) {
    if !explain_enabled {
        for result in results.iter_mut() {
            result.explain = None;
        }
        return;
    }

    for (idx, result) in results.iter_mut().enumerate() {
        if idx >= top_k {
            result.explain = None;
        }
    }
}

fn sort_and_dedupe_scan_results(results: &mut Vec<SearchResult>) {
    sort_results_deterministic(results);
    let mut seen: HashSet<(String, Option<usize>, String)> = HashSet::new();
    results
        .retain(|result| seen.insert((result.path.clone(), result.line, result.snippet.clone())));
}

/// Hybrid search combining BM25 with vector embeddings
#[allow(clippy::too_many_arguments)]
fn hybrid_search(
    query: &str,
    index_root: &Path,
    search_root: &Path,
    workspace_root: &Path,
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
    recursive: bool,
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
        "{}:k{}:wt{}:wv{}:r{}:pv2",
        mode,
        candidate_k,
        weight_text_milli,
        weight_vector_milli,
        usize::from(recursive)
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
                    .map(|hr| {
                        let full_path = resolve_full_path(&hr.path, index_root);
                        let display_path = workspace_display_path(&full_path, workspace_root);
                        SearchResult {
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
                            explain: None,
                        }
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

    let ranking_strategy = legacy_ranking_strategy(query, file_type, changed_filter);
    let bm25_candidates = collect_index_candidates(
        query,
        index_root,
        search_root,
        workspace_root,
        candidate_k,
        "symbol",
        file_type,
        compiled_glob,
        compiled_exclude,
        config_exclude_patterns,
        changed_filter,
        recursive,
        false,
        false,
        &ranking_strategy,
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
        let Some(scope_path) = scope_relative_path(&full_path, search_root) else {
            continue;
        };
        let display_path = workspace_display_path(&full_path, workspace_root);

        // Apply filters
        if !matches_file_type(&scope_path, file_type) {
            continue;
        }
        if !matches_glob_compiled(&scope_path, compiled_glob) {
            continue;
        }
        if should_exclude_compiled(&scope_path, compiled_exclude) {
            continue;
        }
        if config_exclude_patterns
            .iter()
            .any(|p| should_exclude_compiled(&scope_path, Some(p)))
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
            explain: None,
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

fn normalize_hint_path(result_path: &str, search_root: &Path, workspace_root: &Path) -> String {
    let candidate = Path::new(result_path);
    if !candidate.is_absolute() {
        if let Ok(search_relative) = search_root.strip_prefix(workspace_root) {
            if !search_relative.as_os_str().is_empty() {
                if let Ok(stripped) = candidate.strip_prefix(search_relative) {
                    let rendered = stripped.display().to_string();
                    if !rendered.is_empty() {
                        return rendered;
                    }
                }
            }
        }
    }

    let full_path = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        let workspace_candidate = workspace_root.join(candidate);
        if workspace_candidate.exists() {
            workspace_candidate
        } else {
            search_root.join(candidate)
        }
    };
    scope_relative_path(&full_path, search_root).unwrap_or_else(|| result_path.to_string())
}

fn scope_relative_path(full_path: &Path, search_root: &Path) -> Option<String> {
    let rel = full_path.strip_prefix(search_root).ok()?;
    let rendered = rel.display().to_string();
    if !rendered.is_empty() {
        return Some(rendered);
    }

    full_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
}

fn workspace_display_path(full_path: &Path, workspace_root: &Path) -> String {
    if workspace_root != Path::new("/") {
        if let Ok(rel) = full_path.strip_prefix(workspace_root) {
            let rendered = rel.display().to_string();
            if !rendered.is_empty() {
                return rendered;
            }
        }
    }

    if full_path.is_absolute() {
        return normalize_path(full_path).display().to_string();
    }

    normalize_path(&workspace_root.join(full_path))
        .display()
        .to_string()
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
            true,
            false,
            &legacy_ranking_strategy("world", None, None),
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
            true,
            false,
            &legacy_ranking_strategy(r"\d{3}", None, None),
        )
        .expect("scan");

        assert_eq!(outcome.results.len(), 2);
        assert_eq!(outcome.results[0].path, "numbers.txt");
        assert_eq!(outcome.results[0].line, Some(1));
        assert_eq!(outcome.results[1].line, Some(3));
    }

    #[test]
    fn scope_relative_path_for_file_scope_is_non_empty() {
        let root = Path::new("/tmp/work/src/lib.rs");
        let rel = scope_relative_path(root, root).expect("scope path");
        assert_eq!(rel, "lib.rs");
    }

    #[test]
    fn normalize_hint_path_uses_search_root_relative_paths() {
        let search_root = Path::new("/tmp/work/src");
        let workspace_root = Path::new("/tmp/work");
        let normalized = normalize_hint_path("src/lib.rs", search_root, workspace_root);
        assert_eq!(normalized, "lib.rs");
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
            root,
            10,
            0,
            None,
            None,
            None,
            &[],
            None,
            false,
            false,
            true,
            &legacy_ranking_strategy("needle", None, None),
        )
        .expect("index search");

        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "src/sub.rs");
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
            root,
            1,
            0,
            None,
            None,
            None,
            &[],
            None,
            false,
            false,
            true,
            &legacy_ranking_strategy("needle", None, None),
        )
        .expect("index search");

        assert_eq!(outcome.results.len(), 1);
        assert_eq!(outcome.results[0].path, "scoped/target.txt");
    }

    #[test]
    fn index_search_no_recursive_skips_nested_paths() {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path();
        let nested_dir = root.join("nested");
        std::fs::create_dir_all(&nested_dir).expect("create nested");
        std::fs::write(root.join("top.txt"), "needle top\n").expect("write top");
        std::fs::write(nested_dir.join("deep.txt"), "needle deep\n").expect("write deep");

        let builder = IndexBuilder::new(root).expect("builder");
        builder
            .build(false, DEFAULT_WRITER_BUDGET_BYTES)
            .expect("build");

        let outcome = index_search(
            "needle",
            root,
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
            false,
            false,
            &legacy_ranking_strategy("needle", None, None),
        )
        .expect("index search");

        assert!(outcome.results.iter().any(|r| r.path == "top.txt"));
        assert!(outcome.results.iter().all(|r| r.path != "nested/deep.txt"));
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
            root,
            10,
            0,
            None,
            None,
            None,
            &[],
            None,
            false,
            false,
            true,
            &legacy_ranking_strategy("cpu_fallback_path", None, None),
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
                explain: None,
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
                explain: None,
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
            explain: None,
        };

        let a = stable_result_id(&result);
        let b = stable_result_id(&result);
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn ranking_path_bonus_rewards_path_token_overlap() {
        let tokens = query_tokens_for_ranking("auth token refresh");
        let with_overlap = path_ranking_bonus("src/auth/token_manager.rs", &tokens);
        let without_overlap = path_ranking_bonus("src/cache/lru.rs", &tokens);
        assert!(with_overlap > without_overlap);
    }

    #[test]
    fn ranking_path_bonus_penalizes_noise_directories() {
        let tokens = query_tokens_for_ranking("needle");
        let noisy = path_ranking_bonus("target/debug/noise.rs", &tokens);
        let clean = path_ranking_bonus("src/core/noise.rs", &tokens);
        assert!(noisy < clean);
    }

    #[test]
    fn ranking_symbol_bonus_prefers_exact_symbol_match() {
        let exact = symbol_ranking_bonus("symbol", "target_fn", Some("target_fn"));
        let partial = symbol_ranking_bonus("symbol", "target_fn_impl", Some("target_fn"));
        let none = symbol_ranking_bonus("symbol", "other_name", Some("target_fn"));

        assert!(exact > partial);
        assert!(partial >= none);
    }

    #[test]
    fn query_classifier_is_deterministic() {
        assert_eq!(classify_query("target_fn"), QueryClass::IdentifierLike);
        assert_eq!(
            classify_query("crate::service::run"),
            QueryClass::IdentifierLike
        );
        assert_eq!(
            classify_query("retry backoff strategy"),
            QueryClass::PhraseLike
        );
        assert_eq!(classify_query("target-fn"), QueryClass::PhraseLike);
    }

    #[test]
    fn legacy_components_match_previous_keyword_formula() {
        let strategy = legacy_ranking_strategy("target_fn", None, None);
        let components = compute_keyword_score_components(
            3.5,
            "src/auth/target_fn.rs",
            "symbol",
            "target_fn",
            "rust",
            Some("function"),
            &strategy,
        );
        let expected = 3.5
            * (1.0
                + path_ranking_bonus(
                    "src/auth/target_fn.rs",
                    &query_tokens_for_ranking("target_fn"),
                )
                + symbol_ranking_bonus("symbol", "target_fn", Some("target_fn")));
        assert!((components.final_score - expected).abs() < 0.0001);
    }

    #[test]
    fn explain_trimming_keeps_only_top_k() {
        let mut results = vec![
            sample_result("a.rs", 1, "alpha"),
            sample_result("b.rs", 2, "beta"),
            sample_result("c.rs", 3, "gamma"),
        ];
        for (idx, result) in results.iter_mut().enumerate() {
            result.explain = Some(ScoreExplain {
                bm25: 1.0 + idx as f32,
                path_boost: 0.0,
                symbol_boost: 0.0,
                changed_boost: 0.0,
                kind_boost: 0.0,
                penalties: 0.0,
                final_score: 1.0 + idx as f32,
            });
        }
        trim_explain_results(&mut results, true, 2);
        assert!(results[0].explain.is_some());
        assert!(results[1].explain.is_some());
        assert!(results[2].explain.is_none());
    }

    #[test]
    fn keyword_fallback_policy_respects_explicit_mode() {
        let results = vec![sample_result("src/lib.rs", 1, "needle")];
        let explicit = KeywordFallbackPolicy {
            mode: HybridSearchMode::Keyword,
            explicit_mode: true,
            requested_mode: IndexMode::Index,
            no_ignore: false,
            fuzzy: false,
            has_regex: false,
            confidence: 0.1,
            results: &results,
        };
        assert!(!should_attempt_keyword_fallback(&explicit));

        let implicit = KeywordFallbackPolicy {
            mode: HybridSearchMode::Keyword,
            explicit_mode: false,
            requested_mode: IndexMode::Index,
            no_ignore: false,
            fuzzy: false,
            has_regex: false,
            confidence: 0.1,
            results: &results,
        };
        assert!(should_attempt_keyword_fallback(&implicit));
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
            explain: None,
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
