// SPDX-License-Identifier: MIT OR Apache-2.0

//! Agent-oriented query helpers.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::{CliBudgetPreset, CliSearchMode};
use crate::indexer::scanner::FileScanner;
use cgrep::output::print_json;

const AGENT_HINT_CACHE_REL: &str = ".cgrep/cache/agent_expand_hints.json";
const AGENT_HINT_CACHE_VERSION: u32 = 1;
const AGENT_HINT_TTL_SECS: u64 = 60 * 60 * 24 * 7; // 7 days
const AGENT_HINT_MAX_ENTRIES: usize = 10_000;
const PLAN_DEFAULT_MAX_STEPS: usize = 6;
const PLAN_DEFAULT_MAX_CANDIDATES: usize = 5;
const PLAN_MAX_STEPS_LIMIT: usize = 32;
const PLAN_MAX_CANDIDATES_LIMIT: usize = 50;
const PLAN_LOCATE_LIMIT_FACTOR: usize = 4;
const PLAN_MAP_DEPTH: usize = 2;
const PLAN_EXPAND_CONTEXT: usize = 8;
const PLAN_PAYLOAD_CHAR_LIMIT: usize = 24_000;

#[derive(Debug, Serialize)]
struct AgentExpandMeta {
    schema_version: &'static str,
    stage: &'static str,
    requested_ids: usize,
    resolved_ids: usize,
    hint_resolved_ids: usize,
    scan_resolved_ids: usize,
    context: usize,
    search_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentExpandResult {
    id: String,
    path: String,
    line: usize,
    start_line: usize,
    end_line: usize,
    snippet: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    context_before: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    context_after: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AgentExpandPayload {
    meta: AgentExpandMeta,
    results: Vec<AgentExpandResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentHintEntry {
    id: String,
    path: String,
    line: usize,
    updated_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct AgentHintCacheFile {
    version: u32,
    entries: Vec<AgentHintEntry>,
}

#[derive(Debug, Clone)]
pub struct AgentPlanOptions {
    pub path: Option<String>,
    pub changed: Option<String>,
    pub mode: Option<CliSearchMode>,
    pub budget: CliBudgetPreset,
    pub profile: String,
    pub max_steps: Option<usize>,
    pub max_candidates: Option<usize>,
    pub compact: bool,
}

#[derive(Debug, Serialize)]
struct AgentPlanMeta {
    schema_version: &'static str,
    stage: &'static str,
    query: String,
    profile: String,
    budget: String,
    strategy: &'static str,
    max_steps: usize,
    max_candidates: usize,
    truncated: bool,
    repo: AgentPlanRepoMeta,
}

#[derive(Debug, Serialize)]
struct AgentPlanRepoMeta {
    search_root: String,
    repo_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    head_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest_root_hash: Option<String>,
    cgrep_version: &'static str,
}

#[derive(Debug, Serialize)]
struct AgentPlanStep {
    id: String,
    command: String,
    args: Vec<String>,
    reason: String,
    expected_output: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct AgentPlanCandidate {
    id: String,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    summary: String,
    score: f32,
}

#[derive(Debug, Serialize)]
struct AgentPlanDiagnostic {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    step_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct AgentPlanError {
    code: String,
    field: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct AgentPlanPayload {
    meta: AgentPlanMeta,
    steps: Vec<AgentPlanStep>,
    candidates: Vec<AgentPlanCandidate>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<AgentPlanDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<AgentPlanError>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PlanLocateMeta {
    path_aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct PlanLocateResult {
    id: String,
    path: String,
    line: Option<usize>,
    snippet: String,
    score: f32,
}

impl Default for PlanLocateResult {
    fn default() -> Self {
        Self {
            id: String::new(),
            path: String::new(),
            line: None,
            snippet: String::new(),
            score: 0.0,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PlanLocatePayload {
    meta: PlanLocateMeta,
    results: Vec<PlanLocateResult>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PlanExpandPayload {
    results: Vec<AgentExpandResult>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PlanMapPayload {
    entries: Vec<serde_json::Value>,
}

/// Produce a deterministic multi-step retrieval plan for AI agents.
pub fn run_plan(query: &str, options: &AgentPlanOptions) -> Result<()> {
    let search_root = resolve_search_root(options.path.as_deref())?;
    let execution_root =
        std::env::current_dir().context("Cannot determine current directory for planner")?;
    let max_steps = match resolve_plan_limit(
        options.max_steps,
        PLAN_DEFAULT_MAX_STEPS,
        PLAN_MAX_STEPS_LIMIT,
        "max_steps",
    ) {
        Ok(value) => value,
        Err(error) => return emit_plan_error(query, options, &search_root, error),
    };
    let max_candidates = match resolve_plan_limit(
        options.max_candidates,
        PLAN_DEFAULT_MAX_CANDIDATES,
        PLAN_MAX_CANDIDATES_LIMIT,
        "max_candidates",
    ) {
        Ok(value) => value,
        Err(error) => return emit_plan_error(query, options, &search_root, error),
    };
    if query.trim().is_empty() {
        return emit_plan_error(
            query,
            options,
            &search_root,
            AgentPlanError {
                code: "invalid_option".to_string(),
                field: "query".to_string(),
                message: "query cannot be empty".to_string(),
            },
        );
    }

    let profile = normalize_profile_name(&options.profile);
    let budget = budget_name(options.budget).to_string();
    let mut payload = AgentPlanPayload {
        meta: AgentPlanMeta {
            schema_version: "1",
            stage: "plan",
            query: query.to_string(),
            profile,
            budget,
            strategy: "broad-narrow-expand",
            max_steps,
            max_candidates,
            truncated: false,
            repo: plan_repo_meta(&search_root),
        },
        steps: Vec::new(),
        candidates: Vec::new(),
        diagnostics: Vec::new(),
        error: None,
    };

    let mut step_seq = 1usize;
    let include_map = should_include_map_step(query) && max_steps >= 3;
    if include_map && payload.steps.len() < max_steps {
        let mut args = vec!["--depth".to_string(), PLAN_MAP_DEPTH.to_string()];
        if let Some(path) = options.path.as_ref() {
            args.push("--path".to_string());
            args.push(path.clone());
        }
        let step_id = format_step_id(step_seq, "map");
        step_seq += 1;
        let mut step = AgentPlanStep {
            id: step_id.clone(),
            command: "map".to_string(),
            args: args.clone(),
            reason: "Capture top-level structure before retrieval.".to_string(),
            expected_output: "json2.map".to_string(),
            status: "planned".to_string(),
            result_count: None,
        };
        match run_cgrep_json::<PlanMapPayload>(&execution_root, "map", &args) {
            Ok(map_payload) => {
                step.status = "executed".to_string();
                step.result_count = Some(map_payload.entries.len());
            }
            Err(code) => {
                step.status = "failed".to_string();
                payload.diagnostics.push(AgentPlanDiagnostic {
                    code,
                    message: "map step failed".to_string(),
                    step_id: Some(step_id.clone()),
                });
            }
        }
        payload.steps.push(step);
    }

    let locate_limit = max_candidates
        .saturating_mul(PLAN_LOCATE_LIMIT_FACTOR)
        .clamp(max_candidates, 100);
    let mut locate_args = vec![
        query.to_string(),
        "--limit".to_string(),
        locate_limit.to_string(),
    ];
    locate_args.push("--budget".to_string());
    locate_args.push(budget_name(options.budget).to_string());
    if let Some(path) = options.path.as_ref() {
        locate_args.push("--path".to_string());
        locate_args.push(path.clone());
    }
    if let Some(changed) = options.changed.as_ref() {
        locate_args.push("--changed".to_string());
        locate_args.push(changed.clone());
    }
    if let Some(mode) = options.mode {
        locate_args.push("--mode".to_string());
        locate_args.push(mode_name(mode).to_string());
    }

    let locate_step_id = format_step_id(step_seq, "locate");
    step_seq += 1;
    let mut locate_step = AgentPlanStep {
        id: locate_step_id.clone(),
        command: "agent locate".to_string(),
        args: locate_args.clone(),
        reason: "Find broad candidate regions with compact payload.".to_string(),
        expected_output: "json2.search".to_string(),
        status: "planned".to_string(),
        result_count: None,
    };
    let mut selected_locate_results: Vec<PlanLocateResult> = Vec::new();
    if payload.steps.len() < max_steps {
        match run_cgrep_json::<PlanLocatePayload>(&execution_root, "agent locate", &locate_args) {
            Ok(mut locate_payload) => {
                locate_step.status = "executed".to_string();
                locate_step.result_count = Some(locate_payload.results.len());
                for result in &mut locate_payload.results {
                    if let Some(alias_target) = locate_payload.meta.path_aliases.get(&result.path) {
                        result.path = alias_target.clone();
                    }
                }
                locate_payload.results.sort_by(compare_locate_results);
                let mut deduped = Vec::with_capacity(locate_payload.results.len());
                let mut seen_ids = HashSet::new();
                for row in locate_payload.results {
                    if row.id.is_empty() || !seen_ids.insert(row.id.clone()) {
                        continue;
                    }
                    deduped.push(row);
                }
                selected_locate_results = deduped.into_iter().take(max_candidates).collect();
            }
            Err(code) => {
                locate_step.status = "failed".to_string();
                payload.diagnostics.push(AgentPlanDiagnostic {
                    code,
                    message: "locate step failed".to_string(),
                    step_id: Some(locate_step_id.clone()),
                });
            }
        }
        payload.steps.push(locate_step);
    }

    let mut expand_by_id: HashMap<String, AgentExpandResult> = HashMap::new();
    if !selected_locate_results.is_empty() && payload.steps.len() < max_steps {
        let mut expand_args: Vec<String> = Vec::new();
        for row in &selected_locate_results {
            expand_args.push("--id".to_string());
            expand_args.push(row.id.clone());
        }
        expand_args.push("--context".to_string());
        expand_args.push(PLAN_EXPAND_CONTEXT.to_string());
        if let Some(path) = options.path.as_ref() {
            expand_args.push("--path".to_string());
            expand_args.push(path.clone());
        }
        let expand_step_id = format_step_id(step_seq, "expand");
        step_seq += 1;
        let mut expand_step = AgentPlanStep {
            id: expand_step_id.clone(),
            command: "agent expand".to_string(),
            args: expand_args.clone(),
            reason: "Expand top candidates with bounded context.".to_string(),
            expected_output: "json2.expand".to_string(),
            status: "planned".to_string(),
            result_count: None,
        };
        match run_cgrep_json::<PlanExpandPayload>(&execution_root, "agent expand", &expand_args) {
            Ok(expand_payload) => {
                expand_step.status = "executed".to_string();
                expand_step.result_count = Some(expand_payload.results.len());
                for row in expand_payload.results {
                    expand_by_id.insert(row.id.clone(), row);
                }
            }
            Err(code) => {
                expand_step.status = "failed".to_string();
                payload.diagnostics.push(AgentPlanDiagnostic {
                    code,
                    message: "expand step failed".to_string(),
                    step_id: Some(expand_step_id.clone()),
                });
            }
        }
        payload.steps.push(expand_step);
    }
    if !selected_locate_results.is_empty() {
        let mut candidates = Vec::with_capacity(selected_locate_results.len());
        for row in &selected_locate_results {
            let expanded = expand_by_id.get(&row.id);
            let path = expanded
                .map(|entry| entry.path.clone())
                .unwrap_or_else(|| row.path.clone());
            let line = expanded.map(|entry| entry.line).or(row.line);
            let summary_src = expanded
                .map(|entry| entry.snippet.as_str())
                .unwrap_or(row.snippet.as_str());
            candidates.push(AgentPlanCandidate {
                id: row.id.clone(),
                path,
                line,
                summary: truncate_summary(summary_src, 180),
                score: row.score,
            });
        }
        payload.candidates = candidates;
    }

    if let Some(identifier) = identifier_like_query(query) {
        let nav_templates = navigation_templates(
            &identifier,
            options.path.as_deref(),
            options.changed.as_deref(),
        );
        for (label, args, reason, output_type) in nav_templates {
            if payload.steps.len() >= max_steps {
                break;
            }
            let step_id = format_step_id(step_seq, label);
            step_seq += 1;
            payload.steps.push(AgentPlanStep {
                id: step_id,
                command: label.to_string(),
                args,
                reason: reason.to_string(),
                expected_output: output_type.to_string(),
                status: "planned".to_string(),
                result_count: None,
            });
        }
    }

    payload.meta.truncated = enforce_plan_payload_budget(&mut payload, PLAN_PAYLOAD_CHAR_LIMIT);
    print_json(&payload, options.compact)?;
    Ok(())
}

fn resolve_plan_limit(
    requested: Option<usize>,
    default_value: usize,
    max_value: usize,
    field: &str,
) -> std::result::Result<usize, AgentPlanError> {
    let value = requested.unwrap_or(default_value);
    if value == 0 {
        return Err(AgentPlanError {
            code: "invalid_option".to_string(),
            field: field.to_string(),
            message: format!("{field} must be >= 1"),
        });
    }
    if value > max_value {
        return Err(AgentPlanError {
            code: "invalid_option".to_string(),
            field: field.to_string(),
            message: format!("{field} must be <= {max_value}"),
        });
    }
    Ok(value)
}

fn emit_plan_error(
    query: &str,
    options: &AgentPlanOptions,
    search_root: &Path,
    error: AgentPlanError,
) -> Result<()> {
    let payload = AgentPlanPayload {
        meta: AgentPlanMeta {
            schema_version: "1",
            stage: "plan",
            query: query.to_string(),
            profile: normalize_profile_name(&options.profile),
            budget: budget_name(options.budget).to_string(),
            strategy: "broad-narrow-expand",
            max_steps: options.max_steps.unwrap_or(PLAN_DEFAULT_MAX_STEPS),
            max_candidates: options
                .max_candidates
                .unwrap_or(PLAN_DEFAULT_MAX_CANDIDATES),
            truncated: false,
            repo: plan_repo_meta(search_root),
        },
        steps: Vec::new(),
        candidates: Vec::new(),
        diagnostics: Vec::new(),
        error: Some(error),
    };
    print_json(&payload, options.compact)?;
    Ok(())
}

fn normalize_profile_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn budget_name(budget: CliBudgetPreset) -> &'static str {
    match budget {
        CliBudgetPreset::Tight => "tight",
        CliBudgetPreset::Balanced => "balanced",
        CliBudgetPreset::Full => "full",
        CliBudgetPreset::Off => "off",
    }
}

fn mode_name(mode: CliSearchMode) -> &'static str {
    match mode {
        CliSearchMode::Keyword => "keyword",
        CliSearchMode::Semantic => "semantic",
        CliSearchMode::Hybrid => "hybrid",
    }
}

fn plan_repo_meta(search_root: &Path) -> AgentPlanRepoMeta {
    let head_commit = git_output(search_root, &["rev-parse", "HEAD"]);
    let manifest_root_hash = read_trimmed_file(
        &search_root
            .join(".cgrep")
            .join("manifest")
            .join("root.hash"),
    );
    let fingerprint_input = format!(
        "{}|{}|{}",
        head_commit.as_deref().unwrap_or("-"),
        manifest_root_hash.as_deref().unwrap_or("-"),
        search_root.display()
    );
    let hash = blake3::hash(fingerprint_input.as_bytes());
    AgentPlanRepoMeta {
        search_root: search_root.display().to_string(),
        repo_fingerprint: hash.to_hex()[..16].to_string(),
        head_commit,
        manifest_root_hash,
        cgrep_version: env!("CARGO_PKG_VERSION"),
    }
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8(output.stdout).ok()?;
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn read_trimmed_file(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn should_include_map_step(query: &str) -> bool {
    let token_count = query.split_whitespace().filter(|t| !t.is_empty()).count();
    token_count >= 4 || query.len() >= 48
}

fn format_step_id(sequence: usize, label: &str) -> String {
    let mut slug = String::new();
    let mut prev_sep = false;
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_sep = false;
        } else if !prev_sep {
            slug.push('_');
            prev_sep = true;
        }
    }
    let slug = slug.trim_matches('_');
    if slug.is_empty() {
        format!("s{sequence:02}")
    } else {
        format!("s{sequence:02}_{slug}")
    }
}

fn compare_locate_results(left: &PlanLocateResult, right: &PlanLocateResult) -> Ordering {
    right
        .score
        .total_cmp(&left.score)
        .then_with(|| left.path.cmp(&right.path))
        .then_with(|| {
            left.line
                .unwrap_or(usize::MAX)
                .cmp(&right.line.unwrap_or(usize::MAX))
        })
        .then_with(|| left.id.cmp(&right.id))
}

fn truncate_summary(text: &str, max_chars: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let mut out = String::new();
    for (idx, ch) in collapsed.chars().enumerate() {
        if idx >= max_chars.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push_str("...");
    out
}

fn identifier_like_query(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.contains(char::is_whitespace) || trimmed.len() > 128 {
        return None;
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.' | '$'))
    {
        return None;
    }
    let ident = trimmed
        .rsplit("::")
        .next()
        .unwrap_or(trimmed)
        .rsplit('.')
        .next()
        .unwrap_or(trimmed);
    if ident.is_empty() {
        None
    } else {
        Some(ident.to_ascii_lowercase())
    }
}

fn navigation_templates(
    identifier: &str,
    path: Option<&str>,
    changed: Option<&str>,
) -> Vec<(&'static str, Vec<String>, &'static str, &'static str)> {
    let mut definition_args = vec![
        identifier.to_string(),
        "--limit".to_string(),
        "5".to_string(),
    ];
    if let Some(path) = path {
        definition_args.push("--path".to_string());
        definition_args.push(path.to_string());
    }

    let mut reference_args = vec![
        identifier.to_string(),
        "--limit".to_string(),
        "20".to_string(),
        "--mode".to_string(),
        "auto".to_string(),
    ];
    if let Some(path) = path {
        reference_args.push("--path".to_string());
        reference_args.push(path.to_string());
    }
    if let Some(changed) = changed {
        reference_args.push("--changed".to_string());
        reference_args.push(changed.to_string());
    }

    let caller_args = vec![
        identifier.to_string(),
        "--mode".to_string(),
        "auto".to_string(),
    ];

    vec![
        (
            "definition",
            definition_args,
            "Confirm canonical definition for identifier-like query.",
            "json2.definition",
        ),
        (
            "references",
            reference_args,
            "Expand usage graph around the identifier.",
            "json2.references",
        ),
        (
            "callers",
            caller_args,
            "Trace incoming call sites when symbol is callable.",
            "json2.callers",
        ),
    ]
}

fn run_cgrep_json<T>(
    search_root: &Path,
    command: &str,
    args: &[String],
) -> std::result::Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let mut cmd_args = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
    ];
    cmd_args.extend(command.split_whitespace().map(|token| token.to_string()));
    cmd_args.extend(args.iter().cloned());

    let executable =
        std::env::current_exe().map_err(|_| "planner_executable_unavailable".to_string())?;
    let output = StdCommand::new(executable)
        .current_dir(search_root)
        .args(cmd_args)
        .output()
        .map_err(|_| "planner_subcommand_spawn_failed".to_string())?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(format!("planner_subcommand_failed_{code}"));
    }

    serde_json::from_slice::<T>(&output.stdout)
        .map_err(|_| "planner_subcommand_invalid_json".to_string())
}

fn enforce_plan_payload_budget(payload: &mut AgentPlanPayload, max_chars: usize) -> bool {
    let mut truncated = false;
    while serde_json::to_string(payload)
        .map(|json| json.len())
        .unwrap_or_default()
        > max_chars
    {
        if !payload.diagnostics.is_empty() {
            payload.diagnostics.pop();
            truncated = true;
            continue;
        }
        if !payload.candidates.is_empty() {
            payload.candidates.pop();
            truncated = true;
            continue;
        }
        break;
    }
    truncated
}

/// Expand stable result IDs into richer context windows for agent workflows.
pub fn run_expand(ids: &[String], path: Option<&str>, context: usize, compact: bool) -> Result<()> {
    let search_root = resolve_search_root(path)?;
    let wanted: HashSet<String> = ids.iter().cloned().collect();
    let mut unresolved: HashSet<String> = wanted.iter().cloned().collect();
    let mut results: Vec<AgentExpandResult> = Vec::new();
    let mut hint_resolved_ids = 0usize;
    let mut scan_resolved_ids = 0usize;

    let hint_map = load_hint_map(&search_root).unwrap_or_default();
    let mut line_cache: HashMap<String, Vec<String>> = HashMap::new();
    for id in ids {
        if !unresolved.contains(id) {
            continue;
        }
        if let Some(hint) = hint_map.get(id) {
            if let Some(result) = resolve_from_hint(&search_root, hint, context, &mut line_cache) {
                results.push(result);
                unresolved.remove(id);
                hint_resolved_ids += 1;
            }
        }
    }

    if !unresolved.is_empty() {
        let scanner = FileScanner::new(&search_root);
        let files = scanner.list_files()?;
        for file_path in files {
            if unresolved.is_empty() {
                break;
            }

            let content = match fs::read_to_string(&file_path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let rel_path = file_path
                .strip_prefix(&search_root)
                .unwrap_or(&file_path)
                .display()
                .to_string();

            let lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
            for (idx, line) in lines.iter().enumerate() {
                let line_num = idx + 1;
                let snippet = line_to_snippet(line);
                let id = stable_result_id(&rel_path, line_num, &snippet);
                if !unresolved.remove(&id) {
                    continue;
                }

                let (context_before, context_after) =
                    context_from_string_lines(&lines, line_num, context);
                let start_line = line_num.saturating_sub(context_before.len());
                let end_line = line_num + context_after.len();

                results.push(AgentExpandResult {
                    id: id.clone(),
                    path: rel_path.clone(),
                    line: line_num,
                    start_line,
                    end_line,
                    snippet,
                    context_before,
                    context_after,
                });
                scan_resolved_ids += 1;

                if unresolved.is_empty() {
                    break;
                }
            }
        }
    }

    results.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));

    let payload = AgentExpandPayload {
        meta: AgentExpandMeta {
            schema_version: "1",
            stage: "expand",
            requested_ids: wanted.len(),
            resolved_ids: results.len(),
            hint_resolved_ids,
            scan_resolved_ids,
            context,
            search_root: search_root.display().to_string(),
        },
        results,
    };
    print_json(&payload, compact)?;

    Ok(())
}

pub(crate) fn persist_expand_hints(
    search_root: &Path,
    hints: impl IntoIterator<Item = AgentHintInput>,
) -> Result<()> {
    let path = hint_cache_path(search_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let now = current_unix_secs();
    let mut cache = load_hint_cache(&path).unwrap_or_default();
    if cache.version != AGENT_HINT_CACHE_VERSION {
        cache = AgentHintCacheFile {
            version: AGENT_HINT_CACHE_VERSION,
            entries: Vec::new(),
        };
    }

    let mut by_id: HashMap<String, AgentHintEntry> = HashMap::with_capacity(cache.entries.len());
    for entry in cache.entries {
        if now.saturating_sub(entry.updated_at) > AGENT_HINT_TTL_SECS {
            continue;
        }
        by_id.insert(entry.id.clone(), entry);
    }

    for hint in hints {
        if hint.line == 0 || hint.path.is_empty() {
            continue;
        }
        let id = match hint.id {
            Some(id) => id,
            None => {
                if hint.snippet.is_empty() {
                    continue;
                }
                stable_result_id(&hint.path, hint.line, &hint.snippet)
            }
        };
        let entry = AgentHintEntry {
            id: id.clone(),
            path: hint.path,
            line: hint.line,
            updated_at: now,
        };
        by_id.insert(id, entry);
    }

    let mut entries: Vec<AgentHintEntry> = by_id.into_values().collect();
    entries.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    if entries.len() > AGENT_HINT_MAX_ENTRIES {
        entries.truncate(AGENT_HINT_MAX_ENTRIES);
    }

    let data = AgentHintCacheFile {
        version: AGENT_HINT_CACHE_VERSION,
        entries,
    };
    let content = serde_json::to_string_pretty(&data).context("Failed to encode hint cache")?;
    fs::write(&path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct AgentHintInput {
    pub id: Option<String>,
    pub path: String,
    pub line: usize,
    pub snippet: String,
}

fn load_hint_map(search_root: &Path) -> Result<HashMap<String, AgentHintEntry>> {
    let path = hint_cache_path(search_root);
    let cache = load_hint_cache(&path)?;
    let now = current_unix_secs();
    let mut map = HashMap::with_capacity(cache.entries.len());
    for entry in cache.entries {
        if now.saturating_sub(entry.updated_at) > AGENT_HINT_TTL_SECS {
            continue;
        }
        map.insert(entry.id.clone(), entry);
    }
    Ok(map)
}

fn load_hint_cache(path: &Path) -> Result<AgentHintCacheFile> {
    if !path.exists() {
        return Ok(AgentHintCacheFile {
            version: AGENT_HINT_CACHE_VERSION,
            entries: Vec::new(),
        });
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let parsed = serde_json::from_str::<AgentHintCacheFile>(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(parsed)
}

fn hint_cache_path(search_root: &Path) -> PathBuf {
    search_root.join(AGENT_HINT_CACHE_REL)
}

fn resolve_from_hint(
    search_root: &Path,
    hint: &AgentHintEntry,
    context: usize,
    line_cache: &mut HashMap<String, Vec<String>>,
) -> Option<AgentExpandResult> {
    if hint.line == 0 {
        return None;
    }
    let full_path = search_root.join(&hint.path);
    if !full_path.exists() {
        return None;
    }

    if !line_cache.contains_key(&hint.path) {
        let content = fs::read_to_string(&full_path).ok()?;
        let lines = content
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        line_cache.insert(hint.path.clone(), lines);
    }
    let lines = line_cache.get(&hint.path)?;
    if hint.line > lines.len() {
        return None;
    }

    let snippet = line_to_snippet(lines[hint.line - 1].as_str());
    let actual_id = stable_result_id(&hint.path, hint.line, &snippet);
    if actual_id != hint.id {
        return None;
    }

    let (context_before, context_after) = context_from_string_lines(lines, hint.line, context);
    let start_line = hint.line.saturating_sub(context_before.len());
    let end_line = hint.line + context_after.len();

    Some(AgentExpandResult {
        id: hint.id.clone(),
        path: hint.path.clone(),
        line: hint.line,
        start_line,
        end_line,
        snippet,
        context_before,
        context_after,
    })
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
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                cleaned.pop();
            }
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::Normal(_) => {
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

fn line_to_snippet(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() <= 150 {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..150])
    }
}

fn stable_result_id(path: &str, line: usize, snippet: &str) -> String {
    let payload = format!("{}:{}:{}", path, line, snippet);
    let hash = blake3::hash(payload.as_bytes());
    hash.to_hex()[..16].to_string()
}

fn context_from_string_lines(
    lines: &[String],
    line_num: usize,
    context: usize,
) -> (Vec<String>, Vec<String>) {
    if context == 0 || lines.is_empty() {
        return (vec![], vec![]);
    }
    let idx = line_num.saturating_sub(1);
    let start = idx.saturating_sub(context);
    let end = (idx + context + 1).min(lines.len());

    let before = lines[start..idx].to_vec();
    let after = if idx + 1 < end {
        lines[idx + 1..end].to_vec()
    } else {
        vec![]
    };
    (before, after)
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn stable_result_id_is_deterministic() {
        let a = stable_result_id("src/lib.rs", 10, "fn alpha() {}");
        let b = stable_result_id("src/lib.rs", 10, "fn alpha() {}");
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn persist_and_load_hints_roundtrip() {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();
        let hints = vec![AgentHintInput {
            id: None,
            path: "src/lib.rs".to_string(),
            line: 3,
            snippet: "fn alpha() {}".to_string(),
        }];
        persist_expand_hints(root, hints).expect("persist");
        let map = load_hint_map(root).expect("load");
        assert_eq!(map.len(), 1);
        let only = map.values().next().expect("entry");
        assert_eq!(only.path, "src/lib.rs");
        assert_eq!(only.line, 3);
    }
}
