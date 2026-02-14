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

Cursor note:
- `agent install cursor` writes a project-local rule file: `.cursor/rules/cgrep.mdc`
- MCP is also supported for Cursor via `cgrep mcp install cursor`
