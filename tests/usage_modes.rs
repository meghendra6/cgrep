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
fn references_ast_avoids_string_literal_false_positive() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/app.ts"),
        r#"
function target() {
  return "target";
}
const msg = "target";
target();
"#,
    );

    let mut regex_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let regex_assert = regex_cmd
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "--compact",
            "references",
            "target",
            "--mode",
            "regex",
        ])
        .assert()
        .success();
    let regex_stdout =
        String::from_utf8(regex_assert.get_output().stdout.clone()).expect("regex utf8");
    let regex_json: Value = serde_json::from_str(&regex_stdout).expect("regex json");
    let regex_results = regex_json.as_array().expect("regex array");
    assert!(regex_results.iter().any(|v| {
        v.get("code")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("\"target\"")
    }));

    let mut ast_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let ast_assert = ast_cmd
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "--compact",
            "references",
            "target",
            "--mode",
            "ast",
        ])
        .assert()
        .success();
    let ast_stdout = String::from_utf8(ast_assert.get_output().stdout.clone()).expect("ast utf8");
    let ast_json: Value = serde_json::from_str(&ast_stdout).expect("ast json");
    let ast_results = ast_json.as_array().expect("ast array");
    assert!(!ast_results.is_empty());
    assert!(!ast_results.iter().any(|v| {
        v.get("code")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("\"target\"")
    }));
}

#[test]
fn callers_ast_skips_definition_lines() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/app.py"),
        r#"
def target():
    pass

def run():
    target()
"#,
    );

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "--compact",
            "callers",
            "target",
            "--mode",
            "ast",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    let results = json.as_array().expect("array");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["line"], 6);
    assert_eq!(results[0]["code"], "target()");
}
