// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

#[test]
fn agent_install_and_uninstall_cursor_rule() {
    let dir = TempDir::new().expect("tempdir");
    let rule_path = dir.path().join(".cursor").join("rules").join("cgrep.mdc");

    let mut install_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    install_cmd
        .current_dir(dir.path())
        .args(["agent", "install", "cursor"])
        .assert()
        .success();

    assert!(rule_path.exists());
    let content = fs::read_to_string(&rule_path).expect("read rule");
    assert!(content.contains("cgrep Local Code Search"));
    assert!(content.contains("cgrep mcp install cursor"));

    let mut uninstall_cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    uninstall_cmd
        .current_dir(dir.path())
        .args(["agent", "uninstall", "cursor"])
        .assert()
        .success();

    assert!(!rule_path.exists());
}
