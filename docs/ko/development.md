# 개발

## 빌드와 검증

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## 성능 게이트

```bash
python3 scripts/perf_gate.py
```

검색/인덱싱 로직 변경 후 성능 회귀를 확인할 때 사용하세요.
