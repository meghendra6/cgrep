// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use tempfile::TempDir;

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

struct McpProc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl McpProc {
    fn spawn(cwd: &std::path::Path) -> Self {
        Self::spawn_with_env(cwd, &[])
    }

    fn spawn_with_env(cwd: &std::path::Path, envs: &[(&str, &str)]) -> Self {
        let mut base = std::process::Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
        base.current_dir(cwd)
            .args(["mcp", "serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        for (key, value) in envs {
            base.env(key, value);
        }
        let mut child = base.spawn().expect("spawn mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn call(&mut self, req: Value) -> Value {
        let line = serde_json::to_string(&req).expect("encode");
        writeln!(self.stdin, "{}", line).expect("write req");
        self.stdin.flush().expect("flush");

        let mut resp_line = String::new();
        self.stdout.read_line(&mut resp_line).expect("read resp");
        serde_json::from_str(&resp_line).expect("parse resp")
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_initialize_and_list_tools() {
    let dir = TempDir::new().expect("tempdir");
    let mut mcp = McpProc::spawn(dir.path());

    let init = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert!(init["result"]["instructions"]
        .as_str()
        .unwrap_or_default()
        .contains("harness"));

    let tools = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }));
    let names: Vec<String> = tools["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t.get("name").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect();
    assert!(names.contains(&"cgrep_search".to_string()));
    assert!(names.contains(&"cgrep_read".to_string()));
    assert!(names.contains(&"cgrep_index".to_string()));

    let tools_array = tools["result"]["tools"].as_array().expect("tools array");
    for tool_name in [
        "cgrep_search",
        "cgrep_read",
        "cgrep_map",
        "cgrep_symbols",
        "cgrep_definition",
        "cgrep_references",
        "cgrep_callers",
        "cgrep_dependents",
        "cgrep_index",
    ] {
        let tool = tools_array
            .iter()
            .find(|t| t["name"].as_str() == Some(tool_name))
            .unwrap_or_else(|| panic!("missing tool schema for {tool_name}"));
        assert!(
            tool["inputSchema"]["properties"]["cwd"].is_object(),
            "{tool_name} should expose optional cwd in MCP schema"
        );
    }

    mcp.stop();
}

#[test]
fn mcp_tool_call_executes_search_and_read() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn needle_token() {}\npub fn run() { needle_token(); }\n",
    );

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .current_dir(dir.path())
        .args(["index", "--embeddings", "off"])
        .assert()
        .success();

    let mut mcp = McpProc::spawn(dir.path());
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let search = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_search",
            "arguments": {
                "query": "needle_token",
                "path": ".",
                "limit": 5
            }
        }
    }));
    let search_text = search["result"]["content"][0]["text"]
        .as_str()
        .expect("search text");
    let search_json: Value = serde_json::from_str(search_text).expect("search json2");
    assert!(search_json["results"]
        .as_array()
        .map(|arr| !arr.is_empty())
        .unwrap_or(false));

    let read = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "cgrep_read",
            "arguments": {
                "path": "src/lib.rs"
            }
        }
    }));
    let read_text = read["result"]["content"][0]["text"]
        .as_str()
        .expect("read text");
    let read_json: Value = serde_json::from_str(read_text).expect("read json");
    assert!(read_json.is_object() || read_json.is_array());

    mcp.stop();
}

#[test]
fn mcp_unknown_tool_returns_is_error() {
    let dir = TempDir::new().expect("tempdir");
    let mut mcp = McpProc::spawn(dir.path());

    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let resp = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "unknown_tool",
            "arguments": {}
        }
    }));

    assert_eq!(resp["result"]["isError"], true);
    assert!(resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("unknown tool"));

    mcp.stop();
}

#[test]
fn mcp_search_accepts_literal_query_starting_with_dash() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/lib.rs"), "--needle marker\n");

    let mut mcp = McpProc::spawn(dir.path());
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let search = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_search",
            "arguments": {
                "query": "--needle",
                "path": "src",
                "no_index": true
            }
        }
    }));
    let search_text = search["result"]["content"][0]["text"]
        .as_str()
        .expect("search text");
    let search_json: Value = serde_json::from_str(search_text).expect("search json2");
    assert!(search_json["results"]
        .as_array()
        .map(|arr| !arr.is_empty())
        .unwrap_or(false));

    mcp.stop();
}

#[test]
fn mcp_tool_call_honors_cwd_for_relative_paths() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn needle_token() {}\npub fn run() { needle_token(); }\n",
    );
    let cwd = dir.path().to_string_lossy().to_string();

    let mut mcp = McpProc::spawn(std::path::Path::new("/"));
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let search = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_search",
            "arguments": {
                "query": "needle_token",
                "path": ".",
                "cwd": cwd,
                "no_index": true
            }
        }
    }));
    let search_text = search["result"]["content"][0]["text"]
        .as_str()
        .expect("search text");
    let search_json: Value = serde_json::from_str(search_text).expect("search json2");
    let first_path = search_json["results"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("path"))
        .and_then(Value::as_str)
        .expect("result path");
    assert_eq!(first_path, "src/lib.rs");

    let read = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "cgrep_read",
            "arguments": {
                "path": first_path,
                "cwd": dir.path().to_string_lossy().to_string()
            }
        }
    }));
    let read_text = read["result"]["content"][0]["text"]
        .as_str()
        .expect("read text");
    let read_json: Value = serde_json::from_str(read_text).expect("read json");
    assert!(read_json["content"]
        .as_str()
        .map(|content| content.contains("needle_token"))
        .unwrap_or(false));

    mcp.stop();
}

#[test]
fn mcp_rejects_relative_scope_without_cwd_when_server_runs_from_root() {
    let mut mcp = McpProc::spawn(std::path::Path::new("/"));
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let map = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_map",
            "arguments": {
                "path": ".",
                "depth": 1
            }
        }
    }));
    assert_eq!(map["result"]["isError"], true);
    assert!(map["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("requires `cwd`"));

    mcp.stop();
}

#[test]
fn mcp_tool_call_times_out_when_child_exceeds_timeout_budget() {
    let dir = TempDir::new().expect("tempdir");
    let mut mcp = McpProc::spawn_with_env(dir.path(), &[("CGREP_MCP_TOOL_TIMEOUT_MS", "1")]);
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let map = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_map",
            "arguments": {
                "path": "/",
                "depth": 1
            }
        }
    }));
    assert_eq!(map["result"]["isError"], true);
    assert!(map["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("timed out"));

    mcp.stop();
}
