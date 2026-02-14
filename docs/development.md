# Development

## Build and checks

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Performance gate

```bash
python3 scripts/perf_gate.py
```

Run the performance gate after search/indexing changes to catch regressions.
