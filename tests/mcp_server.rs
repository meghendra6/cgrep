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
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("cgrep"))
            .current_dir(cwd)
            .args(["mcp", "serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn mcp");
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
