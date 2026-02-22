# Agent Workflow

Use cgrep to keep AI coding-agent retrieval loops short and deterministic.

## 1) Install for Your Host

```bash
cgrep agent install codex
cgrep agent install claude-code
cgrep agent install cursor
cgrep agent install copilot
cgrep agent install opencode
```

## 2) What is Required vs Optional

- Required: restart the current agent session once after installation.
- Not required for normal use: manual `cgrep index` or always-on daemon.
- Optional: run `cgrep daemon start` during long, high-churn coding sessions.

## 3) Optional Low-Token Two-Stage Retrieval (CLI)

```bash
# Stage 1: locate candidates
ID=$(cgrep agent locate "where token validation happens" --compact | jq -r '.results[0].id')

# Stage 2: expand only selected candidate
cgrep agent expand --id "$ID" -C 8 --compact
```

## 4) Optional Deterministic Retrieval Plan (CLI)

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
cgrep agent uninstall opencode
```

## Validate Quickly

```bash
codex mcp list
cgrep --format json2 --compact s "DispatchKeySet" -p c10/core
```
