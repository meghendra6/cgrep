// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

fn write_file(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

#[test]
fn read_small_file_returns_full_mode() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn alpha() {}\npub fn beta() {}\n",
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json", "read", "src/lib.rs"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(json["mode"], "full");
    assert_eq!(json["path"], "src/lib.rs");
    assert!(json["content"]
        .as_str()
        .unwrap_or("")
        .contains("pub fn alpha"));
}

#[test]
fn read_large_file_returns_outline_mode() {
    let dir = TempDir::new().expect("tempdir");
    let mut content = String::new();
    for i in 0..450 {
        content.push_str(&format!("pub fn function_{i}() -> i32 {{ {i} }}\n"));
    }
    write_file(&dir.path().join("src/large.rs"), &content);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json", "read", "src/large.rs"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(json["mode"], "outline");
    let body = json["content"].as_str().unwrap_or("");
    assert!(body.contains("function function_0"));
}

#[test]
fn read_section_line_range_returns_subset() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("notes.md"), "alpha\nbeta\ngamma\ndelta\n");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json", "read", "notes.md", "--section", "2-3"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(json["mode"], "section");
    assert_eq!(json["content"], "beta\ngamma");
}

#[test]
fn map_json2_includes_symbols() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/a.rs"), "pub fn alpha() {}\n");
    write_file(
        &dir.path().join("src/b.rs"),
        "pub struct Beta { pub n: i32 }\n",
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json2", "map", "--depth", "3"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(json["meta"]["command"], "map");

    let entries = json["entries"].as_array().expect("entries");
    let a_rs = entries
        .iter()
        .find(|entry| entry["path"] == "src/a.rs")
        .expect("src/a.rs entry");
    let symbols = a_rs["symbols"].as_array().expect("symbols");
    assert!(symbols.iter().any(|name| name == "alpha"));
}
