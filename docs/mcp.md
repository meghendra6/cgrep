# MCP

## 1-Minute Setup

```bash
# Codex (agent instructions + Codex MCP wiring)
cgrep agent install codex

# MCP host install examples
cgrep mcp install claude-code
cgrep mcp install cursor
```

- `cgrep mcp install codex` is not a valid host command.
- For Codex, use `cgrep agent install codex`.
- For host list, run `cgrep mcp install --help`.

Run server manually (for diagnostics):

```bash
cgrep mcp serve
```

Alias form:

```bash
cgrep mcp run
```

## Supported Hosts

| Host | Install command |
|---|---|
| `claude-code` | `cgrep mcp install claude-code` |
| `cursor` | `cgrep mcp install cursor` |
| `windsurf` | `cgrep mcp install windsurf` |
| `vscode` | `cgrep mcp install vscode` |
| `claude-desktop` | `cgrep mcp install claude-desktop` |

Alias:

```bash
cgrep mcp add <host>
```

Remove config:

```bash
cgrep mcp uninstall <host>
cgrep mcp rm <host>
```

## Verification

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
| cgrep mcp serve
```

## Behavior Notes

- `cgrep mcp install <host>` writes `command = "cgrep"` by default so binary updates are picked up without reinstalling MCP config.
- If you need a fixed path, set `CGREP_MCP_COMMAND` before install.
- `claude-desktop` auto-path is currently implemented for macOS/Windows.
- MCP tool calls are internally bounded by a timeout; if exceeded, cgrep returns an explicit MCP error instead of hanging until host timeout.

## Exposed MCP Tools

- `cgrep_search`
- `cgrep_agent_locate`
- `cgrep_agent_expand`
- `cgrep_read`
- `cgrep_map`
- `cgrep_symbols`
- `cgrep_definition`
- `cgrep_references`
- `cgrep_callers`
- `cgrep_dependents`
- `cgrep_index`

## Tool Argument Notes

- MCP tools accept optional `cwd` to pin relative path resolution.
- `cgrep_search` defaults to balanced output budget and enables `path_alias`/`dedupe_context`/`suppress_boilerplate` unless explicitly disabled.
- `cgrep_search` defaults to `auto_index=true`; when no index exists it attempts one bootstrap index build, then falls back to scan on bootstrap failure.
- `cgrep_search` treats dash-prefixed queries (e.g. `-n`, `--help`) as literal search text.
- `cgrep_search` rejects empty/whitespace queries consistently (including `regex=true`).
- `cgrep_search` result `path` values stay reusable:
  workspace-internal scopes return workspace-relative paths, external scopes return absolute paths.
- `cgrep_read` rejects empty path arguments (`Error: Path cannot be empty`).
- If MCP server cwd is `/`, relative scopes require `cwd` (or an absolute `path`) to avoid scanning system root by mistake.

## Config File Targets

| Host | Path | Key |
|---|---|---|
| `claude-code` | `~/.claude.json` | `mcpServers` |
| `cursor` | `~/.cursor/mcp.json` | `mcpServers` |
| `windsurf` | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` |
| `vscode` | `.vscode/mcp.json` | `servers` |
| `claude-desktop` | OS-specific desktop config path | `mcpServers` |
