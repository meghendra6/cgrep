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
fn profile_fast_applies_max_results_for_search() {
    let dir = TempDir::new().expect("tempdir");
    let file_path = dir.path().join("sample.txt");

    let content = (0..30)
        .map(|i| format!("needle line {}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    write_file(&file_path, &content);

    let mut with_profile = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = with_profile
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "search",
            "needle",
            "--no-index",
            "--profile",
            "fast",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let with_profile_json: Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(with_profile_json.as_array().expect("array").len(), 10);

    let mut without_profile = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = without_profile
        .current_dir(dir.path())
        .args(["--format", "json", "search", "needle", "--no-index"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let without_profile_json: Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(without_profile_json.as_array().expect("array").len(), 20);
}

#[test]
fn profile_agent_switches_default_output_to_json2() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn needle_fn() {}\npub fn caller() { needle_fn(); }\n",
    );

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .args([
            "index",
            "--path",
            dir.path().to_str().expect("path"),
            "--embeddings",
            "off",
        ])
        .assert()
        .success();

    let mut search_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = search_cmd
        .current_dir(dir.path())
        .args(["search", "needle", "--profile", "agent"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json2 object");

    assert!(json.get("meta").is_some());
    assert!(json.get("results").is_some());
    assert!(json
        .get("meta")
        .and_then(|m| m.get("schema_version"))
        .is_some());
}
