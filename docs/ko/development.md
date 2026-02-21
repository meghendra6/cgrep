# 개발

## 일상 검증 루프

```bash
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## 단일 명령 검증 워크플로우

```bash
CGREP_BIN=cgrep bash scripts/validate_all.sh
```

이 워크플로우는 다음을 한 번에 점검합니다.
- 핵심 인덱싱/검색 흐름
- 증분 업데이트 경로(`--print-diff`)
- agent plan 흐름
- status/search 통계 payload 점검(`json2 --compact`)
- 저장소 통합 파일이 있을 때 doctor 흐름(`scripts/doctor.sh`)
- 문서 로컬 링크 sanity 체크(README + docs 허브 파일)

## 문서 사이트 검증 (GitHub Pages)

로컬 미리보기:

```bash
mkdocs serve
```

CI(`docs-pages` 워크플로우)와 동일한 엄격 빌드:

```bash
mkdocs build --strict
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

```bash
python3 scripts/agent_plan_perf_gate.py \
  --baseline-bin /path/to/baseline/cgrep \
  --candidate-bin /path/to/candidate/cgrep \
  --runs 5 \
  --warmup 2 \
  --files 800
```

검색/인덱싱 관련 변경 뒤에는 반드시 실행하세요.

성능 게이트는 다음 지표의 지연시간 `p50`/`p95`를 추적합니다.
- `--reuse off` 기준 fresh worktree 인덱싱 지연
- `--reuse off` 이후 첫 keyword 검색 지연
- 작은 tracked 파일 변경 후 증분 인덱스 업데이트 지연(`--reuse off`)
- `--reuse strict` 기준 fresh worktree 인덱싱 지연
- `--reuse auto` 기준 fresh worktree 인덱싱 지연
- `--reuse strict` 이후 첫 keyword 검색 지연
- `--reuse auto` 이후 첫 keyword 검색 지연
- identifier-like 단순 쿼리 기준 agent plan 지연
- phrase-like 복합 쿼리 기준 agent plan 지연
- expand-heavy 쿼리 기준 agent plan end-to-end 지연

측정 방법:
- `--warmup`: 지표별 워밍업 실행(리포트 미포함)
- `--runs`: 지표별 측정 실행 횟수
- `p50`: median
- `p95`: nearest-rank percentile

CI 임계값(중앙값 기준):
- 검색 회귀 > `5%`: 실패
- cold index build 회귀 > `10%`: 실패
- 증분/reuse 업데이트 회귀 > `10%`: 실패
- agent plan 회귀 > `10%`: 실패
- agent-plan 성능 체크에서는 작은 절대 편차(`<= 3ms`)를 노이즈로 처리

## 릴리즈 전 체크리스트

- 빌드 통과 (`cargo build`)
- 테스트 통과 (`cargo test`)
- Clippy 경고 0개 (`-D warnings`)
- 검증 워크플로우 통과 (`scripts/validate_all.sh`)
- 성능 게이트 통과 (`scripts/index_perf_gate.py`, `scripts/agent_plan_perf_gate.py`)
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

최신 측정 스냅샷은 벤치마크 문서 본문에 유지됩니다.
