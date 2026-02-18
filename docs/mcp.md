# MCP

## 1-Minute Setup

```bash
cgrep mcp install codex
cgrep mcp install claude-code
cgrep mcp install cursor
```

Run server manually (for diagnostics):

```bash
cgrep mcp serve
```

Alias form:

```bash
cgrep mcp run
```

## Supported Hosts

```bash
cgrep mcp install claude-code
cgrep mcp install cursor
cgrep mcp install windsurf
cgrep mcp install vscode
cgrep mcp install claude-desktop
```

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

- `cgrep mcp install <host>` writes `command` as resolved cgrep executable path
  (absolute when available) to reduce GUI/PATH mismatch failures.
- `claude-desktop` auto-path is currently implemented for macOS/Windows.

## Exposed MCP Tools

- `cgrep_search`
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
- `cgrep_search` treats dash-prefixed queries (e.g. `-n`, `--help`) as literal search text.
- `cgrep_search` result `path` values are workspace-relative (relative to `cwd` when provided), so they can be passed directly to `cgrep_read`.

## Config File Targets

| Host | Path | Key |
|---|---|---|
| `claude-code` | `~/.claude.json` | `mcpServers` |
| `cursor` | `~/.cursor/mcp.json` | `mcpServers` |
| `windsurf` | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` |
| `vscode` | `.vscode/mcp.json` | `servers` |
| `claude-desktop` | OS-specific desktop config path | `mcpServers` |
