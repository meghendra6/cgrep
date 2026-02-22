# Agent Workflow

Use cgrep to keep AI coding-agent retrieval loops short and deterministic.

## 1) Install for Your Host

```bash
# Codex (recommended for this repo)
cgrep agent install codex

# Other hosts
cgrep agent install claude-code
cgrep agent install cursor
cgrep agent install copilot
```

Codex note: after install, restart the current Codex session to reload MCP config.

## 2) Use Low-Token Two-Stage Retrieval

```bash
# Stage 1: locate candidates
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')

# Stage 2: expand only selected candidate
cgrep agent expand --id "$ID" -C 8 --compact
```

## 3) Generate Deterministic Retrieval Plans

```bash
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
```

Useful options:
- `--max-steps <n>`
- `--max-candidates <n>`
- `--budget tight|balanced|full|off`
- `--path`, `--changed`

## Recommended Policy

- Prefer cgrep-first flow: `map -> search -> read -> definition/references/callers`
- Scope early with `-p`, `--glob`, `--changed`
- Use `--format json2 --compact` for deterministic parsing

## Uninstall

```bash
cgrep agent uninstall codex
cgrep agent uninstall claude-code
cgrep agent uninstall cursor
cgrep agent uninstall copilot
```

## Validate Quickly

```bash
codex mcp list
cgrep --format json2 --compact s "DispatchKeySet" -p c10/core
```
