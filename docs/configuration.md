# Configuration

## Precedence

1. `<repo>/.cgreprc.toml`
2. `~/.config/cgrep/config.toml`

## Example

```toml
max_results = 20
exclude_patterns = ["target/**", "node_modules/**"]

[search]
default_mode = "keyword"

[ranking]
enabled = true
path_weight = 1.2
symbol_weight = 1.8
language_weight = 1.0
changed_weight = 1.2
kind_weight = 2.0
weak_signal_penalty = 1.4
explain_top_k = 5

[cache]
ttl_ms = 600000

[index]
exclude_paths = ["vendor/", "dist/"]
respect_git_ignore = true

[profile.agent]
format = "json2"
max_results = 50
context = 6
context_pack = 8
mode = "keyword"
agent_cache = true

[embeddings]
provider = "builtin" # builtin|command|dummy
batch_size = 4      # lower = less memory, often faster on CPU
# max_chars = 2000   # trim per-symbol text before embedding
# command = "embedder"
# model = "local-model-id"
```

## Ranking tuning

- `[ranking] enabled` defaults to `false` for compatibility.
- When disabled, keyword ranking behavior remains legacy-equivalent.
- Weights are bounded for safety:
  - `path_weight`, `symbol_weight`, `language_weight`, `changed_weight`, `kind_weight`, `weak_signal_penalty`: `0.0..=3.0`
  - `explain_top_k`: `1..=50` (default `5`)
- Out-of-range or non-finite values fall back to safe defaults.

## Deterministic output defaults

- For automation/agents, set profile defaults to deterministic output:
  - `[profile.agent].format = "json2"`
  - use CLI `--compact` for stable machine parsing
- Optional payload fields are omitted when empty; consumers should not require `null` placeholders.
- Request timing fields (for example `elapsed_ms`) are informational, not ordering keys.

## Index Ignore Policy

- `cgrep index` now respects `.gitignore`/`.ignore` by default.
- Use `cgrep index --include-ignored` to opt out and include ignored paths.
- Use `cgrep index --include-path <path>` (repeatable) to include specific ignored paths only.
- Config equivalent: `[index] respect_git_ignore = true|false` (default `true`).

## Daemon index profile reuse

- `cgrep daemon` reuses the latest index profile stored in `.cgrep/metadata.json`.
- The reused profile preserves the options from the latest `cgrep index` run as-is.
- If no stored profile exists yet, daemon falls back to `[index]` config defaults.

## Artifact compatibility notes

- `.cgrep/status.json`: background/index readiness state.
- `.cgrep/reuse-state.json`: reuse decisions and fallback reasons (optional).
- `.cgrep/manifest/` + `.cgrep/metadata.json`: incremental/reuse metadata.

These files are additive runtime artifacts. Existing consumers can ignore unknown optional fields safely.
