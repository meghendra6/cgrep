// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use predicates::prelude::*;
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
fn search_help_advanced_prints_hidden_options() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    cmd.args(["search", "--help-advanced"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Advanced search options:"))
        .stdout(predicate::str::contains("--max-total-chars"));
}

#[test]
fn deprecated_mode_alias_prints_warning() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("sample.txt"), "needle\n");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    cmd.current_dir(dir.path())
        .args([
            "--format",
            "json",
            "search",
            "needle",
            "--keyword",
            "--no-index",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("`--keyword` is deprecated"));
}

#[test]
fn grep_alias_with_positional_path_filters_scope() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/hit.txt"), "needle\n");
    write_file(&dir.path().join("other.txt"), "needle\n");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json", "grep", "needle", "src", "--no-index"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    let results = json.as_array().expect("array");
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| {
        r["path"]
            .as_str()
            .map(|p| p.contains("hit.txt"))
            .unwrap_or(false)
    }));
    assert!(
        results.iter().all(|r| {
            r["path"]
                .as_str()
                .map(|p| !p.contains("other.txt"))
                .unwrap_or(false)
        }),
        "grep alias + positional path should exclude out-of-scope files"
    );
}

#[test]
fn explicit_path_flag_takes_precedence_over_positional_path() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("a/a.txt"), "needle\n");
    write_file(&dir.path().join("b/b.txt"), "needle\n");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "search",
            "needle",
            "a",
            "--path",
            "b",
            "--no-index",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    let results = json.as_array().expect("array");
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| {
        r["path"]
            .as_str()
            .map(|p| p.contains("b.txt"))
            .unwrap_or(false)
    }));
    assert!(
        results.iter().all(|r| {
            r["path"]
                .as_str()
                .map(|p| !p.contains("a.txt"))
                .unwrap_or(false)
        }),
        "--path should win over positional path"
    );
}

#[test]
fn search_help_includes_grep_transition_examples() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    cmd.args(["search", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Optional path (grep-style positional form)",
        ))
        .stdout(predicate::str::contains("cgrep grep \"auth flow\" src/"));
}

#[test]
fn agent_locate_and_expand_roundtrip() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn auth_flow() {}\npub fn call() { auth_flow(); }\n",
    );

    let mut locate_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let locate_assert = locate_cmd
        .current_dir(dir.path())
        .args(["agent", "locate", "auth_flow"])
        .assert()
        .success();
    let locate_stdout = String::from_utf8(locate_assert.get_output().stdout.clone()).expect("utf8");
    let locate_json: Value = serde_json::from_str(&locate_stdout).expect("json2");
    let first_id = locate_json["results"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("id"))
        .and_then(Value::as_str)
        .expect("result id")
        .to_string();

    let mut expand_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let expand_assert = expand_cmd
        .current_dir(dir.path())
        .args(["agent", "expand", "--id", &first_id, "-C", "1"])
        .assert()
        .success();
    let expand_stdout = String::from_utf8(expand_assert.get_output().stdout.clone()).expect("utf8");
    let expand_json: Value = serde_json::from_str(&expand_stdout).expect("expand json");

    assert_eq!(expand_json["meta"]["stage"], "expand");
    assert!(expand_json["meta"]["resolved_ids"].as_u64().unwrap_or(0) >= 1);
    assert!(
        expand_json["meta"]["hint_resolved_ids"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert_eq!(expand_json["results"][0]["id"], first_id);
}

#[test]
fn agent_expand_falls_back_to_scan_when_hint_is_stale() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn auth_flow() {}\npub fn call() { auth_flow(); }\n",
    );

    let mut locate_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let locate_assert = locate_cmd
        .current_dir(dir.path())
        .args(["agent", "locate", "auth_flow"])
        .assert()
        .success();
    let locate_stdout = String::from_utf8(locate_assert.get_output().stdout.clone()).expect("utf8");
    let locate_json: Value = serde_json::from_str(&locate_stdout).expect("json2");
    let first_id = locate_json["results"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("id"))
        .and_then(Value::as_str)
        .expect("result id")
        .to_string();

    let hint_path = dir
        .path()
        .join(".cgrep")
        .join("cache")
        .join("agent_expand_hints.json");
    let mut hint_json: Value =
        serde_json::from_str(&fs::read_to_string(&hint_path).expect("hint cache")).expect("hint");
    if let Some(entries) = hint_json.get_mut("entries").and_then(Value::as_array_mut) {
        for entry in entries {
            if entry.get("id").and_then(Value::as_str) == Some(first_id.as_str()) {
                entry["path"] = Value::String("src/missing.rs".to_string());
            }
        }
    }
    fs::write(
        &hint_path,
        serde_json::to_string_pretty(&hint_json).expect("encode hint"),
    )
    .expect("write hint");

    let mut expand_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let expand_assert = expand_cmd
        .current_dir(dir.path())
        .args(["agent", "expand", "--id", &first_id, "-C", "1"])
        .assert()
        .success();
    let expand_stdout = String::from_utf8(expand_assert.get_output().stdout.clone()).expect("utf8");
    let expand_json: Value = serde_json::from_str(&expand_stdout).expect("expand json");

    assert_eq!(expand_json["meta"]["hint_resolved_ids"], 0);
    assert!(
        expand_json["meta"]["scan_resolved_ids"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
    assert_eq!(expand_json["results"][0]["id"], first_id);
}

#[test]
fn search_json2_does_not_persist_agent_hints() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("sample.txt"), "needle\n");

    let mut search_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    search_cmd
        .current_dir(dir.path())
        .args(["--format", "json2", "search", "needle", "--no-index"])
        .assert()
        .success();

    let hint_path = dir
        .path()
        .join(".cgrep")
        .join("cache")
        .join("agent_expand_hints.json");
    assert!(!hint_path.exists());
}
