# 개발

## 일상 검증 루프

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## 성능 게이트

```bash
python3 scripts/index_perf_gate.py \
  --baseline-bin /path/to/baseline/cgrep \
  --candidate-bin /path/to/candidate/cgrep \
  --runs 3 \
  --warmup 1 \
  --files 1200
```

검색/인덱싱 관련 변경 뒤에는 반드시 실행하세요.

M4 게이트는 다음 지표의 지연시간 `p50`/`p95`와 cold index 처리량을 추적합니다.
- legacy keyword latency
- ranking-enabled keyword latency
- identifier-like keyword latency
- scoped keyword latency
- cold index throughput

측정 방법:
- `--warmup`: 지표별 워밍업 실행(리포트 미포함)
- `--runs`: 지표별 측정 실행 횟수
- `p50`: median
- `p95`: nearest-rank percentile

## 릴리즈 전 체크리스트

- 빌드 통과 (`cargo build`)
- 테스트 통과 (`cargo test`)
- Clippy 경고 0개 (`-D warnings`)
- 성능 게이트 통과 (`scripts/index_perf_gate.py`)
- CLI/동작 변경 시 문서 동기화 완료

## 벤치마크: 에이전트 토큰 효율 (PyTorch)

```bash
python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch
```

tier 조정:

```bash
python3 scripts/benchmark_agent_token_efficiency.py \
  --repo /path/to/pytorch \
  --baseline-file-tiers 2,4,6,8,12 \
  --cgrep-expand-tiers 1,2,4,6,8
```

출력:
- `docs/benchmarks/pytorch-agent-token-efficiency.md`
- `local/benchmarks/pytorch-agent-token-efficiency.json` (로컬 전용)

## 벤치마크: Codex 실사용 효율 (PyTorch)

```bash
python3 scripts/benchmark_codex_agent_efficiency.py \
  --repo /path/to/pytorch \
  --cgrep-bin /path/to/cgrep \
  --model gpt-5-codex \
  --reasoning-effort medium \
  --runs 1
```

수집 항목:
- `input_tokens`, `cached_input_tokens`, `output_tokens`
- `billable_tokens = input - cached_input + output`
- 명령 정책 제약 하 성공/실패율

출력:
- `docs/benchmarks/pytorch-codex-agent-efficiency.md`
- `local/benchmarks/pytorch-codex-agent-efficiency.json` (로컬 전용)

최신 점검 스냅샷 (`2026-02-18`, `runs=1`, `gpt-5-codex`, `medium`):
- baseline `89,764` -> cgrep `21,092` billable tokens (`76.5%` 절감)
