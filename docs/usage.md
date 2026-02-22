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
| `cgrep daemon <start|status|stop>` (`bg`) | Manage background indexing daemon |
| `cgrep status` (`st`) | Show basic/full readiness + background index state |
| `cgrep mcp <serve|install|uninstall>` | MCP server + host config integration |
| `cgrep agent <...>` (`a`) | Agent plan/locate/expand + integration install |
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
- `search`/`symbols`/`definition`/`references`/`callers`/`dependents` and `agent locate|plan` auto-bootstrap and call-driven refresh index by default, so manual `cgrep index` is optional for normal use.
- CLI auto-index change checks are debounced in tight command loops to reduce repeated scan overhead.
- `search --no-index` always keeps scan-only behavior.

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
cgrep a p "trace auth middleware flow"
cgrep a l "token validation" -B tight -u
```

## Human quick start

```bash
# 1) Optional warm-up index (not required)
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

# Reuse compatible local cache snapshot (exact HEAD)
cgrep index --reuse strict

# Reuse nearest compatible local snapshot
cgrep index --reuse auto

# Explicitly disable reuse (default)
cgrep index --reuse off
```

## Status guide

```bash
# Human-readable status
cgrep status

# Structured status for agents/automation
cgrep --format json2 --compact status
```

`status` includes optional `reuse` details when reuse is attempted:
- `mode`, `decision`, `active`
- `source`, `snapshot_key`, `repo_key`, `reason` (when available)

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
  --explain \
  -B, --budget tight|balanced|full|off \
  -P, --profile human|agent|fast  # aliases: user/ai/quick
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
cgrep search "token refresh" --mode semantic  # experimental, requires embeddings + index
cgrep search "token refresh" --mode hybrid    # experimental, requires embeddings + index
```

Mode notes:
- `keyword` uses index when present, otherwise scan fallback
- `semantic/hybrid` are **experimental**, require index, and have no scan fallback

Deprecated compatibility aliases:
- `--keyword`, `--semantic`, `--hybrid` (use `--mode`)

### Keyword ranking + explain

```bash
# deterministic score breakdown for top matches (keyword mode)
cgrep --format json2 --compact search "target_fn" --explain
```

Ranking notes:
- Multi-signal keyword ranking is config-gated via `[ranking] enabled = true`.
- Default (`enabled = false`) keeps legacy keyword ordering.
- Query classifier is deterministic:
  - `identifier-like`: single token with `[A-Za-z0-9_:. $]` characters only.
  - `phrase-like`: everything else (including whitespace).
- Stable tie-break order:
  1. final score (desc)
  2. path (asc)
  3. line (asc)
  4. snippet (asc)

`--explain` emits for top K results (`[ranking].explain_top_k`, default `5`):
- `bm25`
- `path_boost` (includes language-filter match boost)
- `symbol_boost`
- `changed_boost`
- `kind_boost`
- `penalties`
- `final_score`

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
| `human` (`user`, `developer`, `dev`) | Readable interactive output |
| `agent` (`ai`, `ai-agent`, `coding-agent`) | Structured + token-efficient defaults |
| `fast` (`quick`) | Quick exploratory search |

Scenario quick recipes:

```bash
# user-focused interactive workflow
cgrep s "auth refresh token" -P user -C 2

# AI coding-agent workflow (deterministic + compact)
cgrep s "auth refresh token" -P ai -B tight --format json2 --compact
```

### Advanced options

```bash
cgrep search --help-advanced
```

Common advanced flags:
- `--no-index`, `--fuzzy`
- `--explain`
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

## Deterministic `json2`/`--compact` contract

Ordering rules:
- top-level object fields are emitted in stable struct order.
- `results[]` are emitted in deterministic ranking order.
- search tie-break order:
  1. score (desc)
  2. path (asc)
  3. line (asc)
  4. snippet (asc)
- agent plan tie-break order:
  1. locate score (desc)
  2. path (asc)
  3. line (asc)
  4. id (asc)

Required and optional field policy:
- required fields are always present for a command's schema (`meta`, `results`, `steps`, `candidates`, `result`).
- optional fields are omitted (not `null`) unless they carry meaningful data.
  Examples:
  - search: `context_before`, `context_after`, `explain`
  - status: `reuse`
  - agent plan: `diagnostics`, `error`
- consumers should parse by field name, not by positional assumptions.

ID stability:
- search/agent IDs are stable for identical repo/query/options/state.
- deterministic mode guarantees stable ordering and field presence; request-level timing fields (for example `elapsed_ms`) remain informational.

## Migration and compatibility notes

Additive flags (default behavior unchanged unless explicitly set):
- search:
  - `--explain`
- index:
  - `--background`
  - `--reuse off|strict|auto` (default `off`)
  - `--manifest-only`
  - `--print-diff`
  - `--no-manifest`
- agent:
  - `agent plan`
  - `agent plan --max-steps`
  - `agent plan --max-candidates`

Compatibility guarantees:
- existing aliases (`s`, `d`, `r`, `c`, `dep`, `i`, `a l`, `a x`) remain valid.
- deprecated mode aliases (`--keyword`, `--semantic`, `--hybrid`) remain accepted.
- `json2` schemas are additive; new optional fields do not break existing required fields.

New `.cgrep/` artifacts to be aware of:
- `.cgrep/status.json`
- `.cgrep/reuse-state.json`
- `.cgrep/manifest/` and `.cgrep/metadata.json` (incremental/reuse metadata)

## Supported languages

AST symbol extraction:
- typescript, tsx, javascript, python, rust, go, c, cpp, java, ruby

Index/scan extensions:
- rs, ts, tsx, js, jsx, py, go, java, c, cpp, h, hpp, cs, rb, php, swift
- kt, kts, scala, lua, md, txt, json, yaml, toml
