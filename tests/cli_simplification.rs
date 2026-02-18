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
fn direct_query_with_positional_path_filters_scope() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/hit.txt"), "needle\n");
    write_file(&dir.path().join("other.txt"), "needle\n");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json", "needle", "src", "--no-index"])
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
        "direct query + positional path should exclude out-of-scope files"
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
fn no_ignore_allows_ignored_files_in_direct_mode() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join(".ignore"), "ignored.txt\n");
    write_file(&dir.path().join("ignored.txt"), "needle\n");

    let mut default_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let default_assert = default_cmd
        .current_dir(dir.path())
        .args(["--format", "json", "needle", "--no-index"])
        .assert()
        .success();
    let default_stdout =
        String::from_utf8(default_assert.get_output().stdout.clone()).expect("utf8");
    let default_json: Value = serde_json::from_str(&default_stdout).expect("json");
    let default_results = default_json.as_array().expect("array");
    assert!(
        default_results.iter().all(|r| {
            r["path"]
                .as_str()
                .map(|p| !p.contains("ignored.txt"))
                .unwrap_or(true)
        }),
        "default scan should respect ignore files"
    );

    let mut no_ignore_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let no_ignore_assert = no_ignore_cmd
        .current_dir(dir.path())
        .args(["--format", "json", "--no-ignore", "needle", "--no-index"])
        .assert()
        .success();
    let no_ignore_stdout =
        String::from_utf8(no_ignore_assert.get_output().stdout.clone()).expect("utf8");
    let no_ignore_json: Value = serde_json::from_str(&no_ignore_stdout).expect("json");
    let no_ignore_results = no_ignore_json.as_array().expect("array");
    assert!(no_ignore_results.iter().any(|r| {
        r["path"]
            .as_str()
            .map(|p| p.contains("ignored.txt"))
            .unwrap_or(false)
    }));
}

#[test]
fn no_ignore_forces_scan_even_when_index_exists() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join(".ignore"), "ignored.txt\n");
    write_file(&dir.path().join("visible.txt"), "needle\n");
    write_file(&dir.path().join("ignored.txt"), "needle\n");

    let mut index_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    index_cmd
        .current_dir(dir.path())
        .args(["index"])
        .assert()
        .success();

    let mut indexed_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let indexed_assert = indexed_cmd
        .current_dir(dir.path())
        .args(["--format", "json2", "needle"])
        .assert()
        .success();
    let indexed_stdout =
        String::from_utf8(indexed_assert.get_output().stdout.clone()).expect("utf8");
    let indexed_json: Value = serde_json::from_str(&indexed_stdout).expect("json");
    assert_eq!(indexed_json["meta"]["index_mode"], "index");

    let mut no_ignore_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let no_ignore_assert = no_ignore_cmd
        .current_dir(dir.path())
        .args(["--format", "json2", "--no-ignore", "needle"])
        .assert()
        .success();
    let no_ignore_stdout =
        String::from_utf8(no_ignore_assert.get_output().stdout.clone()).expect("utf8");
    let no_ignore_json: Value = serde_json::from_str(&no_ignore_stdout).expect("json");
    assert_eq!(no_ignore_json["meta"]["index_mode"], "scan");
}

#[test]
fn no_recursive_limits_scope_and_recursive_short_flag_reenables_depth() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/top.txt"), "needle\n");
    write_file(&dir.path().join("src/nested/deep.txt"), "needle\n");

    let mut shallow_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let shallow_assert = shallow_cmd
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "needle",
            "src",
            "--no-index",
            "--no-recursive",
        ])
        .assert()
        .success();
    let shallow_stdout =
        String::from_utf8(shallow_assert.get_output().stdout.clone()).expect("utf8");
    let shallow_json: Value = serde_json::from_str(&shallow_stdout).expect("json");
    let shallow_results = shallow_json.as_array().expect("array");
    assert!(shallow_results.iter().any(|r| {
        r["path"]
            .as_str()
            .map(|p| p.contains("top.txt"))
            .unwrap_or(false)
    }));
    assert!(
        shallow_results.iter().all(|r| {
            r["path"]
                .as_str()
                .map(|p| !p.contains("deep.txt"))
                .unwrap_or(true)
        }),
        "--no-recursive should skip nested paths"
    );

    let mut recursive_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let recursive_assert = recursive_cmd
        .current_dir(dir.path())
        .args(["--format", "json", "-r", "needle", "src", "--no-index"])
        .assert()
        .success();
    let recursive_stdout =
        String::from_utf8(recursive_assert.get_output().stdout.clone()).expect("utf8");
    let recursive_json: Value = serde_json::from_str(&recursive_stdout).expect("json");
    let recursive_results = recursive_json.as_array().expect("array");
    assert!(recursive_results.iter().any(|r| {
        r["path"]
            .as_str()
            .map(|p| p.contains("deep.txt"))
            .unwrap_or(false)
    }));
}

#[test]
fn include_and_exclude_dir_aliases_work_in_direct_mode() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("src/keep/hit.rs"), "needle\n");
    write_file(&dir.path().join("src/skip/hit.rs"), "needle\n");
    write_file(&dir.path().join("src/keep/ignore.txt"), "needle\n");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "--include",
            "**/*.rs",
            "needle",
            "src",
            "--no-index",
            "--exclude-dir",
            "skip/**",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    let results = json.as_array().expect("array");
    assert!(results.iter().any(|r| {
        r["path"]
            .as_str()
            .map(|p| p.contains("keep/hit.rs"))
            .unwrap_or(false)
    }));
    assert!(
        results.iter().all(|r| {
            let path = r["path"].as_str().unwrap_or_default();
            !path.contains("skip/hit.rs") && !path.contains("ignore.txt")
        }),
        "--include/--exclude-dir aliases should filter paths like grep/rg users expect"
    );
}

#[test]
fn direct_mode_supports_literal_query_starting_with_dash() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("sample.txt"), "--needle marker\n");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json", "--no-index", "--", "--needle"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    let results = json.as_array().expect("array");
    assert!(results.iter().any(|r| {
        r["path"]
            .as_str()
            .map(|p| p.contains("sample.txt"))
            .unwrap_or(false)
    }));
}

#[test]
fn root_help_mentions_direct_mode_and_literal_escape() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    cmd.args(["--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "cgrep [OPTIONS] [SEARCH_OPTIONS] <QUERY> [PATH]",
        ))
        .stdout(predicate::str::contains("Direct search shorthand:"))
        .stdout(predicate::str::contains("cgrep -- --literal"));
}

#[test]
fn direct_mode_accepts_grep_ignore_case_flag() {
    let dir = TempDir::new().expect("tempdir");
    write_file(&dir.path().join("sample.txt"), "Needle marker\n");

    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    let assert = cmd
        .current_dir(dir.path())
        .args(["--format", "json", "-i", "needle", "--no-index"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    let results = json.as_array().expect("array");
    assert!(!results.is_empty());
}

#[test]
fn symbols_help_includes_include_and_exclude_dir_aliases() {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    cmd.args(["symbols", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--include"))
        .stdout(predicate::str::contains("--exclude-dir"));
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
        .stdout(predicate::str::contains("--include"))
        .stdout(predicate::str::contains("--exclude-dir"))
        .stdout(predicate::str::contains("--no-ignore"))
        .stdout(predicate::str::contains("cgrep \"token refresh\" src/"));
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
