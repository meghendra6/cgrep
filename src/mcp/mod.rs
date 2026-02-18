// SPDX-License-Identifier: MIT OR Apache-2.0

//! MCP server support for cgrep (stdio JSON-RPC).

pub mod install;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const PROTOCOL_VERSION: &str = "2024-11-05";
const DEFAULT_MCP_TOOL_TIMEOUT_MS: u64 = 45_000;
const AUTO_INDEX_FAILURE_TTL_MS: u64 = 60_000;
static AUTO_INDEX_FAILURES: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

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
    push_bool_flag(&mut cmd, "--fuzzy", opt_bool(args, "fuzzy"));
    push_bool_flag(&mut cmd, "--bootstrap-index", bootstrap_index);
    cmd.push("--".to_string());
    cmd.push(query.to_string());

    run_cgrep(&cmd, cwd)
}

fn tool_agent_locate(args: &Value) -> Result<String, String> {
    let query = required_str(args, "query")?;
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_agent_locate", cwd, opt_str(args, "path"), true)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
        "agent".to_string(),
        "locate".to_string(),
        query.to_string(),
    ];
    push_opt_flag_value(&mut cmd, "-p", opt_str(args, "path"));
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
    let path = required_str(args, "path")?;
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_read", cwd, Some(path), false)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "read".to_string(),
        path.to_string(),
    ];
    push_opt_flag_value(&mut cmd, "--section", opt_str(args, "section"));
    push_bool_flag(&mut cmd, "--full", opt_bool(args, "full"));
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
    run_cgrep(&cmd, cwd)
}

fn tool_definition(args: &Value) -> Result<String, String> {
    let name = required_str(args, "name")?;
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_definition", cwd, None, true)?;
    let cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "definition".to_string(),
        name.to_string(),
    ];
    run_cgrep(&cmd, cwd)
}

fn tool_references(args: &Value) -> Result<String, String> {
    let name = required_str(args, "name")?;
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_references", cwd, opt_str(args, "path"), true)?;
    let mut cmd = vec![
        "--format".to_string(),
        "json".to_string(),
        "--compact".to_string(),
        "references".to_string(),
        name.to_string(),
    ];
    push_opt_flag_value(&mut cmd, "-p", opt_str(args, "path"));
    push_opt_flag_value_u64(&mut cmd, "--limit", opt_u64(args, "limit"));
    push_changed(&mut cmd, args.get("changed"));
    push_opt_flag_value(&mut cmd, "--mode", opt_str(args, "mode"));
    run_cgrep(&cmd, cwd)
}

fn tool_callers(args: &Value) -> Result<String, String> {
    let function = required_str(args, "function")?;
    let cwd = opt_cwd(args);
    require_bounded_relative_scope("cgrep_callers", cwd, None, true)?;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootstrapOutcome {
    AlreadyIndexed,
    Bootstrapped,
    FellBackToScan,
}

fn ensure_index_for_search(
    cwd: Option<&str>,
    path: Option<&str>,
) -> Result<BootstrapOutcome, String> {
    let search_root = resolve_search_root(cwd, path)?;
    if cgrep::utils::find_index_root(&search_root).is_some() {
        clear_bootstrap_failure(&search_root);
        return Ok(BootstrapOutcome::AlreadyIndexed);
    }
    if recently_failed_bootstrap(&search_root) {
        return Ok(BootstrapOutcome::FellBackToScan);
    }

    let cmd = vec![
        "index".to_string(),
        "-p".to_string(),
        search_root.display().to_string(),
        "--embeddings".to_string(),
        "off".to_string(),
    ];
    match run_cgrep(&cmd, cwd) {
        Ok(_) => {
            clear_bootstrap_failure(&search_root);
            Ok(BootstrapOutcome::Bootstrapped)
        }
        Err(_) => {
            record_bootstrap_failure(&search_root);
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

fn recently_failed_bootstrap(search_root: &Path) -> bool {
    let key = search_root.display().to_string();
    let ttl = Duration::from_millis(AUTO_INDEX_FAILURE_TTL_MS);
    let now = Instant::now();
    let mut cache = failure_cache()
        .lock()
        .expect("bootstrap failure cache lock");
    cache.retain(|_, at| now.duration_since(*at) <= ttl);
    cache.contains_key(&key)
}

fn record_bootstrap_failure(search_root: &Path) {
    let key = search_root.display().to_string();
    let mut cache = failure_cache()
        .lock()
        .expect("bootstrap failure cache lock");
    cache.insert(key, Instant::now());
}

fn clear_bootstrap_failure(search_root: &Path) {
    let key = search_root.display().to_string();
    let mut cache = failure_cache()
        .lock()
        .expect("bootstrap failure cache lock");
    cache.remove(&key);
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
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to execute cgrep: {}", e))?;
    let timeout = mcp_tool_timeout();
    let started_at = Instant::now();

    loop {
        match child
            .try_wait()
            .map_err(|e| format!("failed to wait for cgrep: {}", e))?
        {
            Some(_) => break,
            None => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "cgrep MCP tool call timed out after {}ms. Retry with narrower scope (`path`, `glob`, or `changed`).",
                        timeout.as_millis()
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to read cgrep output: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
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
            msg = format!("cgrep exited with status {}", output.status);
        }
        Err(msg)
    }
}

fn mcp_tool_timeout() -> Duration {
    let timeout_ms = std::env::var("CGREP_MCP_TOOL_TIMEOUT_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_MCP_TOOL_TIMEOUT_MS);
    Duration::from_millis(timeout_ms)
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
                    "query": { "type": "string" },
                    "path": { "type": "string" },
                    "cwd": { "type": "string" },
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
                    "regex": { "type": "boolean" },
                    "case_sensitive": { "type": "boolean" },
                    "no_index": { "type": "boolean" },
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
                "required": ["path"],
                "properties": {
                    "path": { "type": "string" },
                    "cwd": { "type": "string" },
                    "section": { "type": "string" },
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
                    "symbol_type": { "type": "string" },
                    "lang": { "type": "string" },
                    "file_type": { "type": "string" },
                    "glob": { "type": "string" },
                    "exclude": { "type": "string" },
                    "changed": { "oneOf": [{ "type": "boolean" }, { "type": "string" }] }
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
                    "cwd": { "type": "string" }
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
                    "cwd": { "type": "string" }
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
