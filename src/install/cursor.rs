// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cursor installation for cgrep
//!
//! Installs a project-local Cursor rule file so Cursor agents prefer cgrep for
//! code search and navigation.

use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::cli::McpHost;

use super::{content, print_install_success, print_uninstall_success, write_file_if_changed};

fn get_cursor_rule_path() -> Result<PathBuf> {
    let root = env::current_dir().context("Failed to get current directory")?;
    Ok(root.join(".cursor").join("rules").join("cgrep.mdc"))
}

pub fn install() -> Result<()> {
    let rule_path = get_cursor_rule_path()?;
    let cursor_rule = content::cursor_rule();
    let created = write_file_if_changed(&rule_path, cursor_rule.trim_start())
        .context("Failed to write Cursor rule file")?;

    if created {
        println!("Created Cursor rule at {:?}", rule_path);
    } else {
        println!("Cursor rule already up to date");
    }

    print_install_success("Cursor");
    crate::mcp::install::install(McpHost::Cursor).context("Failed to install Cursor MCP config")?;
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

    crate::mcp::install::uninstall(McpHost::Cursor)
        .context("Failed to remove Cursor MCP config")?;

    Ok(())
}
