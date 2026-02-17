// SPDX-License-Identifier: MIT OR Apache-2.0

//! Codex installation for cgrep
//!
//! Installs cgrep as a preferred search tool in Codex's AGENTS.md file.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use super::{home_dir, print_install_success, print_uninstall_success};

const SKILL_CONTENT: &str = r#"
---
name: cgrep
description: A local code search tool using tantivy + tree-sitter. Fast, offline code search.
license: Apache 2.0
---

## When to use this skill

Use cgrep for any local code search or symbol lookup. Prefer it over grep.

## How to use this skill

Default is keyword search (BM25). If an index exists it is used; otherwise it
falls back to scan mode. Use `cgrep index` for repeated searches.

### Usage Examples

```bash
cgrep index
cgrep search "authentication flow"
cgrep search "auth middleware" -C 2 -p src/
cgrep search "validate_token" --regex --no-index
cgrep read src/auth.rs
cgrep map --depth 2
cgrep symbols UserService -T class
cgrep definition handleAuth
cgrep callers validateToken
cgrep references MyClass
cgrep dependents src/auth.rs
cgrep agent locate "token validation" --compact
ID=$(cgrep agent locate "token validation" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
```

### Options

- `-p, --path <path>` - Search in specific directory
- `-C, --context <n>` - Context lines around matches (default: 0)
- `-m, --limit <n>` - Limit number of results (default: 20)
- `--no-index` / `--regex` - Force scan mode or regex search
- `--format json|json2` - Structured output
- `--compact` - Compact JSON output (no pretty formatting)
- `--mode semantic|hybrid` - Optional; requires embeddings + index
- `--agent-cache` / `--cache-ttl` - Cache hybrid/semantic sessions
- `cgrep read` / `cgrep map` - Read focused regions and generate structure maps
- `cgrep agent locate/expand` - Two-stage low-token agent retrieval flow

### Harness Rules

- Prefer structured calls: `map -> search -> read -> symbols/definition/references`.
- Use `--format json --compact` or `--format json2 --compact` for deterministic output.
- Narrow scope early (`-p`, `--glob`, `--changed`) to avoid redundant retries.
- For MCP workflows, use `cgrep mcp serve` and cgrep tools instead of host built-ins.

## MCP Priority Policy

- For repository navigation, use cgrep MCP tools first: `cgrep_map -> cgrep_search -> cgrep_read -> cgrep_definition/references/callers`.
- Use host-native grep/read only if cgrep tools fail or return empty twice.
- Always narrow scope with `path`, `glob`, or `changed` when possible.
- Keep tool outputs deterministic: prefer compact JSON payloads.
"#;

fn get_agents_md_path() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".codex").join("AGENTS.md"))
}

fn has_cgrep_skill(content: &str) -> bool {
    content.contains("---\nname: cgrep\n")
}

fn get_codex_config_path() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".codex").join("config.toml"))
}

fn is_cgrep_binary(path: &std::path::Path) -> bool {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.eq_ignore_ascii_case("cgrep"))
        .unwrap_or(false)
}

fn resolve_cgrep_command() -> String {
    if let Ok(found) = which::which("cgrep") {
        if is_cgrep_binary(&found) {
            return found.to_string_lossy().to_string();
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if is_cgrep_binary(&exe) {
            return exe.to_string_lossy().to_string();
        }
    }

    "cgrep".to_string()
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn mcp_section(command: &str) -> String {
    format!(
        "[mcp_servers.cgrep]\ncommand = \"{}\"\nargs = [\"mcp\", \"serve\"]\n",
        toml_escape(command)
    )
}

fn upsert_mcp_section(content: &str, section: &str) -> Result<String> {
    let header = "[mcp_servers.cgrep]";
    let section_lines: Vec<String> = section.trim_end().lines().map(str::to_string).collect();
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.iter().position(|line| line.trim() == header);

    if let Some(start_idx) = start {
        let mut end_idx = lines.len();
        for (idx, line) in lines.iter().enumerate().skip(start_idx + 1) {
            if line.trim_start().starts_with('[') {
                end_idx = idx;
                break;
            }
        }

        let mut out: Vec<String> = Vec::new();
        out.extend(lines[..start_idx].iter().map(|line| (*line).to_string()));
        if !out.is_empty() && !out.last().is_some_and(|line| line.is_empty()) {
            out.push(String::new());
        }
        out.extend(section_lines.clone());
        if end_idx < lines.len() {
            if !out.last().is_some_and(|line| line.is_empty()) {
                out.push(String::new());
            }
            out.extend(lines[end_idx..].iter().map(|line| (*line).to_string()));
        }
        Ok(format!("{}\n", out.join("\n").trim_end()))
    } else if content.trim().is_empty() {
        Ok(section.to_string())
    } else {
        Ok(format!("{}\n\n{}", content.trim_end(), section.trim_end()))
    }
}

fn normalize_invalid_reasoning_effort(content: &str) -> String {
    content.replace(
        "model_reasoning_effort = \"xhigh\"",
        "model_reasoning_effort = \"high\"",
    )
}

fn ensure_codex_mcp_config() -> Result<bool> {
    let config_path = get_codex_config_path()?;
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let existing = if config_path.exists() {
        fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?
    } else {
        String::new()
    };

    let normalized = normalize_invalid_reasoning_effort(&existing);
    let section = mcp_section(&resolve_cgrep_command());
    let updated = upsert_mcp_section(&normalized, &section)?;

    if updated == existing {
        return Ok(false);
    }

    fs::write(&config_path, updated)
        .with_context(|| format!("Failed to write {}", config_path.display()))?;
    Ok(true)
}

pub fn install() -> Result<()> {
    let path = get_agents_md_path()?;
    let existing = if path.exists() {
        fs::read_to_string(&path).context("Failed to read existing AGENTS.md")?
    } else {
        String::new()
    };
    let added = if has_cgrep_skill(&existing) {
        false
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let mut merged = existing;
        if !merged.trim().is_empty() {
            merged.push('\n');
        }
        merged.push_str(SKILL_CONTENT.trim());
        merged.push('\n');
        fs::write(&path, merged).context("Failed to update AGENTS.md")?;
        true
    };

    let mcp_changed = ensure_codex_mcp_config().context("Failed to update Codex MCP config")?;

    if added {
        print_install_success("Codex");
    } else {
        println!("cgrep is already installed in Codex");
    }
    if mcp_changed {
        println!("Configured Codex MCP server entry in ~/.codex/config.toml");
    } else {
        println!("Codex MCP server entry already up to date");
    }

    Ok(())
}

pub fn uninstall() -> Result<()> {
    let path = get_agents_md_path()?;

    if !path.exists() {
        println!("Codex AGENTS.md not found");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let skill_trimmed = SKILL_CONTENT.trim();

    if content.contains(skill_trimmed) {
        let updated = content.replace(skill_trimmed, "");
        let cleaned: String = updated
            .lines()
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();

        if cleaned.is_empty() {
            std::fs::remove_file(&path)?;
        } else {
            std::fs::write(&path, cleaned)?;
        }
        print_uninstall_success("Codex");
    } else {
        println!("cgrep is not installed in Codex");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{mcp_section, normalize_invalid_reasoning_effort, upsert_mcp_section};

    #[test]
    fn upsert_mcp_section_appends_when_missing() {
        let input = "model = \"gpt-5-codex\"\n";
        let output = upsert_mcp_section(input, &mcp_section("/tmp/cgrep")).unwrap();
        assert!(output.contains("[mcp_servers.cgrep]"));
        assert!(output.contains("command = \"/tmp/cgrep\""));
    }

    #[test]
    fn upsert_mcp_section_replaces_existing_block() {
        let input = r#"
[mcp_servers.cgrep]
command = "cgrep"
args = ["mcp", "serve"]

[projects."/tmp/demo"]
trust_level = "trusted"
"#;
        let output = upsert_mcp_section(input, &mcp_section("/opt/cgrep")).unwrap();
        assert!(output.contains("command = \"/opt/cgrep\""));
        assert!(!output.contains("command = \"cgrep\""));
        assert!(output.contains("[projects.\"/tmp/demo\"]"));
    }

    #[test]
    fn normalize_invalid_reasoning_effort_rewrites_xhigh() {
        let input = "model_reasoning_effort = \"xhigh\"\n";
        let output = normalize_invalid_reasoning_effort(input);
        assert_eq!(output, "model_reasoning_effort = \"high\"\n");
    }
}
