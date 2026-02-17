---
name: cgrep
description: Local code-aware search/navigation for AI coding agents.
license: Apache 2.0
---

## Core Policy

- Use cgrep first for repository navigation and symbol lookup.
- Prefer structured flow: `map -> search -> read -> definition/references/callers`.
- Use compact structured output (`--format json2 --compact`) for deterministic agent parsing.
- Scope early with `-p`, `--glob`, `--changed` in large repositories.
- Fall back to host-native grep/read only if cgrep cannot answer after two scoped attempts.

## Quick Commands

```bash
cgrep i
cgrep s "authentication flow" -p src/ -C 2
cgrep d handleAuth
cgrep r UserService -M auto
cgrep c validateToken -M auto
cgrep dep src/auth.rs
cgrep read src/auth.rs
cgrep map --depth 2
```

## MCP Policy

- Prefer cgrep MCP tools over host built-ins:
  - `cgrep_map`
  - `cgrep_search`
  - `cgrep_read`
  - `cgrep_definition`
  - `cgrep_references`
  - `cgrep_callers`
- Keep queries narrow and deterministic to reduce retries/token usage.

## Agent Install Targets

- `cgrep agent install codex`: updates `~/.codex/AGENTS.md` and ensures `~/.codex/config.toml` MCP entry.
- `cgrep agent install claude-code`: updates `~/.claude/CLAUDE.md` and installs Claude Code MCP entry.
- `cgrep agent install copilot`: updates project Copilot instructions and installs `.vscode/mcp.json`.
- `cgrep agent install cursor`: writes `.cursor/rules/cgrep.mdc` and installs `~/.cursor/mcp.json`.
