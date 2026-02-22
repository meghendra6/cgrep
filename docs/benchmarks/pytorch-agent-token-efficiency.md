# PyTorch AI Agent Token Efficiency Benchmark

Generated: 2026-02-22T05:40:03.631143+00:00

> Snapshot note: benchmark outputs vary by repository state and model behavior.
> Compare trends over repeated runs rather than relying on one run.

## What This Measures

1. **Baseline (without cgrep):** `grep` locate + incremental snippet expansion tiers.
2. **With cgrep:** `agent locate` once + incremental `agent expand` ID tiers.
3. **Completion rule:** scenario is complete when each marker-group has at least one match in cumulative tool outputs.
4. **Primary metric:** cumulative tokens consumed until completion (`tokens-to-complete`).
5. **Tokenizer:** OpenAI `cl100k_base` when available (fallback: byte/4 approximation).

## Environment

- OS: `macOS-26.3-arm64-arm-64bit`
- cgrep commit: `be95ef6`
- pytorch commit: `66e77ae932c`
- PyTorch files (`git ls-files`): `21634`
- Tokenizer: `tiktoken:cl100k_base`
- Baseline file tiers: `[2, 4, 6, 8, 12]`
- cgrep expand tiers: `[1, 2, 4, 6, 8]`

## Results

| Scenario | Representative coding task | Baseline done | cgrep done | Baseline attempts | cgrep attempts | Baseline tokens-to-complete | cgrep tokens-to-complete | Reduction | Baseline latency (ms) | cgrep latency (ms) |
|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| Find where autograd engine evaluate_function is implemented and inspected. | Patch autograd evaluate_function flow and verify the implementation file + autograd context. | yes | yes | 1 | 1 | 7,488 | 1,071 | 85.7% | 2205.27 | 118.21 |
| Find TensorIterator definition and major implementation usage points. | Prepare a TensorIterator behavior change by locating the core declaration and implementation paths. | yes | yes | 1 | 1 | 42,834 | 1,098 | 97.4% | 1269.47 | 23.12 |
| Locate PythonArgParser implementation and usage points. | Implement a parser-related fix by gathering PythonArgParser definition and source implementation. | yes | yes | 1 | 1 | 6,846 | 1,049 | 84.7% | 1173.61 | 118.82 |
| Understand DispatchKeySet representation and references. | Refactor DispatchKeySet logic with confidence by finding its representation and core references. | yes | yes | 1 | 1 | 43,781 | 1,120 | 97.4% | 1150.99 | 26.29 |
| Locate CUDAGraph implementation-related code quickly. | Make a CUDAGraph code-path update by collecting implementation and CUDA path context. | yes | yes | 1 | 1 | 12,593 | 1,107 | 91.2% | 1171.03 | 120.72 |
| Find addmm implementation and call sites. | Modify addmm behavior by locating native implementation and addmm_out call path. | yes | yes | 1 | 1 | 15,677 | 1,222 | 92.2% | 1721.68 | 24.04 |

## Aggregate

- One-time index build: **5.37s**
- Scenarios completed (baseline): **6/6**
- Scenarios completed (cgrep): **6/6**
- Baseline tokens-to-complete (total): **129,219**
- cgrep tokens-to-complete (total): **6,667**
- Token reduction (to completion): **94.8%**
- Token compression ratio (baseline/cgrep): **19.38x**
- Baseline total latency to completion: **8692.04ms**
- cgrep total latency to completion: **431.20ms**

## Re-run

```bash
python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch
```

## Periodic Tracking

```bash
python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch --history-dir local/benchmarks/history
```

```cron
0 3 * * 1 cd /path/to/cgrep && python3 scripts/benchmark_agent_token_efficiency.py --repo /path/to/pytorch --history-dir local/benchmarks/history
```
