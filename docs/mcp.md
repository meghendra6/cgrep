# MCP

## Quick Setup

```bash
# Codex: use agent installer (includes MCP wiring)
cgrep agent install codex

# Direct MCP host install examples
cgrep mcp install claude-code
cgrep mcp install cursor
cgrep mcp install vscode
```

Supported `mcp install` hosts:
- `claude-code`
- `cursor`
- `windsurf`
- `vscode`
- `claude-desktop`

## Run MCP Server Manually (Debug)

```bash
cgrep mcp serve
```

## Common MCP Tools

- `cgrep_search`
- `cgrep_read`
- `cgrep_map`
- `cgrep_definition`
- `cgrep_references`
- `cgrep_callers`
- `cgrep_symbols`
- `cgrep_dependents`
- `cgrep_agent_locate`
- `cgrep_agent_expand`

## Behavior You Should Know

- Most MCP tools default to `auto_index=true`.
- If index is missing, first call bootstraps it automatically.
- Refresh is call-driven + file-change-aware while MCP server is alive.
- No always-on periodic reindex loop is required for normal MCP usage.
- Semantic/hybrid mode is experimental and still needs embeddings index.

## Troubleshooting

```bash
# Verify MCP server handshake quickly
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
| cgrep mcp serve
```

If path resolution looks wrong, pass `cwd` in MCP tool arguments.
