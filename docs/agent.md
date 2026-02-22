# Agent Workflow

## Core policy

- Use cgrep first for local repository navigation.
- Prefer this flow: `map -> search -> read -> definition/references/callers`.
- In MCP/Codex loops, prefer `cgrep_agent_locate -> cgrep_agent_expand` before direct `cgrep_search`.
- Scope early with `-p`, `--glob`, `--changed`.
- Keep payload deterministic for agents: `--format json2 --compact`.
- In MCP usage, retrieval tools default to `auto_index=true`: first call bootstraps index if needed, and later refresh is call-driven + file-change-aware (no periodic reindex loop required).

## Two-stage retrieval (`agent`)

`cgrep agent` is optimized for low-token loops.

```bash
# Stage 1: locate compact candidates
cgrep agent locate "where token validation happens" --changed --budget balanced --compact
ID=$(cgrep agent locate "token validation" --compact | jq -r '.results[0].id')

# Stage 2: expand selected IDs
cgrep agent expand --id "$ID" -C 8 --compact
```

## Deterministic planning (`agent plan`)

`agent plan` orchestrates bounded `map -> agent locate -> agent expand` and emits deterministic `json2`.

```bash
cgrep --format json2 --compact agent plan "trace authentication middleware flow"
cgrep --format json2 --compact agent plan "validate_token" --max-steps 6 --max-candidates 5
```

Planner options:
- `--max-steps <n>`: cap emitted steps (default `6`)
- `--max-candidates <n>`: cap final candidates (default `5`)
- `--budget <tight|balanced|full|off>`: reused for locate stage
- `--profile <agent|ai|...>`: planner metadata profile label (aliases normalize to built-ins)
- `--path`, `--changed`, `--mode`: forwarded to locate strategy
- map execution policy:
  - with `--path`: planner executes `map`
  - without `--path`: planner keeps `map` as `planned` to bound latency on large repos
  - after `locate/expand`, planner adds bounded `read` follow-up steps for top candidates to speed verification loops

`json2` payload fields:
- `meta`: query/profile/budget/strategy and repository fingerprint/version info
- `steps[]`: stable step IDs, command, args, reason, expected output type, status
- `candidates[]`: stable IDs + short follow-up summaries
- `error` (optional): deterministic machine-parseable option validation failures

Deterministic ordering and tie-break rules:
- step order is emitted by strategy stage sequence (`map -> locate -> expand -> navigation -> read-verification`).
- step IDs are stable (`sNN_<slug>`).
- candidate order follows locate ranking with deterministic ties:
  1. score (desc)
  2. path (asc)
  3. line (asc)
  4. id (asc)
- optional fields (`diagnostics`, `error`) are omitted when empty.

Short alias form:

```bash
cgrep a l "where token validation happens" -u -B balanced --compact
```

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
- `mcp install` host values are `claude-code`, `cursor`, `windsurf`, `vscode`, `claude-desktop` (Codex uses `agent install codex`).
- `agent install claude-code` also runs MCP install for `claude-code` host.
- `agent install codex` also ensures `~/.codex/config.toml` has `[mcp_servers.cgrep]` with `command = "cgrep"`, `args = ["mcp", "serve"]`, and an explicit startup timeout.
- `agent install copilot` also runs MCP install for `vscode` host (`.vscode/mcp.json`).
- `agent install cursor` also writes `.cursor/rules/cgrep.mdc` and runs MCP install for `cursor` host.
- `agent install opencode` writes the OpenCode tool file only.

Codex runtime note:
- After `agent install codex`, restart the current Codex session so updated MCP config is reloaded.
- `agent uninstall codex` removes both `~/.codex/AGENTS.md` skill content and the `[mcp_servers.cgrep]` config block when present.

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
# Confirm MCP registration (Codex host)
codex mcp list

# Confirm MCP server responds
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
| cgrep mcp serve
```
