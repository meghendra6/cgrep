// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

#[test]
fn mcp_install_and_uninstall_claude_code_updates_config() {
    let dir = TempDir::new().expect("tempdir");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).expect("home");
    let config_path = home.join(".claude.json");

    let mut install_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    install_cmd
        .current_dir(dir.path())
        .env("HOME", &home)
        .args(["mcp", "install", "claude-code"])
        .assert()
        .success();

    let raw = fs::read_to_string(&config_path).expect("read config");
    let json: Value = serde_json::from_str(&raw).expect("parse config");
    assert!(json["mcpServers"]["cgrep"].is_object());
    assert_eq!(json["mcpServers"]["cgrep"]["command"], "cgrep");

    let mut uninstall_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    uninstall_cmd
        .current_dir(dir.path())
        .env("HOME", &home)
        .args(["mcp", "uninstall", "claude-code"])
        .assert()
        .success();

    let raw = fs::read_to_string(&config_path).expect("read config");
    let json: Value = serde_json::from_str(&raw).expect("parse config");
    assert!(json["mcpServers"]["cgrep"].is_null());
}

#[test]
fn mcp_install_vscode_uses_servers_key() {
    let dir = TempDir::new().expect("tempdir");
    let vscode_dir = dir.path().join(".vscode");
    fs::create_dir_all(&vscode_dir).expect("mkdir");

    let mut install_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    install_cmd
        .current_dir(dir.path())
        .args(["mcp", "install", "vscode"])
        .assert()
        .success();

    let config_path = vscode_dir.join("mcp.json");
    let raw = fs::read_to_string(&config_path).expect("read config");
    let json: Value = serde_json::from_str(&raw).expect("parse config");
    assert!(json["servers"]["cgrep"].is_object());
    assert!(json["mcpServers"]["cgrep"].is_null());

    let mut uninstall_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    uninstall_cmd
        .current_dir(dir.path())
        .args(["mcp", "uninstall", "vscode"])
        .assert()
        .success();

    let raw = fs::read_to_string(&config_path).expect("read config");
    let json: Value = serde_json::from_str(&raw).expect("parse config");
    assert!(json["servers"]["cgrep"].is_null());
}
