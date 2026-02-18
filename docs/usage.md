# Usage

## Command overview

| Command | Description |
|---|---|
| `cgrep [search] <query> [path]` (`s`, `find`, `q`) | Full-text search (`search` keyword optional) |
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
| `cgrep mcp <serve|install|uninstall>` | MCP server + host config integration |
| `cgrep agent <...>` (`a`) | Agent locate/expand + integration install |
| `cgrep completions <shell>` | Generate shell completions |

## grep/rg migration quick path

```bash
# grep -R "token validation" src/
cgrep "token validation" src/

# grep/rg + manual open loop
cgrep d handle_auth
cgrep r UserService -M auto
cgrep rd src/auth.rs
cgrep mp -d 2
```

- `cgrep "query" src/` is supported for direct grep-style usage.
- Direct mode also accepts option-first form: `cgrep -r --include '**/*.rs' needle src/`.
- If query starts with `-` or overlaps a command name, use `--` (e.g., `cgrep -- --literal`, `cgrep -- read`).
- grep-style scope flags are supported: `-r/--recursive`, `--no-recursive`, `--include`, `--exclude-dir`.
- `--no-ignore` forces scan mode and disables `.gitignore`/`.ignore` filtering during scan.
- `-p <path>` remains available when you prefer explicit path flags.

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
cgrep "authentication flow" src/
cgrep d handle_auth
cgrep r UserService -M auto
cgrep rd src/auth.rs
cgrep mp -d 2

# 3) Optional narrowing / changed-files
cgrep "token refresh" -t rust -p src/
cgrep "retry logic" -u
```

## Search guide

`search` keyword is optional. These are equivalent:

```bash
cgrep "token refresh" src/
cgrep search "token refresh" src/
```

Core options:

```bash
cgrep search "<query>" \
  -p <path> \
  -r | --no-recursive \
  -m <limit> \
  -C <context> \
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
cgrep -r --no-ignore "token validation" src/
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
