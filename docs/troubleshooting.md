# Troubleshooting

## Quick Symptom Table

| Symptom | Likely cause | Action |
|---|---|---|
| `semantic/hybrid` errors or weak results | missing/stale index or embeddings setup | run `cgrep index`, then verify embeddings config |
| results look incomplete from subdirectories | scope mismatch | pass explicit scope: `-p <path>` |
| agent output is too large | payload budget too loose | use `--budget tight` or `--profile agent` |
| semantic/hybrid returns nothing without clear error | embeddings/index not ready | rebuild with `cgrep index --embeddings auto` |
| keyword works but semantic/hybrid fails | expected behavior without index | `keyword` can scan fallback; `semantic/hybrid` require index |

## Fast Recovery Sequence

```bash
cgrep index
cgrep search "sanity check" -m 5
cgrep search "sanity check" --mode keyword -m 5
```
