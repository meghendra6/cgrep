# Usage

## Command overview

| Command | Description |
|---|---|
| `cgrep search <query> [path]` (`s`, `find`, `q`) | Full-text search |
| `cgrep read <path>` (`rd`, `cat`, `view`) | Smart file read (small file full, large file outline) |
| `cgrep map` (`mp`, `tree`) | Structural codebase map (file + symbol skeleton) |
| `cgrep symbols <name>` (`sym`, `sy`) | Symbol search |
| `cgrep definition <name>` (`def`, `d`) | Definition lookup |
| `cgrep callers <function>` (`calls`, `c`) | Caller lookup |
| `cgrep references <name>` (`refs`, `r`) | References lookup |
| `cgrep dependents <file>` (`deps`, `dep`) | Reverse dependency lookup |
| `cgrep index` (`ix`, `i`) | Build/rebuild index |
| `cgrep watch` (`wt`, `w`) | Reindex on file changes |
| `cgrep daemon <start|status|stop>` (`bg`) | Manage background watch daemon |
| `cgrep status` (`st`) | Show basic/full readiness + background index state |
| `cgrep mcp <serve|install|uninstall>` | MCP server + host config integration |
| `cgrep agent <...>` (`a`) | Agent locate/expand + integration install |
| `cgrep completions <shell>` | Generate shell completions |

## grep/rg migration quick path

```bash
# grep -R "token validation" src/
cgrep search "token validation" src/

# grep/rg + manual open loop
cgrep d handle_auth
cgrep r UserService
cgrep rd src/auth.rs
cgrep mp -d 2
```

- Use `cgrep search` (or `cgrep s`) for text search.
- Option-first form is still supported: `cgrep search -r --include '**/*.rs' needle src/`.
- If query starts with `-`, use `--` after `search` (e.g., `cgrep search -- --literal`).
- If query starts with `-` and you also pass scope flags, place flags/path before `--`
  (e.g., `cgrep search -p src -- --help`).
- grep-style scope flags are supported: `-r/--recursive`, `--no-recursive`, `--include`, `--exclude-dir`.
- `--no-ignore` forces scan mode and disables `.gitignore`/`.ignore` filtering during scan.
- `-p <path>` remains available when you prefer explicit path flags.
- Empty/whitespace queries are rejected consistently in every search mode (including `--regex`).
- `cgrep read` rejects empty paths (`Error: Path cannot be empty`).
- `search` result `path` is always round-trip safe:
  workspace-internal scopes return workspace-relative paths, and external scopes return absolute paths.

Validation examples:

```bash
cgrep search ""            # Error: Search query cannot be empty
cgrep search --regex ""    # Error: Search query cannot be empty
cgrep read ""              # Error: Path cannot be empty
```

## Shortcut-first flow

```bash
cgrep i                           # index
cgrep s "authentication flow"     # search
cgrep d handle_auth               # definition
cgrep r UserService               # references
cgrep c validate_token            # callers
cgrep dep src/auth.rs             # dependents
cgrep a l "token validation" -B tight -u
```

## Human quick start

```bash
# 1) Build index
cgrep index

# 2) Core 5 commands
cgrep search "authentication flow" src/
cgrep d handle_auth
cgrep r UserService
cgrep rd src/auth.rs
cgrep mp -d 2

# 3) Optional narrowing / changed-files
cgrep search "token refresh" -t rust -p src/
cgrep search "retry logic" -u
```

## Index flags

```bash
# Default incremental index path (manifest enabled)
cgrep index

# Print added/modified/deleted diff from manifest
cgrep index --print-diff

# Refresh only manifest metadata (no document reindex)
cgrep index --manifest-only --print-diff

# Disable manifest path and use legacy incremental flow
cgrep index --no-manifest

# Build full index in background and return immediately
cgrep index --background
```

## Status guide

```bash
# Human-readable status
cgrep status

# Structured status for agents/automation
cgrep --format json2 --compact status
```

## Search guide

Use `search` (or alias `s`) explicitly:

Core options:

```bash
cgrep search "<query>" \
  -p <path> \
  -r | --no-recursive \
  -m <limit> \
  -C <context> \
  -i | --ignore-case \
  --case-sensitive \
  -t <language> \
  --glob|--include <pattern> \
  -x, --exclude|--exclude-dir <pattern> \
  --no-ignore \
  -u, --changed [REV] \
  -M, --mode keyword|semantic|hybrid \
  -B, --budget tight|balanced|full|off \
  -P, --profile human|agent|fast
```

Examples:

```bash
cgrep search "jwt decode" -m 10
cgrep s "retry backoff" -u
cgrep search -r --no-ignore "token validation" src/
cgrep s "controller middleware" -B tight -P agent
```

### Modes

```bash
cgrep search "token refresh" --mode keyword   # default
cgrep search "token refresh" --mode semantic  # requires embeddings + index
cgrep search "token refresh" --mode hybrid    # requires embeddings + index
```

Mode notes:
- `keyword` uses index when present, otherwise scan fallback
- `semantic/hybrid` require index; no scan fallback

Deprecated compatibility aliases:
- `--keyword`, `--semantic`, `--hybrid` (use `--mode`)

### Budget presets

| Preset | Intent |
|---|---|
| `tight` | Minimal payload / strict token control |
| `balanced` | Default agent-oriented balance |
| `full` | More context, larger payload |
| `off` | No preset budget limits |

### Profiles

| Profile | Typical use |
|---|---|
| `human` | Readable interactive output |
| `agent` | Structured + token-efficient defaults |
| `fast` | Quick exploratory search |

### Advanced options

```bash
cgrep search --help-advanced
```

Common advanced flags:
- `--no-index`, `--fuzzy`
- `--agent-cache`, `--cache-ttl`
- `--context-pack`
- `--max-chars-per-snippet`, `--max-context-chars`, `--max-total-chars`
- `--path-alias`, `--dedupe-context`, `--suppress-boilerplate`

## Read guide

```bash
cgrep read src/auth.rs
cgrep read src/auth.rs --section 120-220
cgrep read docs/usage.md --section "Search guide"
cgrep read src/auth.rs --full
```

Notes:
- `read` expects a non-empty file path.
- `--section` accepts either a line range (`start-end`) or a markdown heading.
- `--full` disables smart outline mode and prints the full file.

## Map guide

```bash
cgrep map
cgrep map -d 2
cgrep map -p src -d 3
```

Notes:
- Default depth is `3`.
- Use `-p` to focus on a subtree before running follow-up `search/read`.

## Definition guide (optional tuning)

```bash
cgrep definition <name> \
  -p <path> \
  -m <limit>
```

Notes:
- In most repos, plain `cgrep d <name>` is enough.
- Use `-p` only when you intentionally want to constrain lookup to a subtree.
- Use `-m` only when you need a stricter payload budget (default: `20`).

## Output formats

Global flags:

```bash
--format text|json|json2
--compact
```

Format summary:
- `text`: human-readable
- `json`: simple array/object payload
- `json2`: structured payload for automation/agents

## Supported languages

AST symbol extraction:
- typescript, tsx, javascript, python, rust, go, c, cpp, java, ruby

Index/scan extensions:
- rs, ts, tsx, js, jsx, py, go, java, c, cpp, h, hpp, cs, rb, php, swift
- kt, kts, scala, lua, md, txt, json, yaml, toml
