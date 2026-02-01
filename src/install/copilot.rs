//! GitHub Copilot installation for lgrep
//!
//! Installs lgrep instructions in the project's .github/instructions directory.

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

use super::{print_install_success, print_uninstall_success, write_file_if_changed};

const LGREP_INSTRUCTIONS: &str = r#"---
name: lgrep Semantic Search
description: Use lgrep for local semantic code search instead of built-in grep/search tools.
applyTo: "**/*"
---

# lgrep Local Semantic Search Instructions

## CRITICAL: Tool Override

When you need to search for code or files locally, **use lgrep instead of built-in search tools**.

- **AVOID**: Using grep_search for content lookup
- **AVOID**: Multiple grep attempts to find relevant code

- **USE**: `lgrep search "natural language query"` for semantic file search
- **USE**: `lgrep symbols <name>` for symbol search
- **USE**: `lgrep definition <name>` for finding definitions

## When to Use lgrep

Use lgrep **IMMEDIATELY** when:

- Searching for code patterns, functions, or implementations
- Looking up how something is done in the codebase
- Finding files related to a concept or feature
- User asks "where is...", "how does...", "find..."
- You need context about the codebase structure

## Usage Examples

```bash
# Semantic code search
lgrep search "authentication flow implementation"
lgrep search "error handling patterns" -p src/

# Symbol search  
lgrep symbols handleAuth -t function
lgrep definition UserService

# Dependency tracking
lgrep callers process_request
lgrep dependents src/auth.rs
```

## Options Reference

| Option | Description |
|--------|-------------|
| `-m, --max <n>` | Limit number of results (default: 20) |
| `-c, --context <n>` | Context lines (default: 3) |
| `-p, --path <path>` | Search in specific directory |

## Best Practices

### Do

- Use natural language queries: `lgrep search "How are database connections managed?"`
- Specify subdirectories for focused search: `lgrep search "validation logic" -p src/lib/`

### Don't

- Use single-word queries: `lgrep search "parser"` (too vague)
- Skip indexing: Run `lgrep index` first if the index doesn't exist

## Keywords

semantic search, grep, search, find code, code search, file search, codebase search
"#;

const COPILOT_INSTRUCTIONS_APPEND: &str = r#"

## lgrep Local Semantic Search

This project uses **lgrep** for local semantic code search. When searching for code or content:

1. Use `lgrep search "natural language query"` instead of grep-based searches
2. For symbol lookup: `lgrep symbols <name>`
3. For definitions: `lgrep definition <name>`

lgrep uses tantivy + tree-sitter for fast offline semantic search.
"#;

fn get_project_root() -> Result<PathBuf> {
    env::current_dir().context("Failed to get current directory")
}

pub fn install() -> Result<()> {
    let project_root = get_project_root()?;
    let github_dir = project_root.join(".github");
    let instructions_dir = github_dir.join("instructions");
    let lgrep_instructions_path = instructions_dir.join("lgrep.instructions.md");
    let copilot_instructions_path = github_dir.join("copilot-instructions.md");

    // Create lgrep.instructions.md
    let created = write_file_if_changed(&lgrep_instructions_path, LGREP_INSTRUCTIONS.trim_start())
        .context("Failed to write lgrep instructions")?;

    if created {
        println!("Created lgrep instructions at {:?}", lgrep_instructions_path);
    } else {
        println!("lgrep instructions already up to date");
    }

    // Append to copilot-instructions.md if it exists
    if copilot_instructions_path.exists() {
        let existing = std::fs::read_to_string(&copilot_instructions_path)?;
        if !existing.contains("## lgrep Local Semantic Search") && !existing.contains("lgrep") {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&copilot_instructions_path)?;
            use std::io::Write;
            write!(file, "{}", COPILOT_INSTRUCTIONS_APPEND)?;
            println!("Added lgrep section to {:?}", copilot_instructions_path);
        }
    }

    print_install_success("GitHub Copilot");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let project_root = get_project_root()?;
    let instructions_path = project_root
        .join(".github")
        .join("instructions")
        .join("lgrep.instructions.md");
    let copilot_instructions_path = project_root.join(".github").join("copilot-instructions.md");

    if instructions_path.exists() {
        std::fs::remove_file(&instructions_path)?;
        println!("Removed {:?}", instructions_path);
    } else {
        println!("lgrep instructions file not found");
    }

    if copilot_instructions_path.exists() {
        let content = std::fs::read_to_string(&copilot_instructions_path)?;
        if content.contains(COPILOT_INSTRUCTIONS_APPEND.trim()) {
            let updated = content.replace(COPILOT_INSTRUCTIONS_APPEND, "");
            std::fs::write(&copilot_instructions_path, updated)?;
            println!("Removed lgrep section from {:?}", copilot_instructions_path);
        }
    }

    print_uninstall_success("GitHub Copilot");
    Ok(())
}
