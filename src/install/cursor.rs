// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cursor installation for cgrep
//!
//! Installs a project-local Cursor rule file so Cursor agents prefer cgrep for
//! code search and navigation.

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

use super::{print_install_success, print_uninstall_success, write_file_if_changed};

const CURSOR_RULE: &str = r#"---
description: Use cgrep for local code search/navigation instead of ad-hoc grep loops.
globs:
  - "**/*"
alwaysApply: false
---

# cgrep Local Code Search

Use `cgrep` as the default local retrieval tool.

## Preferred flow

1. `cgrep map --depth 2`
2. `cgrep search "query" --format json2 --compact`
3. `cgrep read <path>` or `cgrep definition/references/callers`
4. For low-token loops: `cgrep agent locate` -> `cgrep agent expand`

## Examples

```bash
cgrep index
cgrep search "authentication flow" -p src/ -C 2
cgrep symbols UserService -T class
cgrep definition handleAuth
cgrep references UserService
cgrep callers validateToken
cgrep dependents src/auth.rs
cgrep agent locate "token validation" --compact
ID=$(cgrep agent locate "token validation" --compact | jq -r '.results[0].id')
cgrep agent expand --id "$ID" -C 8 --compact
```

## Notes

- Prefer `--format json2 --compact` for deterministic agent parsing.
- Use path/scope flags early (`-p`, `--glob`, `--changed`) to reduce retries.
- MCP mode is available via `cgrep mcp serve` and `cgrep mcp install cursor`.
"#;

fn get_cursor_rule_path() -> Result<PathBuf> {
    let root = env::current_dir().context("Failed to get current directory")?;
    Ok(root.join(".cursor").join("rules").join("cgrep.mdc"))
}

pub fn install() -> Result<()> {
    let rule_path = get_cursor_rule_path()?;
    let created = write_file_if_changed(&rule_path, CURSOR_RULE.trim_start())
        .context("Failed to write Cursor rule file")?;

    if created {
        println!("Created Cursor rule at {:?}", rule_path);
    } else {
        println!("Cursor rule already up to date");
    }

    print_install_success("Cursor");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let rule_path = get_cursor_rule_path()?;

    if rule_path.exists() {
        fs::remove_file(&rule_path)?;
        println!("Removed Cursor rule {:?}", rule_path);
        print_uninstall_success("Cursor");
    } else {
        println!("Cursor rule not found");
    }

    Ok(())
}
