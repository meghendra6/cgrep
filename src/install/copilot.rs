// SPDX-License-Identifier: MIT OR Apache-2.0

//! GitHub Copilot installation for cgrep
//!
//! Installs cgrep instructions in the project's .github/instructions directory.

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

use crate::cli::McpHost;

use super::{content, print_install_success, print_uninstall_success, write_file_if_changed};

fn has_cgrep_section(existing: &str) -> bool {
    existing.contains("## cgrep Local Code Search")
        || existing.contains("## cgrep Local Semantic Search")
}

fn get_project_root() -> Result<PathBuf> {
    env::current_dir().context("Failed to get current directory")
}

pub fn install() -> Result<()> {
    let project_root = get_project_root()?;
    let github_dir = project_root.join(".github");
    let instructions_dir = github_dir.join("instructions");
    let cgrep_instructions_path = instructions_dir.join("cgrep.instructions.md");
    let copilot_instructions_path = github_dir.join("copilot-instructions.md");
    let instructions_content = content::copilot_instructions();
    let append_content = content::copilot_appendix();

    // Create cgrep.instructions.md
    let created =
        write_file_if_changed(&cgrep_instructions_path, instructions_content.trim_start())
            .context("Failed to write cgrep instructions")?;

    if created {
        println!(
            "Created cgrep instructions at {:?}",
            cgrep_instructions_path
        );
    } else {
        println!("cgrep instructions already up to date");
    }

    // Append to copilot-instructions.md if it exists
    if copilot_instructions_path.exists() {
        let existing = std::fs::read_to_string(&copilot_instructions_path)?;
        if !has_cgrep_section(&existing) {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&copilot_instructions_path)?;
            use std::io::Write;
            write!(file, "{}", append_content)?;
            println!("Added cgrep section to {:?}", copilot_instructions_path);
        } else {
            println!("copilot-instructions already contains a cgrep section");
        }
    }

    crate::mcp::install::install(McpHost::Vscode)
        .context("Failed to install VS Code MCP config for Copilot")?;
    print_install_success("GitHub Copilot");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let project_root = get_project_root()?;
    let instructions_path = project_root
        .join(".github")
        .join("instructions")
        .join("cgrep.instructions.md");
    let copilot_instructions_path = project_root.join(".github").join("copilot-instructions.md");

    if instructions_path.exists() {
        std::fs::remove_file(&instructions_path)?;
        println!("Removed {:?}", instructions_path);
    } else {
        println!("cgrep instructions file not found");
    }

    if copilot_instructions_path.exists() {
        let content = std::fs::read_to_string(&copilot_instructions_path)?;
        let append_content = content::copilot_appendix();
        if content.contains(append_content.trim()) {
            let updated = content.replace(&append_content, "");
            std::fs::write(&copilot_instructions_path, updated)?;
            println!("Removed cgrep section from {:?}", copilot_instructions_path);
        }
    }

    crate::mcp::install::uninstall(McpHost::Vscode)
        .context("Failed to remove VS Code MCP config for Copilot")?;
    print_uninstall_success("GitHub Copilot");
    Ok(())
}
