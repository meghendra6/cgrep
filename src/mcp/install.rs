// SPDX-License-Identifier: MIT OR Apache-2.0

//! MCP host configuration helpers.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use crate::cli::McpHost;

struct HostInfo {
    path: PathBuf,
    servers_key: &'static str,
    note: Option<&'static str>,
}

fn server_entry(command: &str) -> Value {
    json!({
        "command": command,
        "args": ["mcp", "serve"]
    })
}

fn required_home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))
}

fn resolve_cgrep_command() -> String {
    if let Ok(value) = std::env::var("CGREP_MCP_COMMAND") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    "cgrep".to_string()
}

fn host_info(host: McpHost) -> Result<HostInfo> {
    let info = match host {
        McpHost::Vscode => HostInfo {
            path: PathBuf::from(".vscode").join("mcp.json"),
            servers_key: "servers",
            note: Some("Project scope: run from the project root"),
        },
        McpHost::ClaudeCode => HostInfo {
            path: required_home_dir()?.join(".claude.json"),
            servers_key: "mcpServers",
            note: Some("User scope: available in all projects"),
        },
        McpHost::Cursor => HostInfo {
            path: required_home_dir()?.join(".cursor").join("mcp.json"),
            servers_key: "mcpServers",
            note: None,
        },
        McpHost::Windsurf => HostInfo {
            path: required_home_dir()?
                .join(".codeium")
                .join("windsurf")
                .join("mcp_config.json"),
            servers_key: "mcpServers",
            note: None,
        },
        McpHost::ClaudeDesktop => HostInfo {
            path: claude_desktop_path(&required_home_dir()?)?,
            servers_key: "mcpServers",
            note: None,
        },
    };
    Ok(info)
}

#[cfg(target_os = "macos")]
fn claude_desktop_path(home: &std::path::Path) -> Result<PathBuf> {
    Ok(home.join("Library/Application Support/Claude/claude_desktop_config.json"))
}

#[cfg(target_os = "windows")]
fn claude_desktop_path(_home: &std::path::Path) -> Result<PathBuf> {
    let appdata = std::env::var("APPDATA").context("APPDATA not set")?;
    Ok(PathBuf::from(appdata)
        .join("Claude")
        .join("claude_desktop_config.json"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn claude_desktop_path(_home: &std::path::Path) -> Result<PathBuf> {
    anyhow::bail!("claude-desktop path is not supported on this OS");
}

pub fn install(host: McpHost) -> Result<()> {
    let info = host_info(host)?;
    let command = resolve_cgrep_command();
    let mut config = if info.path.exists() {
        let raw = fs::read_to_string(&info.path)
            .with_context(|| format!("failed to read {}", info.path.display()))?;
        serde_json::from_str::<Value>(&raw)
            .with_context(|| format!("invalid JSON in {}", info.path.display()))?
    } else {
        json!({})
    };

    config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("root config is not a JSON object"))?
        .entry(info.servers_key)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("{} is not a JSON object", info.servers_key))?
        .insert("cgrep".to_string(), server_entry(&command));

    if let Some(parent) = info.path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(
        &info.path,
        serde_json::to_string_pretty(&config).expect("serializable"),
    )
    .with_context(|| format!("failed to write {}", info.path.display()))?;

    println!("✓ MCP config installed at {}", info.path.display());
    println!("  command: {}", command);
    if let Some(note) = info.note {
        println!("  {}", note);
    }
    Ok(())
}

pub fn uninstall(host: McpHost) -> Result<()> {
    let info = host_info(host)?;
    if !info.path.exists() {
        println!("MCP config file not found: {}", info.path.display());
        return Ok(());
    }

    let raw = fs::read_to_string(&info.path)
        .with_context(|| format!("failed to read {}", info.path.display()))?;
    let mut config: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid JSON in {}", info.path.display()))?;

    let removed = config
        .as_object_mut()
        .and_then(|obj| obj.get_mut(info.servers_key))
        .and_then(Value::as_object_mut)
        .and_then(|servers| servers.remove("cgrep"))
        .is_some();

    if !removed {
        println!("cgrep MCP entry not found in {}", info.path.display());
        return Ok(());
    }

    fs::write(
        &info.path,
        serde_json::to_string_pretty(&config).expect("serializable"),
    )
    .with_context(|| format!("failed to write {}", info.path.display()))?;

    println!("✓ MCP config removed from {}", info.path.display());
    Ok(())
}
