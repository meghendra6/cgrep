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

## Manual Indexing And Daemon FAQ

- Do I need to run `cgrep index` manually before MCP?
  - Usually no. Default MCP tools run with `auto_index=true` and bootstrap/refresh as needed.
- Do I need to run `cgrep daemon start` for MCP?
  - Not required. MCP auto-index is call-driven and uses file-change signals while the MCP server is alive.
- When should I still run daemon manually?
  - During heavy active coding sessions when you want the index to stay warm continuously between tool calls.
- Semantic/hybrid via MCP:
  - still requires an embeddings-enabled index and remains experimental.

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
- `cgrep_search`, `cgrep_agent_locate`, `cgrep_symbols`, `cgrep_definition`, `cgrep_references`, `cgrep_callers`, and `cgrep_dependents` default to `auto_index=true`.
- With `auto_index=true`, MCP bootstraps index on first use when missing.
- For existing indexes, MCP uses file-change-aware refresh: while the MCP server process is alive it subscribes to filesystem change events, then refreshes on the next MCP tool call only when changes are detected (no periodic background reindex loop).
- If MCP/agent usage stops, auto-index activity also stops because refresh is call-driven.
- MCP auto-indexing uses embeddings-off indexing for predictable latency/cost; semantic/hybrid (experimental) still require an explicit embeddings-enabled index.
- Set `auto_index=false` per tool call if you want to skip this behavior.
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
