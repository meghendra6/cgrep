// SPDX-License-Identifier: MIT OR Apache-2.0

use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

const FLOW_SENTENCE: &str = "map -> search -> read -> definition/references/callers";

fn install_provider(cwd: &Path, home: Option<&Path>, provider: &str) {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("cgrep"));
    cmd.current_dir(cwd).args(["agent", "install", provider]);
    if let Some(home_dir) = home {
        cmd.env("HOME", home_dir);
    }
    cmd.assert().success();
}

fn assert_no_deprecated_mode_aliases(content: &str) {
    assert!(!content.contains("--keyword"));
    assert!(!content.contains("--semantic"));
    assert!(!content.contains("--hybrid"));
}

fn assert_compacted(content: &str, baseline_chars: usize) {
    let max_chars = baseline_chars * 70 / 100;
    assert!(
        content.len() <= max_chars,
        "expected content length <= {} (70% of baseline {}), got {}",
        max_chars,
        baseline_chars,
        content.len()
    );
}

fn assert_core_phrases(content: &str) {
    assert!(content.contains(FLOW_SENTENCE));
    assert!(content.contains("cgrep agent locate"));
    assert!(content.contains("cgrep agent expand"));
}

#[test]
fn installed_agent_instructions_are_compact_and_consistent() {
    let dir = TempDir::new().expect("tempdir");
    let home = dir.path().join("home");
    fs::create_dir_all(&home).expect("home");

    install_provider(dir.path(), Some(&home), "codex");
    install_provider(dir.path(), Some(&home), "claude-code");
    install_provider(dir.path(), None, "copilot");
    install_provider(dir.path(), Some(&home), "cursor");
    install_provider(dir.path(), Some(&home), "opencode");

    let codex_path = home.join(".codex").join("AGENTS.md");
    let codex = fs::read_to_string(&codex_path).expect("read codex skill");
    assert_core_phrases(&codex);
    assert_no_deprecated_mode_aliases(&codex);
    assert_compacted(&codex, 2354);

    let claude_path = home.join(".claude").join("CLAUDE.md");
    let claude = fs::read_to_string(&claude_path).expect("read claude skill");
    assert!(claude.contains("## cgrep Local Code Search"));
    assert_core_phrases(&claude);
    assert_no_deprecated_mode_aliases(&claude);
    assert_compacted(&claude, 1601);

    let copilot_path = dir
        .path()
        .join(".github")
        .join("instructions")
        .join("cgrep.instructions.md");
    let copilot = fs::read_to_string(&copilot_path).expect("read copilot instructions");
    assert!(copilot.contains("# cgrep Local Code Search Instructions"));
    assert_core_phrases(&copilot);
    assert_no_deprecated_mode_aliases(&copilot);
    assert_compacted(&copilot, 3044);

    let cursor_path = dir.path().join(".cursor").join("rules").join("cgrep.mdc");
    let cursor = fs::read_to_string(&cursor_path).expect("read cursor rule");
    assert!(cursor.contains("cgrep mcp install cursor"));
    assert_core_phrases(&cursor);
    assert_no_deprecated_mode_aliases(&cursor);
    assert_compacted(&cursor, 1113);

    let opencode_path = home
        .join(".config")
        .join("opencode")
        .join("tool")
        .join("cgrep.ts");
    let opencode = fs::read_to_string(&opencode_path).expect("read opencode tool");
    let start = "const SKILL = `";
    let end = "`\n\nexport default tool(\"cgrep\"";
    let start_idx = opencode.find(start).expect("skill start") + start.len();
    let end_idx = opencode[start_idx..].find(end).expect("skill end") + start_idx;
    let skill = opencode[start_idx..end_idx].trim();
    assert_core_phrases(skill);
    assert_no_deprecated_mode_aliases(skill);
    assert_compacted(skill, 1741);
}
