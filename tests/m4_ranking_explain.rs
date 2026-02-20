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

fn write_config(root: &Path, content: &str) {
    fs::write(root.join(".cgreprc.toml"), content).expect("write config");
}

fn run_index(root: &Path) {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    cmd.current_dir(root)
        .args(["index", "--embeddings", "off"])
        .assert()
        .success();
}

fn run_json2(root: &Path, args: &[&str]) -> Value {
    let mut full_args = vec!["--format", "json2", "--compact"];
    full_args.extend_from_slice(args);

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd.current_dir(root).args(&full_args).assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    serde_json::from_str(&stdout).expect("json2 output")
}

fn init_git_repo(root: &Path) {
    run_git(root, &["init", "-q"]);
    run_git(root, &["config", "user.email", "test@example.com"]);
    run_git(root, &["config", "user.name", "test"]);
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

fn write_ranking_enabled_config(root: &Path) {
    write_config(
        root,
        r#"
[ranking]
enabled = true
path_weight = 1.2
symbol_weight = 1.8
language_weight = 1.0
changed_weight = 1.2
kind_weight = 2.0
weak_signal_penalty = 1.4
explain_top_k = 5
"#,
    );
}

#[test]
fn default_mode_preserves_legacy_behavior_without_explain() {
    let dir = TempDir::new().expect("tempdir");
    write_file(
        &dir.path().join("src/lib.rs"),
        "pub fn target_fn() {}\npub fn caller() { target_fn(); }\n",
    );
    run_index(dir.path());

    let baseline = run_json2(dir.path(), &["search", "target_fn", "--limit", "10"]);

    write_config(
        dir.path(),
        r#"
[ranking]
enabled = false
"#,
    );
    let explicit_disabled = run_json2(dir.path(), &["search", "target_fn", "--limit", "10"]);

    assert_eq!(baseline["results"], explicit_disabled["results"]);
    let results = baseline["results"].as_array().expect("results array");
    assert!(!results.is_empty());
    assert!(results.iter().all(|r| r.get("explain").is_none()));
}

#[test]
fn explain_json2_compact_is_deterministic_and_parseable() {
    let dir = TempDir::new().expect("tempdir");
    write_ranking_enabled_config(dir.path());
    write_file(
        &dir.path().join("src/primary.rs"),
        "pub fn target_fn() {}\npub fn run() { target_fn(); }\n",
    );
    write_file(
        &dir.path().join("docs/noise.txt"),
        "target_fn target_fn target_fn\n",
    );
    run_index(dir.path());

    let first = run_json2(
        dir.path(),
        &["search", "target_fn", "--limit", "10", "--explain"],
    );
    let second = run_json2(
        dir.path(),
        &["search", "target_fn", "--limit", "10", "--explain"],
    );

    assert_eq!(first["results"], second["results"]);
    let results = first["results"].as_array().expect("results array");
    assert!(!results.is_empty());
    let explain = results[0]["explain"].as_object().expect("explain object");
    for key in [
        "bm25",
        "path_boost",
        "symbol_boost",
        "changed_boost",
        "kind_boost",
        "penalties",
        "final_score",
    ] {
        assert!(
            explain.get(key).and_then(Value::as_f64).is_some(),
            "missing numeric explain field: {key}"
        );
    }
}

#[test]
fn identifier_like_queries_prefer_symbol_definitions_when_enabled() {
    let dir = TempDir::new().expect("tempdir");
    write_ranking_enabled_config(dir.path());
    write_file(
        &dir.path().join("src/primary.rs"),
        "pub fn target_fn() {}\npub fn run() { target_fn(); }\n",
    );
    write_file(
        &dir.path().join("docs/noisy.txt"),
        &"target_fn ".repeat(200),
    );
    run_index(dir.path());

    let payload = run_json2(
        dir.path(),
        &["search", "target_fn", "--limit", "10", "--explain"],
    );
    let first = payload["results"][0]["path"].as_str().expect("first path");
    assert_eq!(first, "src/primary.rs");
}

#[test]
fn phrase_like_queries_keep_full_text_relevance_priority() {
    let dir = TempDir::new().expect("tempdir");
    write_ranking_enabled_config(dir.path());
    write_file(
        &dir.path().join("src/impl.rs"),
        "pub fn retry_plan() -> &'static str { \"backoff\" }\n",
    );
    write_file(
        &dir.path().join("docs/guide.md"),
        "retry backoff strategy\nretry backoff strategy\nretry backoff strategy\n",
    );
    run_index(dir.path());

    let payload = run_json2(
        dir.path(),
        &[
            "search",
            "retry backoff strategy",
            "--limit",
            "10",
            "--explain",
        ],
    );
    let first = payload["results"][0]["path"].as_str().expect("first path");
    assert_eq!(first, "docs/guide.md");
}

#[test]
fn explain_components_match_final_score_and_tiebreak_is_stable() {
    let dir = TempDir::new().expect("tempdir");
    write_ranking_enabled_config(dir.path());
    write_file(&dir.path().join("a.txt"), "shared_token\n");
    write_file(&dir.path().join("b.txt"), "shared_token\n");

    let payload = run_json2(
        dir.path(),
        &[
            "search",
            "shared_token",
            "--no-index",
            "--limit",
            "10",
            "--explain",
        ],
    );
    let results = payload["results"].as_array().expect("results");
    assert!(results.len() >= 2);
    assert_eq!(results[0]["path"], "a.txt");
    assert_eq!(results[1]["path"], "b.txt");

    for result in results.iter().take(2) {
        let explain = result["explain"].as_object().expect("explain");
        let bm25 = explain["bm25"].as_f64().expect("bm25");
        let path_boost = explain["path_boost"].as_f64().expect("path");
        let symbol_boost = explain["symbol_boost"].as_f64().expect("symbol");
        let changed_boost = explain["changed_boost"].as_f64().expect("changed");
        let kind_boost = explain["kind_boost"].as_f64().expect("kind");
        let penalties = explain["penalties"].as_f64().expect("penalties");
        let final_score = explain["final_score"].as_f64().expect("final");
        let recomposed =
            bm25 * (1.0 + path_boost + symbol_boost + changed_boost + kind_boost + penalties);
        assert!((recomposed - final_score).abs() < 0.0001);

        let result_score = result["score"].as_f64().expect("result score");
        assert!((result_score - final_score).abs() < 0.0001);
    }
}

#[test]
fn changed_language_and_scope_filters_remain_correct_with_ranking() {
    let dir = TempDir::new().expect("tempdir");
    write_ranking_enabled_config(dir.path());
    init_git_repo(dir.path());

    write_file(&dir.path().join("src/a.rs"), "pub fn target_fn() {}\n");
    write_file(&dir.path().join("src/b.rs"), "pub fn target_fn() {}\n");
    write_file(
        &dir.path().join("src/nested/mod.rs"),
        "pub fn scope_marker() {}\n",
    );
    write_file(
        &dir.path().join("src/nested/helper.py"),
        "def scope_marker():\n    return 1\n",
    );
    run_git(dir.path(), &["add", "."]);
    run_git(dir.path(), &["commit", "--quiet", "-m", "initial"]);

    write_file(
        &dir.path().join("src/a.rs"),
        "pub fn target_fn() { let _v = 1; }\n",
    );

    let changed_payload = run_json2(
        dir.path(),
        &[
            "search",
            "target_fn",
            "--no-index",
            "--changed",
            "--explain",
            "--limit",
            "20",
        ],
    );
    let changed_results = changed_payload["results"]
        .as_array()
        .expect("changed results");
    assert!(!changed_results.is_empty());
    assert!(changed_results.iter().all(|r| r["path"] == "src/a.rs"));

    let scoped_payload = run_json2(
        dir.path(),
        &[
            "search",
            "scope_marker",
            "-p",
            "src/nested",
            "--no-index",
            "-t",
            "rust",
            "--explain",
            "--limit",
            "20",
        ],
    );
    let scoped_results = scoped_payload["results"]
        .as_array()
        .expect("scoped results");
    assert!(!scoped_results.is_empty());
    for result in scoped_results {
        let path = result["path"].as_str().expect("path");
        assert!(path.starts_with("src/nested/"));
        assert!(path.ends_with(".rs"));
    }
}
