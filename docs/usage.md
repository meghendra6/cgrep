# Usage

## Command overview

| Command | Description |
|---|---|
| `cgrep search <query>` (`s`, `find`, `q`) | Full-text search |
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

# 2) Basic search
cgrep search "authentication flow"

# 3) Narrow by language/path
cgrep search "token refresh" -t rust -p src/

# 4) Search only changed files
cgrep search "retry logic" -u

# 5) Symbol/navigation commands
cgrep symbols UserService -T class
cgrep d handle_auth
cgrep c validate_token -M auto
cgrep r UserService -M auto

# 6) Dependency lookup
cgrep dep src/auth.rs

# 7) Smart file reading / map
cgrep rd src/auth.rs
cgrep rd README.md -s "## Configuration"
cgrep mp -d 2
```

## Search guide

Core options:

```bash
cgrep search "<query>" \
  -p <path> \
  -m <limit> \
  -C <context> \
  -t <language> \
  --glob <pattern> \
  -x, --exclude <pattern> \
  -u, --changed [REV] \
  -M, --mode keyword|semantic|hybrid \
  -B, --budget tight|balanced|full|off \
  -P, --profile human|agent|fast
```

Examples:

```bash
cgrep search "jwt decode" -m 10
cgrep s "retry backoff" -u
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
