// SPDX-License-Identifier: MIT OR Apache-2.0

//! Claude Code installation for cgrep
//!
//! Installs cgrep as a preferred search tool in Claude Code's CLAUDE.md file.

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::cli::McpHost;

use super::{
    append_if_not_present, content, home_dir, print_install_success, print_uninstall_success,
};

fn get_claude_md_path() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".claude").join("CLAUDE.md"))
}

pub fn install() -> Result<()> {
    let path = get_claude_md_path()?;
    let skill_content = content::claude_skill();

    let added =
        append_if_not_present(&path, &skill_content).context("Failed to update CLAUDE.md")?;

    if added {
        print_install_success("Claude Code");
    } else {
        println!("cgrep is already installed in Claude Code");
    }

    crate::mcp::install::install(McpHost::ClaudeCode)
        .context("Failed to install Claude Code MCP config")?;

    Ok(())
}

pub fn uninstall() -> Result<()> {
    let path = get_claude_md_path()?;

    if !path.exists() {
        println!("Claude Code CLAUDE.md not found");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let skill_content = content::claude_skill();
    let skill_trimmed = skill_content.trim();

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
        println!("cgrep is not installed in Claude Code");
    }

    crate::mcp::install::uninstall(McpHost::ClaudeCode)
        .context("Failed to remove Claude Code MCP config")?;

    Ok(())
}
