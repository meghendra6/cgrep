// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;
use tempfile::TempDir;

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

fn run_success(root: &Path, args: &[String]) -> String {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd.current_dir(root).args(args).assert().success();
    String::from_utf8(assert.get_output().stdout.clone()).expect("stdout utf8")
}

fn run_json2(root: &Path, args: &[&str]) -> Value {
    let mut all = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
    ];
    all.extend(args.iter().map(|v| v.to_string()));
    let stdout = run_success(root, &all);
    serde_json::from_str(&stdout).expect("json parse")
}

fn run_json2_raw(root: &Path, args: &[&str]) -> String {
    let mut all = vec![
        "--format".to_string(),
        "json2".to_string(),
        "--compact".to_string(),
    ];
    all.extend(args.iter().map(|v| v.to_string()));
    run_success(root, &all)
}

fn run_index(root: &Path) {
    let args = vec![
        "index".to_string(),
        "--embeddings".to_string(),
        "off".to_string(),
    ];
    let _ = run_success(root, &args);
}

fn run_git(root: &Path, args: &[&str]) {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_git_repo(root: &Path) {
    run_git(root, &["init", "-q"]);
    run_git(root, &["config", "user.email", "test@example.com"]);
    run_git(root, &["config", "user.name", "test"]);
}

fn write_fixture(root: &Path) {
    write_file(
        &root.join("src/lib.rs"),
        "pub fn validate_token(input: &str) -> bool {\n    input.starts_with(\"tok_\")\n}\n",
    );
    write_file(
        &root.join("src/service.rs"),
        "pub fn auth_flow() {\n    if validate_token(\"tok_seed\") {\n        println!(\"ok\");\n    }\n}\n",
    );
    write_file(
        &root.join("src/callers.rs"),
        "pub fn invoke_auth() {\n    let _ = validate_token(\"tok_x\");\n}\n",
    );
    write_file(
        &root.join("docs/guide.md"),
        "authentication middleware retry flow orchestration\n",
    );
}

#[test]
fn json2_compact_contract_is_stable_for_status_and_plan() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let status_1 = run_json2_raw(dir.path(), &["status"]);
    let status_2 = run_json2_raw(dir.path(), &["status"]);
    assert_eq!(status_1, status_2);

    let status: Value = serde_json::from_str(&status_1).expect("status json");
    assert_eq!(status["meta"]["schema_version"], "1");
    assert!(status["result"].get("phase").is_some());
    assert!(status["result"].get("progress").is_some());

    let plan_1 = run_json2_raw(
        dir.path(),
        &[
            "agent",
            "plan",
            "validate_token",
            "--max-steps",
            "6",
            "--max-candidates",
            "4",
        ],
    );
    let plan_2 = run_json2_raw(
        dir.path(),
        &[
            "agent",
            "plan",
            "validate_token",
            "--max-steps",
            "6",
            "--max-candidates",
            "4",
        ],
    );
    assert_eq!(plan_1, plan_2);
    assert!(plan_1.starts_with("{\"meta\":"));

    let plan: Value = serde_json::from_str(&plan_1).expect("plan json");
    assert_eq!(plan["meta"]["stage"], "plan");
    assert!(plan["steps"].is_array());
    assert!(plan["candidates"].is_array());
    assert!(plan.get("error").is_none());
}

#[test]
fn cross_feature_matrix_smoke_for_major_option_combinations() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    init_git_repo(dir.path());
    run_git(dir.path(), &["add", "."]);
    run_git(dir.path(), &["commit", "--quiet", "-m", "seed"]);

    write_file(
        &dir.path().join("src/service.rs"),
        "pub fn auth_flow() {\n    if validate_token(\"tok_changed\") {\n        println!(\"changed\");\n    }\n}\n",
    );

    run_index(dir.path());

    let scoped = run_json2(
        dir.path(),
        &[
            "search",
            "validate_token",
            "-p",
            "src",
            "--glob",
            "*.rs",
            "--changed",
            "HEAD",
            "--budget",
            "balanced",
            "--profile",
            "agent",
            "--mode",
            "keyword",
            "--limit",
            "10",
        ],
    );
    let scoped_results = scoped["results"].as_array().expect("scoped results");
    assert!(!scoped_results.is_empty());
    let alias_map = scoped["meta"]["path_aliases"].as_object();
    assert!(scoped_results.iter().all(|row| {
        row["path"]
            .as_str()
            .map(|raw| {
                let resolved = alias_map
                    .and_then(|aliases| aliases.get(raw))
                    .and_then(Value::as_str)
                    .unwrap_or(raw);
                resolved.ends_with(".rs")
            })
            .unwrap_or(false)
    }));

    let regex_scan = run_json2(
        dir.path(),
        &[
            "search",
            "validate_token\\(\"tok_",
            "--regex",
            "--no-index",
            "-p",
            "src",
            "--limit",
            "10",
        ],
    );
    assert!(regex_scan["results"].as_array().is_some());

    let explained = run_json2(
        dir.path(),
        &[
            "search",
            "validate_token",
            "--mode",
            "keyword",
            "--explain",
            "--limit",
            "5",
        ],
    );
    assert!(explained["results"][0].get("explain").is_some());

    let references = run_json2(
        dir.path(),
        &[
            "references",
            "validate_token",
            "--changed",
            "HEAD",
            "--mode",
            "auto",
            "--limit",
            "20",
        ],
    );
    assert!(references.is_array());

    let plan = run_json2(
        dir.path(),
        &[
            "agent",
            "plan",
            "validate_token",
            "--path",
            "src",
            "--changed",
            "HEAD",
            "--budget",
            "balanced",
            "--max-steps",
            "6",
            "--max-candidates",
            "4",
        ],
    );
    assert!(plan["steps"].as_array().is_some());

    let status = run_json2(dir.path(), &["status"]);
    assert!(status["result"]["basic_ready"].is_boolean());
    assert!(status["result"]["full_ready"].is_boolean());
}

#[test]
fn legacy_mode_aliases_remain_compatible_with_keyword_default() {
    let dir = TempDir::new().expect("tempdir");
    write_fixture(dir.path());
    run_index(dir.path());

    let default_payload = run_json2(
        dir.path(),
        &[
            "search",
            "validate_token",
            "--mode",
            "keyword",
            "--limit",
            "10",
        ],
    );
    let legacy_payload = run_json2(
        dir.path(),
        &["search", "validate_token", "--keyword", "--limit", "10"],
    );

    assert_eq!(default_payload["results"], legacy_payload["results"]);
}
