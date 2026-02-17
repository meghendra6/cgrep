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

## 에이전트 토큰 벤치마크 (PyTorch)

```bash
python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch
```

이 벤치마크는 시나리오 **완료까지 필요한 토큰(tokens-to-complete)** 을 측정합니다.
- baseline: `grep` locate + 파일 스니펫 확장 tier
- cgrep: `agent locate` + `agent expand` ID 확장 tier

tier 조정:

```bash
python3 scripts/benchmark_agent_token_efficiency.py \
  --repo /path/to/pytorch \
  --baseline-file-tiers 2,4,6,8,12 \
  --cgrep-expand-tiers 1,2,4,6,8
```

출력 파일:
- `docs/benchmarks/pytorch-agent-token-efficiency.md`
- `local/benchmarks/pytorch-agent-token-efficiency.json` (로컬 전용)

## Codex 실사용 벤치마크 (PyTorch)

```bash
python3 scripts/benchmark_codex_agent_efficiency.py \
  --repo /path/to/pytorch \
  --cgrep-bin /path/to/cgrep \
  --model gpt-5-codex \
  --reasoning-effort medium \
  --runs 2
```

이 벤치마크는 실제 `codex exec` 세션을 실행하고 provider telemetry를 수집합니다:
- `input_tokens`, `cached_input_tokens`, `output_tokens`
- `billable_tokens = input - cached_input + output`
- 명령 정책 위반 포함 성공/실패율
- `all_cases`와 `success_only` 집계를 함께 보고

출력 파일:
- `docs/benchmarks/pytorch-codex-agent-efficiency.md`
- `local/benchmarks/pytorch-codex-agent-efficiency.json` (로컬 전용)
