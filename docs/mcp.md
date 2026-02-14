# MCP

## Run MCP server

```bash
cgrep mcp serve
```

## Install host config

```bash
cgrep mcp install claude-code
cgrep mcp install cursor
cgrep mcp install windsurf
cgrep mcp install vscode
cgrep mcp install claude-desktop
```

## Remove host config

```bash
cgrep mcp uninstall claude-code
```

## Harness guidance

MCP mode follows harness-style principles for reliable tool-calling:
- Use structured tool chains (`search -> read -> symbol navigation`) instead of ad-hoc grep loops
- Keep outputs deterministic (`json/json2` + `--compact`) to reduce retry churn
- Narrow path/scope early for stable, low-token retrieval
- Expose read/search primitives only (no mutation tools)

Reference: <https://blog.can.ac/2026/02/12/the-harness-problem/>

## Exposed MCP tools

- `cgrep_search`
- `cgrep_read`
- `cgrep_map`
- `cgrep_symbols`
- `cgrep_definition`
- `cgrep_references`
- `cgrep_callers`
- `cgrep_dependents`
- `cgrep_index`

## Config file targets

| Host | Path | Key |
|---|---|---|
| `claude-code` | `~/.claude.json` | `mcpServers` |
| `cursor` | `~/.cursor/mcp.json` | `mcpServers` |
| `windsurf` | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` |
| `vscode` | `.vscode/mcp.json` | `servers` |
| `claude-desktop` | OS-specific desktop config path | `mcpServers` |

Note:
- `claude-desktop` auto-path is currently implemented for macOS/Windows.
