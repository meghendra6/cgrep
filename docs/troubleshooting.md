# Troubleshooting

## Quick Symptom Table

| Symptom | Likely cause | Action |
|---|---|---|
| `semantic/hybrid` errors or weak results | missing/stale index or embeddings setup | run `cgrep index`, then verify embeddings config |
| results look incomplete from subdirectories | scope mismatch | pass explicit scope: `-p <path>` |
| agent output is too large | payload budget too loose | use `--budget tight` or `--profile agent` |
| semantic/hybrid returns nothing without clear error | embeddings/index not ready | rebuild with `cgrep index --embeddings auto` |
| keyword works but semantic/hybrid fails | expected behavior without index | `keyword` can scan fallback; `semantic/hybrid` require index |
| `Error: Search query cannot be empty` | query is empty/whitespace (including `--regex ""`) | pass a non-empty query string |
| `Error: Path cannot be empty` from `read` | missing path argument | pass a valid file path to `cgrep read <path>` |
| `error: unexpected argument '<path>' found` when query starts with `-` | `--` separator was placed before options/path | put flags/path first, then `--` (e.g., `cgrep search -p src -- --help`) |
| `invalid value 'codex' for '<HOST>'` from `mcp install` | `codex` is not an MCP host value for this command | use `cgrep agent install codex` for Codex, or choose a host from `cgrep mcp install --help` |
| `GLIBC_2.39 not found` on Linux after release install | host glibc is older than the downloaded Linux asset | upgrade to the latest release (Linux builds are now pinned to Ubuntu 22.04 / glibc 2.35 baseline) or install from source (`cargo install --path .`) |

## Fast Recovery Sequence

```bash
cgrep index
cgrep search "sanity check" -m 5
cgrep search "sanity check" --mode keyword -m 5
cgrep --format json2 --compact status
CGREP_BIN=cgrep bash scripts/validate_all.sh
```
