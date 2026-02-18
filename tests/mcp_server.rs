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

fn resolved_paths(payload: &Value) -> Vec<String> {
    let aliases = payload["meta"]["path_aliases"].as_object();
    payload["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row.get("path").and_then(Value::as_str))
        .map(|raw| {
            aliases
                .and_then(|map| map.get(raw))
                .and_then(Value::as_str)
                .unwrap_or(raw)
                .to_string()
        })
        .collect()
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
    assert!(names.contains(&"cgrep_agent_locate".to_string()));
    assert!(names.contains(&"cgrep_agent_expand".to_string()));
    assert!(names.contains(&"cgrep_read".to_string()));
    assert!(names.contains(&"cgrep_index".to_string()));

    let tools_array = tools["result"]["tools"].as_array().expect("tools array");
    for tool_name in [
        "cgrep_search",
        "cgrep_agent_locate",
        "cgrep_agent_expand",
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
fn mcp_search_applies_default_budget_metadata() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn budget_meta_marker() {}\n",
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
                "query": "budget_meta_marker",
                "path": "src",
                "limit": 5
            }
        }
    }));
    let text = search["result"]["content"][0]["text"]
        .as_str()
        .expect("search text");
    let payload: Value = serde_json::from_str(text).expect("json");
    assert_eq!(payload["meta"]["max_total_chars"], 6000);
    assert_eq!(payload["meta"]["dedupe_context"], true);
    assert_eq!(payload["meta"]["path_alias"], true);
    assert_eq!(payload["meta"]["suppress_boilerplate"], true);
    assert!(payload["meta"]["fallback_chain"].as_array().is_some());
    assert!(payload["meta"]["payload_chars"].as_u64().is_some());
    assert!(payload["meta"]["payload_tokens_estimate"]
        .as_u64()
        .is_some());

    mcp.stop();
}

#[test]
fn mcp_agent_locate_and_expand_roundtrip() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn roundtrip_marker() {}\npub fn call_roundtrip() { roundtrip_marker(); }\n",
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

    let locate = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_agent_locate",
            "arguments": {
                "query": "roundtrip_marker"
            }
        }
    }));
    let locate_text = locate["result"]["content"][0]["text"]
        .as_str()
        .expect("locate text");
    let locate_json: Value = serde_json::from_str(locate_text).expect("locate json");
    let first_id = locate_json["results"][0]["id"]
        .as_str()
        .expect("first id")
        .to_string();

    let expand = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "cgrep_agent_expand",
            "arguments": {
                "ids": [first_id],
                "context": 2
            }
        }
    }));
    let expand_text = expand["result"]["content"][0]["text"]
        .as_str()
        .expect("expand text");
    let expand_json: Value = serde_json::from_str(expand_text).expect("expand json");
    assert!(expand_json["meta"]["resolved_ids"].as_u64().unwrap_or(0) >= 1);
    assert!(expand_json["results"]
        .as_array()
        .map(|arr| !arr.is_empty())
        .unwrap_or(false));

    mcp.stop();
}

#[test]
fn mcp_search_auto_indexes_once_when_missing() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn bootstrap_index_marker() {}\n",
    );

    let mut mcp = McpProc::spawn(dir.path());
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let first = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_search",
            "arguments": {
                "query": "bootstrap_index_marker",
                "path": "src"
            }
        }
    }));
    let first_text = first["result"]["content"][0]["text"]
        .as_str()
        .expect("first search text");
    let first_json: Value = serde_json::from_str(first_text).expect("first search json");
    assert_eq!(first_json["meta"]["index_mode"], "index");
    assert_eq!(first_json["meta"]["bootstrap_index"], true);

    let second = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "cgrep_search",
            "arguments": {
                "query": "bootstrap_index_marker",
                "path": "src"
            }
        }
    }));
    let second_text = second["result"]["content"][0]["text"]
        .as_str()
        .expect("second search text");
    let second_json: Value = serde_json::from_str(second_text).expect("second search json");
    assert_eq!(second_json["meta"]["index_mode"], "index");
    assert_eq!(second_json["meta"]["bootstrap_index"], false);

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
fn mcp_search_treats_colon_query_as_literal_in_index_mode() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/match.rs"),
        "const META: &str = \"schema_version:\";\n",
    );
    write_file(
        &dir.path().join("src/noise.rs"),
        "const META: &str = \"schema_version\";\n",
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
                "query": "schema_version:",
                "path": "src",
                "limit": 10
            }
        }
    }));
    let search_text = search["result"]["content"][0]["text"]
        .as_str()
        .expect("search text");
    let search_json: Value = serde_json::from_str(search_text).expect("search json2");
    assert_eq!(search_json["meta"]["index_mode"], "index");
    let paths = resolved_paths(&search_json);
    assert!(!paths.is_empty());
    assert!(paths.iter().any(|p| p.contains("src/match.rs")));
    assert!(paths.iter().all(|p| !p.contains("src/noise.rs")));

    mcp.stop();
}

#[test]
fn mcp_search_rejects_empty_or_whitespace_query() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/lib.rs"), "pub fn needle_token() {}\n");

    let mut mcp = McpProc::spawn(dir.path());
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    for (id, query, regex) in [
        (2, "", false),
        (3, " ", false),
        (4, "", true),
        (5, " ", true),
    ] {
        let search = mcp.call(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "cgrep_search",
                "arguments": {
                    "query": query,
                    "path": "src",
                    "regex": regex,
                    "limit": 5
                }
            }
        }));
        assert_eq!(search["result"]["isError"], true);
        assert!(search["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("Search query cannot be empty"));
    }

    mcp.stop();
}

#[test]
fn mcp_scan_search_truncates_utf8_snippets_without_panic() {
    let dir = TempDir::new().expect("tempdir");
    let long_emoji = "😀".repeat(1200);
    write_file(
        &dir.path().join("src/emoji.rs"),
        &format!("line0\n// {long_emoji}\nline2\n"),
    );

    let mut mcp = McpProc::spawn(dir.path());
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    for (id, context) in [(2, 0), (3, 2)] {
        let search = mcp.call(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "cgrep_search",
                "arguments": {
                    "query": "😀😀😀",
                    "path": "src",
                    "no_index": true,
                    "context": context,
                    "limit": 1
                }
            }
        }));
        assert_ne!(search["result"]["isError"], true);
        let search_text = search["result"]["content"][0]["text"]
            .as_str()
            .expect("search text");
        let search_json: Value = serde_json::from_str(search_text).expect("search json2");
        let results = search_json["results"].as_array().expect("results");
        assert!(!results.is_empty());
        let snippet = results[0]["snippet"].as_str().unwrap_or_default();
        assert!(snippet.contains("😀"));
        assert!(snippet.chars().count() <= 153);
    }

    mcp.stop();
}

#[test]
fn mcp_case_sensitive_search_matches_scan_behavior() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/lib.rs"), "Needle marker\n");

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

    let indexed = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_search",
            "arguments": {
                "query": "needle",
                "path": "src",
                "case_sensitive": true,
                "limit": 5
            }
        }
    }));
    let indexed_text = indexed["result"]["content"][0]["text"]
        .as_str()
        .expect("indexed text");
    let indexed_json: Value = serde_json::from_str(indexed_text).expect("indexed json");
    assert_eq!(indexed_json["meta"]["index_mode"], "index");
    assert_eq!(indexed_json["meta"]["total_matches"], 0);

    let scanned = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "cgrep_search",
            "arguments": {
                "query": "needle",
                "path": "src",
                "case_sensitive": true,
                "no_index": true,
                "limit": 5
            }
        }
    }));
    let scanned_text = scanned["result"]["content"][0]["text"]
        .as_str()
        .expect("scanned text");
    let scanned_json: Value = serde_json::from_str(scanned_text).expect("scanned json");
    assert_eq!(scanned_json["meta"]["index_mode"], "scan");
    assert_eq!(scanned_json["meta"]["total_matches"], 0);

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
    let first_raw_path = search_json["results"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("path"))
        .and_then(Value::as_str)
        .expect("result path");
    let first_path = search_json["meta"]["path_aliases"]
        .as_object()
        .and_then(|aliases| aliases.get(first_raw_path))
        .and_then(Value::as_str)
        .unwrap_or(first_raw_path);
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

#[test]
fn mcp_references_file_scope_paths_roundtrip_to_read() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/install/codex.rs"),
        r#"
fn resolve_cgrep_command() -> String {
    "cgrep".to_string()
}

fn mcp_section() {
    let _cmd = resolve_cgrep_command();
}
"#,
    );

    let mut mcp = McpProc::spawn(std::path::Path::new("/"));
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let refs = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_references",
            "arguments": {
                "name": "resolve_cgrep_command",
                "path": "src/install/codex.rs",
                "mode": "regex",
                "cwd": dir.path().to_string_lossy().to_string()
            }
        }
    }));
    let refs_text = refs["result"]["content"][0]["text"]
        .as_str()
        .expect("references text");
    let refs_json: Value = serde_json::from_str(refs_text).expect("references json");
    let refs_results = refs_json.as_array().expect("references array");
    assert!(!refs_results.is_empty());
    let first_path = refs_results[0]["path"].as_str().expect("path");
    assert_eq!(first_path, "src/install/codex.rs");

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
        .map(|content| content.contains("resolve_cgrep_command"))
        .unwrap_or(false));

    mcp.stop();
}

#[test]
fn mcp_read_rejects_empty_path() {
    let dir = TempDir::new().expect("tempdir");
    let mut mcp = McpProc::spawn(dir.path());
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let read = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_read",
            "arguments": {
                "path": ""
            }
        }
    }));
    assert_eq!(read["result"]["isError"], true);
    assert!(read["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("Path cannot be empty"));

    mcp.stop();
}

#[test]
fn mcp_map_root_contract_is_stable() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/lib.rs"), "pub fn alpha() {}\n");

    let mut mcp = McpProc::spawn(std::path::Path::new("/"));
    let _ = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));

    let dot_map = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cgrep_map",
            "arguments": {
                "path": ".",
                "depth": 1,
                "cwd": dir.path().to_string_lossy().to_string()
            }
        }
    }));
    let dot_map_text = dot_map["result"]["content"][0]["text"]
        .as_str()
        .expect("dot map text");
    let dot_map_json: Value = serde_json::from_str(dot_map_text).expect("dot map json");
    assert_eq!(dot_map_json["root"], ".");

    let abs_map = mcp.call(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "cgrep_map",
            "arguments": {
                "path": dir.path().to_string_lossy().to_string(),
                "depth": 1
            }
        }
    }));
    let abs_map_text = abs_map["result"]["content"][0]["text"]
        .as_str()
        .expect("abs map text");
    let abs_map_json: Value = serde_json::from_str(abs_map_text).expect("abs map json");
    assert_eq!(abs_map_json["root"], dir.path().to_string_lossy().as_ref());

    mcp.stop();
}
