// SPDX-License-Identifier: MIT OR Apache-2.0

//! MCP server support for cgrep (stdio JSON-RPC).

pub mod install;

use crate::indexer::scanner::is_indexable_extension;
use notify::{
    Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode,
    Watcher as NotifyWatcher,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const PROTOCOL_VERSION: &str = "2024-11-05";
const DEFAULT_MCP_TOOL_TIMEOUT_MS: u64 = 45_000;
const DEFAULT_MCP_TOOL_MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const PIPE_DRAIN_GRACE_MS: u64 = 250;
const MIN_PIPE_DRAIN_WAIT_MS: u64 = 1_000;
const AUTO_INDEX_FAILURE_TTL_MS: u64 = 60_000;
const AUTO_INDEX_REFRESH_DEBOUNCE_MS: u64 = 500;
const AUTO_INDEX_REFRESH_FAILURE_TTL_MS: u64 = 60_000;
const AUTO_INDEX_WATCH_POLL_INTERVAL_MS: u64 = 1_500;
const AUTO_INDEX_SCOPE_IDLE_TTL_MS: u64 = 15 * 60_000;
static AUTO_INDEX_FAILURES: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
static AUTO_INDEX_SCOPE_STATES: OnceLock<Mutex<HashMap<String, AutoIndexScopeState>>> =
    OnceLock::new();

// Keep harness guidance close to the server so every MCP host gets the same behavior.
const HARNESS_INSTRUCTIONS: &str = "\
cgrep MCP harness (search/navigation only).\n\
\n\
Use cgrep tools instead of host built-in search/read tools for repository navigation.\n\
\n\
Recommended workflow:\n\
1) cgrep_map for structure\n\
2) cgrep_agent_locate for low-token candidate IDs\n\
3) cgrep_agent_expand for exact windows on selected IDs\n\
4) cgrep_search/cgrep_read only when locate/expand is insufficient\n\
5) cgrep_definition/cgrep_references/cgrep_callers for symbol relationships\n\
\n\
Harness rules:\n\
- Prefer structured tool calls with explicit arguments.\n\
- Keep calls deterministic: tools return JSON (compact) from cgrep CLI.\n\
- Narrow scope/path early to reduce retries and token churn.\n\
- For `cgrep_read` sections, use `start-end` line ranges (`start:end` is accepted and normalized).\n\
- Use `cgrep_read.path` for one file or `cgrep_read.paths` for batched reads.\n\
- Use tool-specific filters before widening scope:\n\
  cgrep_search(path/glob/exclude/changed/mode/budget/limit/context),\n\
  cgrep_symbols(symbol_type/lang/file_type/glob/exclude/changed),\n\
  cgrep_definition(path/limit), cgrep_references(path/limit/changed/mode),\n\
  cgrep_index(exclude_paths/include_paths/include_ignored/high_memory).\n\
- For edits, use your host's edit tool after locating exact targets with cgrep.\n\
\n\
This server is read/search oriented; it does not mutate files.";

pub fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let req = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => req,
            Err(err) => {
                write_error(&mut stdout, None, -32700, &format!("parse error: {}", err))?;
                continue;
            }
        };

        // JSON-RPC notifications have no id; no response needed.
        if req.id.is_none() {
            continue;
        }

        let resp = handle_request(&req);
        serde_json::to_writer(&mut stdout, &resp)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

fn handle_request(req: &JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "cgrep",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "instructions": HARNESS_INSTRUCTIONS
            })),
            error: None,
        },
        "ping" => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(json!({})),
            error: None,
        },
        "tools/list" => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(json!({
                "tools": tool_definitions()
            })),
            error: None,
        },
        "tools/call" => handle_tool_call(req),
        _ => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("method not found: {}", req.method),
            }),
        },
    }
}

fn handle_tool_call(req: &JsonRpcRequest) -> JsonRpcResponse {
    let params = &req.params;
    let tool_name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let args = params.get("arguments").unwrap_or(&Value::Null);

    let result = dispatch_tool(tool_name, args);
    match result {
        Ok(output) => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(json!({
                "content": [{
                    "type": "text",
                    "text": output
                }]
            })),
            error: None,
        },
        Err(err) => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(json!({
                "content": [{
                    "type": "text",
                    "text": err
                }],
                "isError": true
            })),
            error: None,
        },
    }
}

fn dispatch_tool(tool: &str, args: &Value) -> Result<String, String> {
    match tool {
        "cgrep_search" => tool_search(args),
        "cgrep_agent_locate" => tool_agent_locate(args),
        "cgrep_agent_expand" => tool_agent_expand(args),
        "cgrep_read" => tool_read(args),
        "cgrep_map" => tool_map(args),
        "cgrep_symbols" => tool_symbols(args),
        "cgrep_definition" => tool_definition(args),
        "cgrep_references" => tool_references(args),
        "cgrep_callers" => tool_callers(args),
        "cgrep_dependents" => tool_dependents(args),
        "cgrep_index" => tool_index(args),
        _ => Err(format!("unknown tool: {}", tool)),
    }
}

fn tool_search(args: &Value) -> Result<String, String> {
    let query = required_str(args, "query")?;
    let cwd = opt_cwd(args);
    let path = opt_str(args, "path");
    require_bounded_relative_scope("cgrep_search", cwd, path, true)?;
    let auto_index = opt_bool_value(args, "auto_index").unwrap_or(true);
    let mut bootstrap_index = false;
    let mut force_scan_from_bootstrap = false;
    if auto_index {
        match ensure_index_for_search(cwd, path) {
            Ok(BootstrapOutcome::AlreadyIndexed) => {}
            Ok(BootstrapOutcome::Refreshed) => {}
            Ok(BootstrapOutcome::Bootstrapped) => bootstrap_index = true,
            Ok(BootstrapOutcome::FellBackToScan) => force_scan_from_bootstrap = true,
            Err(err) => return Err(err),
        }
    }

    let mut cmd = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
        "search".to_string(),
    ];

    push_opt_flag_value(&mut cmd, "-p", path);
    push_opt_flag_value_u64(&mut cmd, "-m", opt_u64(args, "limit"));
    push_opt_flag_value_u64(&mut cmd, "-C", opt_u64(args, "context"));
    push_opt_flag_value(&mut cmd, "-t", opt_str(args, "file_type"));
    push_opt_flag_value(&mut cmd, "--glob", opt_str(args, "glob"));
    push_opt_flag_value(&mut cmd, "--exclude", opt_str(args, "exclude"));
    push_opt_flag_value(
        &mut cmd,
        "-B",
        Some(opt_str(args, "budget").unwrap_or("balanced")),
    );
    push_opt_flag_value_u64(
        &mut cmd,
        "--max-total-chars",
        opt_u64(args, "max_total_chars"),
    );
    push_opt_flag_value_u64(
        &mut cmd,
        "--max-chars-per-snippet",
        opt_u64(args, "max_chars_per_snippet"),
    );
    push_opt_flag_value_u64(
        &mut cmd,
        "--max-context-chars",
        opt_u64(args, "max_context_chars"),
    );
    push_opt_flag_value(&mut cmd, "-P", opt_str(args, "profile"));
    push_opt_flag_value(&mut cmd, "--mode", opt_str(args, "mode"));
    push_changed(&mut cmd, args.get("changed"));
    push_bool_flag(
        &mut cmd,
        "--dedupe-context",
        opt_bool_value(args, "dedupe_context").unwrap_or(true),
    );
    push_bool_flag(
        &mut cmd,
        "--path-alias",
        opt_bool_value(args, "path_alias").unwrap_or(true),
    );
    push_bool_flag(
        &mut cmd,
        "--suppress-boilerplate",
        opt_bool_value(args, "suppress_boilerplate").unwrap_or(true),
    );
    push_bool_flag(&mut cmd, "--regex", opt_bool(args, "regex"));
    push_bool_flag(
        &mut cmd,
        "--case-sensitive",
        opt_bool(args, "case_sensitive"),
    );
    push_bool_flag(
        &mut cmd,
        "--no-index",
        opt_bool(args, "no_index") || force_scan_from_bootstrap,
    );
    push_bool_flag(&mut cmd, "--no-recursive", opt_bool(args, "no_recursive"));
    push_bool_flag(&mut cmd, "--no-ignore", opt_bool(args, "no_ignore"));
    push_bool_flag(&mut cmd, "--fuzzy", opt_bool(args, "fuzzy"));
    push_bool_flag(&mut cmd, "-q", opt_bool(args, "quiet"));
    push_bool_flag(&mut cmd, "--bootstrap-index", bootstrap_index);
    cmd.push("--".to_string());
    cmd.push(query.to_string());

    run_cgrep(&cmd, cwd)
}

fn tool_agent_locate(args: &Value) -> Result<String, String> {
    let query = required_str(args, "query")?;
    let cwd = opt_cwd(args);
    let path = opt_str(args, "path");
    require_bounded_relative_scope("cgrep_agent_locate", cwd, path, true)?;
    maybe_prepare_auto_index(args, cwd, path)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
        "agent".to_string(),
        "locate".to_string(),
        query.to_string(),
    ];
    push_opt_flag_value(&mut cmd, "-p", path);
    push_changed(&mut cmd, args.get("changed"));
    push_opt_flag_value_u64(&mut cmd, "--limit", opt_u64(args, "limit"));
    push_opt_flag_value(&mut cmd, "--mode", opt_str(args, "mode"));
    push_opt_flag_value(
        &mut cmd,
        "-B",
        Some(opt_str(args, "budget").unwrap_or("balanced")),
    );
    run_cgrep(&cmd, cwd)
}

fn tool_agent_expand(args: &Value) -> Result<String, String> {
    let ids = required_array_str(args, "ids")?;
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_agent_expand", cwd, opt_str(args, "path"), true)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
        "agent".to_string(),
        "expand".to_string(),
    ];
    for id in ids {
        cmd.push("--id".to_string());
        cmd.push(id);
    }
    push_opt_flag_value(&mut cmd, "-p", opt_str(args, "path"));
    push_opt_flag_value_u64(&mut cmd, "-C", opt_u64(args, "context"));
    run_cgrep(&cmd, cwd)
}

fn tool_read(args: &Value) -> Result<String, String> {
    let cwd = opt_cwd(args);
    let paths = read_paths(args)?;
    let section = resolve_read_section(args)?;
    let full = opt_bool(args, "full");

    for path in &paths {
        require_bounded_relative_scope("cgrep_read", cwd, Some(path.as_str()), false)?;
    }

    if paths.len() == 1 {
        return run_read_for_path(paths[0].as_str(), section.as_deref(), full, cwd);
    }

    let mut results: Vec<Value> = Vec::with_capacity(paths.len());
    for path in paths {
        let output = run_read_for_path(path.as_str(), section.as_deref(), full, cwd)?;
        let parsed =
            serde_json::from_str::<Value>(&output).unwrap_or_else(|_| json!({ "raw": output }));
        results.push(json!({
            "path": path,
            "read": parsed
        }));
    }

    serde_json::to_string(&json!({
        "meta": {
            "schema_version": "1",
            "tool": "cgrep_read",
            "batched": true,
            "count": results.len()
        },
        "results": results
    }))
    .map_err(|err| format!("failed to encode batched read response: {err}"))
}

fn run_read_for_path(
    path: &str,
    section: Option<&str>,
    full: bool,
    cwd: Option<&str>,
) -> Result<String, String> {
    let mut cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "read".to_string(),
        path.to_string(),
    ];
    push_opt_flag_value(&mut cmd, "--section", section);
    push_bool_flag(&mut cmd, "--full", full);
    run_cgrep(&cmd, cwd)
}

fn tool_map(args: &Value) -> Result<String, String> {
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_map", cwd, opt_str(args, "path"), true)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "map".to_string(),
    ];
    push_opt_flag_value(&mut cmd, "-p", opt_str(args, "path"));
    push_opt_flag_value_u64(&mut cmd, "--depth", opt_u64(args, "depth"));
    run_cgrep(&cmd, cwd)
}

fn tool_symbols(args: &Value) -> Result<String, String> {
    let name = required_str(args, "name")?;
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_symbols", cwd, None, true)?;
    maybe_prepare_auto_index(args, cwd, None)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "symbols".to_string(),
        name.to_string(),
    ];
    push_opt_flag_value(&mut cmd, "-T", opt_str(args, "symbol_type"));
    push_opt_flag_value(&mut cmd, "--lang", opt_str(args, "lang"));
    push_opt_flag_value(&mut cmd, "--file-type", opt_str(args, "file_type"));
    push_opt_flag_value(&mut cmd, "--glob", opt_str(args, "glob"));
    push_opt_flag_value(&mut cmd, "--exclude", opt_str(args, "exclude"));
    push_changed(&mut cmd, args.get("changed"));
    push_bool_flag(&mut cmd, "-q", opt_bool(args, "quiet"));
    run_cgrep(&cmd, cwd)
}

fn tool_definition(args: &Value) -> Result<String, String> {
    let name = required_str(args, "name")?;
    let cwd = opt_cwd(args);
    let path = opt_str(args, "path");
    require_bounded_relative_scope("cgrep_definition", cwd, path, true)?;
    maybe_prepare_auto_index(args, cwd, path)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "definition".to_string(),
        name.to_string(),
    ];
    push_opt_flag_value(&mut cmd, "-p", path);
    push_opt_flag_value_u64(&mut cmd, "--limit", opt_u64(args, "limit"));
    run_cgrep(&cmd, cwd)
}

fn tool_references(args: &Value) -> Result<String, String> {
    let name = required_str(args, "name")?;
    let cwd = opt_cwd(args);
    let path = opt_str(args, "path");
    require_bounded_relative_scope("cgrep_references", cwd, path, true)?;
    maybe_prepare_auto_index(args, cwd, path)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "references".to_string(),
        name.to_string(),
    ];
    push_opt_flag_value(&mut cmd, "-p", path);
    push_opt_flag_value_u64(&mut cmd, "--limit", opt_u64(args, "limit"));
    push_changed(&mut cmd, args.get("changed"));
    push_opt_flag_value(&mut cmd, "--mode", opt_str(args, "mode"));
    run_cgrep(&cmd, cwd)
}

fn tool_callers(args: &Value) -> Result<String, String> {
    let function = required_str(args, "function")?;
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_callers", cwd, None, true)?;
    maybe_prepare_auto_index(args, cwd, None)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "callers".to_string(),
        function.to_string(),
    ];
    push_opt_flag_value(&mut cmd, "--mode", opt_str(args, "mode"));
    run_cgrep(&cmd, cwd)
}

fn tool_dependents(args: &Value) -> Result<String, String> {
    let file = required_str(args, "file")?;
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_dependents", cwd, Some(file), false)?;
    let dependents_scope = Path::new(file)
        .parent()
        .and_then(|parent| parent.to_str())
        .filter(|parent| !parent.is_empty() && *parent != ".");
    maybe_prepare_auto_index(args, cwd, dependents_scope)?;
    let cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "dependents".to_string(),
        file.to_string(),
    ];
    run_cgrep(&cmd, cwd)
}

fn tool_index(args: &Value) -> Result<String, String> {
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_index", cwd, opt_str(args, "path"), true)?;
    let mut cmd = vec!["index".to_string()];
    push_opt_flag_value(&mut cmd, "-p", opt_str(args, "path"));
    push_bool_flag(&mut cmd, "--force", opt_bool(args, "force"));
    push_bool_flag(&mut cmd, "--high-memory", opt_bool(args, "high_memory"));
    push_bool_flag(
        &mut cmd,
        "--include-ignored",
        opt_bool(args, "include_ignored"),
    );
    push_opt_flag_value(&mut cmd, "--embeddings", opt_str(args, "embeddings"));

    if let Some(excludes) = opt_array_str(args, "exclude_paths") {
        for pattern in excludes {
            cmd.push("--exclude".to_string());
            cmd.push(pattern.to_string());
        }
    }
    if let Some(includes) = opt_array_str(args, "include_paths") {
        for path in includes {
            cmd.push("--include-path".to_string());
            cmd.push(path.to_string());
        }
    }

    run_cgrep(&cmd, cwd)
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing required parameter: {}", key))
}

fn required_array_str(args: &Value, key: &str) -> Result<Vec<String>, String> {
    let values = args
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing required parameter: {}", key))?;
    let out = values
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if out.is_empty() {
        return Err(format!(
            "parameter `{}` must contain at least one string",
            key
        ));
    }
    Ok(out)
}

fn read_paths(args: &Value) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();

    if let Some(path) = args.get("path").and_then(Value::as_str) {
        if path.trim().is_empty() {
            return Err("Path cannot be empty".to_string());
        }
        out.push(path.to_string());
    }

    if let Some(paths) = opt_array_str(args, "paths") {
        for path in paths {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
        }
    }

    if out.is_empty() {
        return Err("missing required parameter: path (or non-empty paths[])".to_string());
    }

    let mut seen = HashSet::new();
    out.retain(|path| seen.insert(path.clone()));
    Ok(out)
}

fn resolve_read_section(args: &Value) -> Result<Option<String>, String> {
    let section_start = opt_u64(args, "section_start");
    let section_end = opt_u64(args, "section_end");

    if section_start.is_some() != section_end.is_some() {
        return Err("`section_start` and `section_end` must be provided together".to_string());
    }

    if let (Some(start), Some(end)) = (section_start, section_end) {
        return Ok(Some(format!("{start}-{end}")));
    }

    let section = opt_str(args, "section")
        .map(str::trim)
        .filter(|value| !value.is_empty());
    Ok(section.map(normalize_section_range))
}

fn normalize_section_range(value: &str) -> String {
    let trimmed = value.trim();
    if let Some((start, end)) = trimmed.split_once(':') {
        let start = start.trim();
        let end = end.trim();
        if !start.is_empty()
            && !end.is_empty()
            && start.chars().all(|c| c.is_ascii_digit())
            && end.chars().all(|c| c.is_ascii_digit())
        {
            return format!("{start}-{end}");
        }
    }
    trimmed.to_string()
}

fn opt_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn opt_u64(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(Value::as_u64)
}

fn opt_bool(args: &Value, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn opt_bool_value(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

fn opt_array_str<'a>(args: &'a Value, key: &str) -> Option<Vec<&'a str>> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|vals| vals.iter().filter_map(Value::as_str).collect::<Vec<_>>())
}

fn opt_cwd(args: &Value) -> Option<&str> {
    opt_str(args, "cwd").filter(|value| !value.trim().is_empty())
}

fn require_bounded_relative_scope(
    tool_name: &str,
    cwd: Option<&str>,
    path_value: Option<&str>,
    defaults_to_cwd: bool,
) -> Result<(), String> {
    if cwd.is_some() {
        return Ok(());
    }

    let resolves_from_server_cwd = match path_value {
        Some(path) => !Path::new(path).is_absolute(),
        None => defaults_to_cwd,
    };
    if !resolves_from_server_cwd {
        return Ok(());
    }

    let server_cwd =
        std::env::current_dir().map_err(|err| format!("failed to resolve server cwd: {err}"))?;
    if server_cwd == Path::new("/") {
        return Err(format!(
            "{tool_name} requires `cwd` (or an absolute `path`) when server cwd is `/` to avoid scanning the system root"
        ));
    }

    Ok(())
}

struct AutoIndexScopeState {
    dirty: Arc<AtomicBool>,
    has_watcher: bool,
    // Hold watcher lifetime for this scope. Dropping it stops watching.
    _watcher: Option<RecommendedWatcher>,
    last_seen_at: Instant,
    last_refresh_attempt_at: Option<Instant>,
    last_refresh_failure_at: Option<Instant>,
}

impl AutoIndexScopeState {
    fn new(scope: &Path) -> Self {
        let dirty = Arc::new(AtomicBool::new(true));
        let watcher = create_scope_watcher(scope, Arc::clone(&dirty));
        Self {
            dirty,
            has_watcher: watcher.is_some(),
            _watcher: watcher,
            last_seen_at: Instant::now(),
            last_refresh_attempt_at: None,
            last_refresh_failure_at: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootstrapOutcome {
    AlreadyIndexed,
    Refreshed,
    Bootstrapped,
    FellBackToScan,
}

fn maybe_prepare_auto_index(
    args: &Value,
    cwd: Option<&str>,
    path: Option<&str>,
) -> Result<(), String> {
    if !opt_bool_value(args, "auto_index").unwrap_or(true) {
        return Ok(());
    }
    let _ = ensure_index_for_search(cwd, path)?;
    Ok(())
}

fn ensure_index_for_search(
    cwd: Option<&str>,
    path: Option<&str>,
) -> Result<BootstrapOutcome, String> {
    let search_root = resolve_search_root(cwd, path)?;
    let existing_index_root = cgrep::utils::find_index_root(&search_root);
    let index_scope = existing_index_root
        .as_ref()
        .map(|root| root.root.clone())
        .unwrap_or_else(|| search_root.clone());
    if existing_index_root.is_some() {
        clear_bootstrap_failure(&index_scope);
        if maybe_refresh_existing_index(cwd, &index_scope)? {
            return Ok(BootstrapOutcome::Refreshed);
        }
        return Ok(BootstrapOutcome::AlreadyIndexed);
    }
    if recently_failed_bootstrap(&index_scope) {
        return Ok(BootstrapOutcome::FellBackToScan);
    }

    match run_index_for_scope(cwd, &index_scope) {
        Ok(_) => {
            clear_bootstrap_failure(&index_scope);
            mark_scope_indexed(&index_scope);
            Ok(BootstrapOutcome::Bootstrapped)
        }
        Err(_) => {
            record_bootstrap_failure(&index_scope);
            Ok(BootstrapOutcome::FellBackToScan)
        }
    }
}

fn resolve_search_root(cwd: Option<&str>, path: Option<&str>) -> Result<PathBuf, String> {
    let base = match cwd {
        Some(raw) => PathBuf::from(raw),
        None => std::env::current_dir().map_err(|err| format!("failed to resolve cwd: {err}"))?,
    };
    let requested = path.map(PathBuf::from).unwrap_or_else(|| base.clone());
    let mut absolute = if requested.is_absolute() {
        requested
    } else {
        base.join(requested)
    };
    if let Ok(canonical) = absolute.canonicalize() {
        absolute = canonical;
    }
    Ok(absolute)
}

fn failure_cache() -> &'static Mutex<HashMap<String, Instant>> {
    AUTO_INDEX_FAILURES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn scope_state_cache() -> &'static Mutex<HashMap<String, AutoIndexScopeState>> {
    AUTO_INDEX_SCOPE_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn recently_failed_bootstrap(search_root: &Path) -> bool {
    let key = search_root.display().to_string();
    let ttl = Duration::from_millis(AUTO_INDEX_FAILURE_TTL_MS);
    let now = Instant::now();
    let mut cache = failure_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.retain(|_, at| now.duration_since(*at) <= ttl);
    cache.contains_key(&key)
}

fn record_bootstrap_failure(search_root: &Path) {
    let key = search_root.display().to_string();
    let mut cache = failure_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.insert(key, Instant::now());
}

fn clear_bootstrap_failure(search_root: &Path) {
    let key = search_root.display().to_string();
    let mut cache = failure_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.remove(&key);
}

fn maybe_refresh_existing_index(cwd: Option<&str>, index_scope: &Path) -> Result<bool, String> {
    if !should_attempt_index_refresh(index_scope) {
        return Ok(false);
    }
    match run_index_for_scope(cwd, index_scope) {
        Ok(_) => {
            record_scope_refresh_result(index_scope, true);
            Ok(true)
        }
        Err(_) => {
            record_scope_refresh_result(index_scope, false);
            Ok(false)
        }
    }
}

fn should_attempt_index_refresh(index_scope: &Path) -> bool {
    let now = Instant::now();
    let key = index_scope.display().to_string();
    let refresh_debounce = Duration::from_millis(AUTO_INDEX_REFRESH_DEBOUNCE_MS);
    let refresh_failure_ttl = Duration::from_millis(AUTO_INDEX_REFRESH_FAILURE_TTL_MS);

    let mut cache = scope_state_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prune_idle_scope_states(&mut cache, now);

    let state = cache
        .entry(key)
        .or_insert_with(|| AutoIndexScopeState::new(index_scope));
    state.last_seen_at = now;

    if state
        .last_refresh_attempt_at
        .is_some_and(|at| now.duration_since(at) < refresh_debounce)
    {
        return false;
    }
    if state
        .last_refresh_failure_at
        .is_some_and(|at| now.duration_since(at) < refresh_failure_ttl)
    {
        return false;
    }

    let should_refresh = if state.has_watcher {
        state.dirty.load(Ordering::Acquire)
    } else {
        // If watcher setup fails, keep correctness by refreshing opportunistically.
        true
    };
    if !should_refresh {
        return false;
    }

    state.last_refresh_attempt_at = Some(now);
    true
}

fn mark_scope_indexed(index_scope: &Path) {
    let now = Instant::now();
    let key = index_scope.display().to_string();
    let mut cache = scope_state_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prune_idle_scope_states(&mut cache, now);
    let state = cache
        .entry(key)
        .or_insert_with(|| AutoIndexScopeState::new(index_scope));
    state.last_seen_at = now;
    state.dirty.store(false, Ordering::Release);
    state.last_refresh_failure_at = None;
}

fn record_scope_refresh_result(index_scope: &Path, success: bool) {
    let now = Instant::now();
    let key = index_scope.display().to_string();
    let mut cache = scope_state_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    prune_idle_scope_states(&mut cache, now);
    let state = cache
        .entry(key)
        .or_insert_with(|| AutoIndexScopeState::new(index_scope));
    state.last_seen_at = now;
    if success {
        state.dirty.store(false, Ordering::Release);
        state.last_refresh_failure_at = None;
    } else {
        state.last_refresh_failure_at = Some(now);
    }
}

fn prune_idle_scope_states(cache: &mut HashMap<String, AutoIndexScopeState>, now: Instant) {
    let idle_ttl = Duration::from_millis(AUTO_INDEX_SCOPE_IDLE_TTL_MS);
    cache.retain(|_, state| now.duration_since(state.last_seen_at) <= idle_ttl);
}

fn create_scope_watcher(index_scope: &Path, dirty: Arc<AtomicBool>) -> Option<RecommendedWatcher> {
    let watch_root = index_scope.to_path_buf();
    let callback_root = watch_root.clone();
    let callback_dirty = Arc::clone(&dirty);
    let config = NotifyConfig::default()
        .with_poll_interval(Duration::from_millis(AUTO_INDEX_WATCH_POLL_INTERVAL_MS));
    let mut watcher = match RecommendedWatcher::new(
        move |event: Result<Event, notify::Error>| {
            if let Ok(event) = event {
                if should_mark_scope_dirty(&callback_root, &event) {
                    callback_dirty.store(true, Ordering::Release);
                }
            }
        },
        config,
    ) {
        Ok(watcher) => watcher,
        Err(_) => return None,
    };
    if watcher
        .watch(&watch_root, RecursiveMode::Recursive)
        .is_err()
    {
        return None;
    }
    Some(watcher)
}

fn should_mark_scope_dirty(scope_root: &Path, event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && event
        .paths
        .iter()
        .any(|path| should_track_auto_index_path(scope_root, path))
}

fn should_track_auto_index_path(scope_root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(scope_root).unwrap_or(path);
    if relative.as_os_str().is_empty() {
        return false;
    }

    for component in relative.components() {
        if let Component::Normal(name) = component {
            let Some(name) = name.to_str() else { continue };
            if matches!(name, ".cgrep" | ".git" | ".hg" | ".svn") {
                return false;
            }
        }
    }

    let file_name = relative.file_name().and_then(|f| f.to_str()).unwrap_or("");
    if file_name.starts_with('.')
        || file_name.starts_with(".#")
        || file_name.ends_with('~')
        || file_name.ends_with(".tmp")
        || file_name.ends_with(".swp")
        || file_name.ends_with(".swo")
    {
        return false;
    }

    let Some(ext) = relative.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    is_indexable_extension(ext)
}

fn run_index_for_scope(cwd: Option<&str>, scope: &Path) -> Result<String, String> {
    let cmd = vec![
        "index".to_string(),
        "-p".to_string(),
        scope.display().to_string(),
        "--embeddings".to_string(),
        "off".to_string(),
    ];
    run_cgrep(&cmd, cwd)
}

fn push_opt_flag_value(cmd: &mut Vec<String>, flag: &str, value: Option<&str>) {
    if let Some(value) = value {
        cmd.push(flag.to_string());
        cmd.push(value.to_string());
    }
}

fn push_opt_flag_value_u64(cmd: &mut Vec<String>, flag: &str, value: Option<u64>) {
    if let Some(value) = value {
        cmd.push(flag.to_string());
        cmd.push(value.to_string());
    }
}

fn push_bool_flag(cmd: &mut Vec<String>, flag: &str, enabled: bool) {
    if enabled {
        cmd.push(flag.to_string());
    }
}

fn push_changed(cmd: &mut Vec<String>, value: Option<&Value>) {
    match value {
        Some(Value::Bool(true)) => {
            cmd.push("--changed".to_string());
        }
        Some(Value::String(rev)) if !rev.is_empty() => {
            cmd.push("--changed".to_string());
            cmd.push(rev.to_string());
        }
        _ => {}
    }
}

fn run_cgrep(args: &[String], cwd: Option<&str>) -> Result<String, String> {
    let exe =
        std::env::current_exe().map_err(|e| format!("failed to resolve executable: {}", e))?;
    let mut command = Command::new(exe);
    command
        .args(args)
        .env("CGREP_DISABLE_CLI_AUTO_INDEX", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to execute cgrep: {}", e))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture cgrep stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture cgrep stderr".to_string())?;
    let max_output_bytes = mcp_tool_max_output_bytes();
    let stdout_reader = spawn_pipe_reader(stdout, "stdout", max_output_bytes);
    let stderr_reader = spawn_pipe_reader(stderr, "stderr", max_output_bytes);

    let timeout = mcp_tool_timeout();
    let started_at = Instant::now();
    let mut captured_stdout: Option<Vec<u8>> = None;
    let mut captured_stderr: Option<Vec<u8>> = None;

    let status = loop {
        if captured_stdout.is_none() {
            match poll_pipe_reader(&stdout_reader, "stdout") {
                Ok(Some(bytes)) => captured_stdout = Some(bytes),
                Ok(None) => {}
                Err(err) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    drain_pipe_reader(&stdout_reader);
                    drain_pipe_reader(&stderr_reader);
                    return Err(err);
                }
            }
        }
        if captured_stderr.is_none() {
            match poll_pipe_reader(&stderr_reader, "stderr") {
                Ok(Some(bytes)) => captured_stderr = Some(bytes),
                Ok(None) => {}
                Err(err) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    drain_pipe_reader(&stdout_reader);
                    drain_pipe_reader(&stderr_reader);
                    return Err(err);
                }
            }
        }

        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    drain_pipe_reader(&stdout_reader);
                    drain_pipe_reader(&stderr_reader);
                    return Err(format!(
                        "cgrep MCP tool call timed out after {}ms. Retry with narrower scope (`path`, `glob`, or `changed`).",
                        timeout.as_millis()
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                drain_pipe_reader(&stdout_reader);
                drain_pipe_reader(&stderr_reader);
                return Err(format!("failed to wait for cgrep: {err}"));
            }
        }
    };

    let pipe_wait = output_drain_wait(timeout, started_at);
    let stdout_bytes = match captured_stdout.take() {
        Some(bytes) => bytes,
        None => wait_pipe_reader(&stdout_reader, "stdout", pipe_wait)?,
    };
    let stderr_bytes = match captured_stderr.take() {
        Some(bytes) => bytes,
        None => wait_pipe_reader(&stderr_reader, "stderr", pipe_wait)?,
    };
    let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

    if status.success() {
        Ok(stdout.trim_end().to_string())
    } else {
        let mut msg = String::new();
        if !stderr.trim().is_empty() {
            msg.push_str(stderr.trim());
        }
        if !stdout.trim().is_empty() {
            if !msg.is_empty() {
                msg.push('\n');
            }
            msg.push_str(stdout.trim());
        }
        if msg.is_empty() {
            msg = format!("cgrep exited with status {status}");
        }
        Err(msg)
    }
}

fn spawn_pipe_reader<R>(
    mut pipe: R,
    stream: &'static str,
    max_output_bytes: usize,
) -> mpsc::Receiver<Result<Vec<u8>, String>>
where
    R: Read + Send + 'static,
{
    let (tx, rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = read_pipe_with_limit(&mut pipe, stream, max_output_bytes);
        let _ = tx.send(result);
    });
    rx
}

fn read_pipe_with_limit<R: Read>(
    pipe: &mut R,
    stream: &'static str,
    max_output_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = pipe
            .read(&mut chunk)
            .map_err(|err| format!("failed to read cgrep {stream}: {err}"))?;
        if read == 0 {
            break;
        }
        if out.len().saturating_add(read) > max_output_bytes {
            return Err(format!(
                "cgrep MCP tool call output exceeded {} bytes. Retry with narrower scope (`path`, `glob`, or `changed`).",
                max_output_bytes
            ));
        }
        out.extend_from_slice(&chunk[..read]);
    }
    Ok(out)
}

fn poll_pipe_reader(
    reader: &mpsc::Receiver<Result<Vec<u8>, String>>,
    stream: &'static str,
) -> Result<Option<Vec<u8>>, String> {
    match reader.try_recv() {
        Ok(Ok(bytes)) => Ok(Some(bytes)),
        Ok(Err(err)) => Err(err),
        Err(mpsc::TryRecvError::Empty) => Ok(None),
        Err(mpsc::TryRecvError::Disconnected) => {
            Err(format!("failed to receive cgrep {stream} output"))
        }
    }
}

fn wait_pipe_reader(
    reader: &mpsc::Receiver<Result<Vec<u8>, String>>,
    stream: &'static str,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    match reader.recv_timeout(timeout) {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(err)) => Err(err),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(format!("timed out while draining cgrep {stream} output"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(format!("failed to receive cgrep {stream} output"))
        }
    }
}

fn drain_pipe_reader(reader: &mpsc::Receiver<Result<Vec<u8>, String>>) {
    let _ = reader.recv_timeout(Duration::from_millis(PIPE_DRAIN_GRACE_MS));
}

fn output_drain_wait(timeout: Duration, started_at: Instant) -> Duration {
    let remaining = timeout.saturating_sub(started_at.elapsed());
    remaining.max(Duration::from_millis(MIN_PIPE_DRAIN_WAIT_MS))
}

fn mcp_tool_timeout() -> Duration {
    let timeout_ms = std::env::var("CGREP_MCP_TOOL_TIMEOUT_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_MCP_TOOL_TIMEOUT_MS);
    Duration::from_millis(timeout_ms)
}

fn mcp_tool_max_output_bytes() -> usize {
    std::env::var("CGREP_MCP_TOOL_MAX_OUTPUT_BYTES")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MCP_TOOL_MAX_OUTPUT_BYTES)
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "cgrep_search",
            "description": "Full-text code search with deterministic JSON2 output.",
            "inputSchema": {
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string", "description": "Literal query text to search for." },
                    "path": { "type": "string", "description": "Optional scope root for this search." },
                    "cwd": { "type": "string", "description": "Working directory used to resolve relative paths." },
                    "limit": { "type": "number" },
                    "context": { "type": "number" },
                    "file_type": { "type": "string" },
                    "glob": { "type": "string" },
                    "exclude": { "type": "string" },
                    "budget": { "type": "string", "enum": ["tight", "balanced", "full", "off"] },
                    "max_total_chars": { "type": "number" },
                    "max_chars_per_snippet": { "type": "number" },
                    "max_context_chars": { "type": "number" },
                    "dedupe_context": { "type": "boolean" },
                    "path_alias": { "type": "boolean" },
                    "suppress_boilerplate": { "type": "boolean" },
                    "auto_index": { "type": "boolean" },
                    "changed": { "oneOf": [{ "type": "boolean" }, { "type": "string" }] },
                    "mode": { "type": "string", "enum": ["keyword", "semantic", "hybrid"] },
                    "profile": { "type": "string" },
                    "regex": { "type": "boolean" },
                    "case_sensitive": { "type": "boolean" },
                    "no_index": { "type": "boolean" },
                    "no_recursive": { "type": "boolean" },
                    "no_ignore": { "type": "boolean" },
                    "quiet": { "type": "boolean" },
                    "fuzzy": { "type": "boolean" }
                }
            }
        }),
        json!({
            "name": "cgrep_agent_locate",
            "description": "Stage 1 low-token retrieval: locate candidate IDs.",
            "inputSchema": {
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string" },
                    "path": { "type": "string" },
                    "cwd": { "type": "string" },
                    "auto_index": { "type": "boolean" },
                    "changed": { "oneOf": [{ "type": "boolean" }, { "type": "string" }] },
                    "limit": { "type": "number" },
                    "mode": { "type": "string", "enum": ["keyword", "semantic", "hybrid"] },
                    "budget": { "type": "string", "enum": ["tight", "balanced", "full", "off"] }
                }
            }
        }),
        json!({
            "name": "cgrep_agent_expand",
            "description": "Stage 2 retrieval: expand selected locate IDs with context.",
            "inputSchema": {
                "type": "object",
                "required": ["ids"],
                "properties": {
                    "ids": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "path": { "type": "string" },
                    "cwd": { "type": "string" },
                    "context": { "type": "number" }
                }
            }
        }),
        json!({
            "name": "cgrep_read",
            "description": "Read a file with smart full/outline behavior.",
            "inputSchema": {
                "type": "object",
                "oneOf": [
                    { "required": ["path"] },
                    { "required": ["paths"] }
                ],
                "properties": {
                    "path": { "type": "string", "description": "Single file path to read." },
                    "paths": { "type": "array", "items": { "type": "string" }, "description": "Optional batched file paths; each path is read independently." },
                    "cwd": { "type": "string", "description": "Working directory used to resolve relative paths." },
                    "section": { "type": "string", "description": "Line range (`start-end`) or heading text. Numeric `start:end` is also accepted." },
                    "section_start": { "type": "number", "description": "Optional start line number for range reads (use with section_end)." },
                    "section_end": { "type": "number", "description": "Optional end line number for range reads (use with section_start)." },
                    "full": { "type": "boolean" }
                }
            }
        }),
        json!({
            "name": "cgrep_map",
            "description": "Print a structural map of the codebase.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "cwd": { "type": "string" },
                    "path": { "type": "string" },
                    "depth": { "type": "number" }
                }
            }
        }),
        json!({
            "name": "cgrep_symbols",
            "description": "Find symbols by name and optional filters.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "cwd": { "type": "string" },
                    "auto_index": { "type": "boolean" },
                    "symbol_type": { "type": "string" },
                    "lang": { "type": "string" },
                    "file_type": { "type": "string" },
                    "glob": { "type": "string" },
                    "exclude": { "type": "string" },
                    "changed": { "oneOf": [{ "type": "boolean" }, { "type": "string" }] },
                    "quiet": { "type": "boolean" }
                }
            }
        }),
        json!({
            "name": "cgrep_definition",
            "description": "Find definition location for a symbol.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "cwd": { "type": "string" },
                    "path": { "type": "string" },
                    "auto_index": { "type": "boolean" },
                    "limit": { "type": "number" }
                }
            }
        }),
        json!({
            "name": "cgrep_references",
            "description": "Find references to a symbol.",
            "inputSchema": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "cwd": { "type": "string" },
                    "path": { "type": "string" },
                    "auto_index": { "type": "boolean" },
                    "limit": { "type": "number" },
                    "changed": { "oneOf": [{ "type": "boolean" }, { "type": "string" }] },
                    "mode": { "type": "string", "enum": ["auto", "regex", "ast"] }
                }
            }
        }),
        json!({
            "name": "cgrep_callers",
            "description": "Find call sites for a function.",
            "inputSchema": {
                "type": "object",
                "required": ["function"],
                "properties": {
                    "function": { "type": "string" },
                    "cwd": { "type": "string" },
                    "auto_index": { "type": "boolean" },
                    "mode": { "type": "string", "enum": ["auto", "regex", "ast"] }
                }
            }
        }),
        json!({
            "name": "cgrep_dependents",
            "description": "Find files depending on a target file.",
            "inputSchema": {
                "type": "object",
                "required": ["file"],
                "properties": {
                    "file": { "type": "string" },
                    "cwd": { "type": "string" },
                    "auto_index": { "type": "boolean" }
                }
            }
        }),
        json!({
            "name": "cgrep_index",
            "description": "Build or refresh the local cgrep index.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "cwd": { "type": "string" },
                    "force": { "type": "boolean" },
                    "high_memory": { "type": "boolean" },
                    "include_ignored": { "type": "boolean" },
                    "embeddings": { "type": "string", "enum": ["off", "auto", "precompute"] },
                    "exclude_paths": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "include_paths": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }
        }),
    ]
}

fn write_error(w: &mut impl Write, id: Option<Value>, code: i32, message: &str) -> io::Result<()> {
    let resp = JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
        }),
    };
    serde_json::to_writer(&mut *w, &resp)?;
    w.write_all(b"\n")?;
    w.flush()
}
