# Embeddings

Embeddings are optional and used by `--mode semantic|hybrid`.
This feature is **experimental** and may change.

## Basic flow

```bash
cgrep index --embeddings auto
cgrep search "natural language query" --mode hybrid
```

If embeddings DB/provider is unavailable, search falls back to BM25-only with a warning.

## Tuning for large repositories

- Exclude build/artifact paths during indexing (example: `-e target/ -e node_modules/ -e .venv/`)
- Lower `[embeddings].batch_size` (recommended range: `2` to `16`)
- Rebuild index after major configuration changes
