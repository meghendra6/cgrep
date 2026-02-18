// SPDX-License-Identifier: MIT OR Apache-2.0

//! MCP server support for cgrep (stdio JSON-RPC).

pub mod install;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};

const PROTOCOL_VERSION: &str = "2024-11-05";

// Keep harness guidance close to the server so every MCP host gets the same behavior.
const HARNESS_INSTRUCTIONS: &str = "\
cgrep MCP harness (search/navigation only).\n\
\n\
Use cgrep tools instead of host built-in search/read tools for repository navigation.\n\
\n\
Recommended workflow:\n\
1) cgrep_map for structure\n\
2) cgrep_search for candidate locations\n\
3) cgrep_read for exact context\n\
4) cgrep_definition/cgrep_references/cgrep_callers for symbol relationships\n\
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
    let mut cmd = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
        "search".to_string(),
    ];

    push_opt_flag_value(&mut cmd, "-p", opt_str(args, "path"));
    push_opt_flag_value_u64(&mut cmd, "-m", opt_u64(args, "limit"));
    push_opt_flag_value_u64(&mut cmd, "-C", opt_u64(args, "context"));
    push_opt_flag_value(&mut cmd, "-t", opt_str(args, "file_type"));
    push_opt_flag_value(&mut cmd, "--glob", opt_str(args, "glob"));
    push_opt_flag_value(&mut cmd, "--exclude", opt_str(args, "exclude"));
    push_opt_flag_value(&mut cmd, "--mode", opt_str(args, "mode"));
    push_changed(&mut cmd, args.get("changed"));
    push_bool_flag(&mut cmd, "--regex", opt_bool(args, "regex"));
    push_bool_flag(
        &mut cmd,
        "--case-sensitive",
        opt_bool(args, "case_sensitive"),
    );
    push_bool_flag(&mut cmd, "--no-index", opt_bool(args, "no_index"));
    push_bool_flag(&mut cmd, "--fuzzy", opt_bool(args, "fuzzy"));
    cmd.push("--".to_string());
    cmd.push(query.to_string());

    run_cgrep(&cmd, cwd)
}

fn tool_read(args: &Value) -> Result<String, String> {
    let path = required_str(args, "path")?;
    let cwd = opt_cwd(args);
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
    let mut cmd = vec!["index".to_string()];
    push_opt_flag_value(&mut cmd, "-p", opt_str(args, "path"));
    push_bool_flag(&mut cmd, "--force", opt_bool(args, "force"));
    push_bool_flag(&mut cmd, "--high-memory", opt_bool(args, "high_memory"));
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

fn opt_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn opt_u64(args: &Value, key: &str) -> Option<u64> {
    args.get(key).and_then(Value::as_u64)
}

fn opt_bool(args: &Value, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn opt_array_str<'a>(args: &'a Value, key: &str) -> Option<Vec<&'a str>> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|vals| vals.iter().filter_map(Value::as_str).collect::<Vec<_>>())
}

fn opt_cwd(args: &Value) -> Option<&str> {
    opt_str(args, "cwd").filter(|value| !value.trim().is_empty())
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
    command.args(args).stdin(Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|e| format!("failed to execute cgrep: {}", e))?;

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
