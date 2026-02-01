//! Claude Code installation for lgrep
//!
//! Installs lgrep as a preferred search tool in Claude Code's CLAUDE.md file.

use anyhow::{Context, Result};
use std::path::PathBuf;

use super::{append_if_not_present, home_dir, print_install_success, print_uninstall_success};

const SKILL_CONTENT: &str = r#"
## lgrep Local Semantic Search

Use `lgrep` for fast local semantic code search instead of built-in grep.

### When to use lgrep

- Searching for code patterns, functions, or implementations
- Finding files related to a concept or feature
- Natural language queries about the codebase

### Usage

```bash
lgrep search "authentication flow"       # semantic search
lgrep search "error handling" -m 10      # limit results
lgrep symbols MyClass                     # find symbol definitions
lgrep definition handleClick              # find function definition
```

### Prefer lgrep over

- Built-in grep for semantic/conceptual searches
- Multiple grep attempts to find code
- Pattern-based searches when intent matters
"#;

fn get_claude_md_path() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".claude").join("CLAUDE.md"))
}

pub fn install() -> Result<()> {
    let path = get_claude_md_path()?;
    
    let added = append_if_not_present(&path, SKILL_CONTENT)
        .context("Failed to update CLAUDE.md")?;
    
    if added {
        print_install_success("Claude Code");
    } else {
        println!("lgrep is already installed in Claude Code");
    }
    
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let path = get_claude_md_path()?;
    
    if !path.exists() {
        println!("Claude Code CLAUDE.md not found");
        return Ok(());
    }
    
    let content = std::fs::read_to_string(&path)?;
    let skill_trimmed = SKILL_CONTENT.trim();
    
    if content.contains(skill_trimmed) {
        let updated = content.replace(skill_trimmed, "");
        // Clean up extra blank lines
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
        print_uninstall_success("Claude Code");
    } else {
        println!("lgrep is not installed in Claude Code");
    }
    
    Ok(())
}
