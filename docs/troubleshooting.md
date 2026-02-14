# Troubleshooting

- `semantic/hybrid` returns errors or weak results:
  - run `cgrep index`
  - verify embeddings settings in config
- Running from subdirectories misses files:
  - set explicit scope with `-p`
- Output is too large for agents:
  - use `--budget tight` or `--profile agent`
- No index present:
  - `keyword` mode falls back to scan
  - `semantic/hybrid` modes require index
