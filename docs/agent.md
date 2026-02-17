# Agent Workflow

## Two-stage retrieval

`cgrep agent` is optimized for low-token loops with deterministic payloads.

1. `locate`: return compact candidate set
2. `expand`: fetch richer context only for selected IDs

```bash
# Stage 1: locate (json2-oriented output)
cgrep agent locate "where token validation happens" --changed --budget balanced --compact

# Short alias form:
cgrep a l "where token validation happens" -u -B balanced --compact

# Select first result ID (example)
ID=$(cgrep agent locate "token validation" --compact | jq -r '.results[0].id')

# Stage 2: expand chosen result(s)
cgrep agent expand --id "$ID" -C 8 --compact
```

Notes:
- `agent locate/expand` use payload minimization defaults
- `agent locate` supports caching for repeated prompts

## Integration install

Install instructions into supported agents:

```bash
cgrep agent install claude-code
cgrep agent install codex
cgrep agent install copilot
cgrep agent install cursor
cgrep agent install opencode
```

Uninstall:

```bash
cgrep agent uninstall claude-code
cgrep agent uninstall codex
cgrep agent uninstall copilot
cgrep agent uninstall cursor
cgrep agent uninstall opencode
```

Legacy `install-*` and `uninstall-*` commands remain for compatibility.

Auto MCP setup during install:
- `agent install claude-code` also runs MCP install for `claude-code` host.
- `agent install codex` also ensures `~/.codex/config.toml` has `[mcp_servers.cgrep]` with `cgrep mcp serve`.
- `agent install copilot` also runs MCP install for `vscode` host (`.vscode/mcp.json`).
- `agent install cursor` also writes `.cursor/rules/cgrep.mdc` and runs MCP install for `cursor` host.
- `agent install opencode` writes the OpenCode tool file only.

## Instruction/skill file outputs

`cgrep agent install <provider>` writes provider-native instruction/skill files:

| Provider | File(s) created/updated |
|---|---|
| `claude-code` | `~/.claude/CLAUDE.md` |
| `codex` | `~/.codex/AGENTS.md` |
| `copilot` | `.github/instructions/cgrep.instructions.md` (and optional append to `.github/copilot-instructions.md`) |
| `cursor` | `.cursor/rules/cgrep.mdc` |
| `opencode` | `~/.config/opencode/tool/cgrep.ts` |

## Verify in one minute

```bash
# Confirm MCP registration (Codex)
codex mcp list

# Confirm MCP server responds
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
| cgrep mcp serve
```
