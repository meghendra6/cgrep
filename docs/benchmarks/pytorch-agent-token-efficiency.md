# PyTorch AI Agent Token Efficiency Benchmark

Generated: 2026-02-16T11:58:18.234788+00:00

## What This Measures

1. **Baseline (without cgrep):** `grep` locate + incremental snippet expansion tiers.
2. **With cgrep:** `agent locate` once + incremental `agent expand` ID tiers.
3. **Completion rule:** scenario is complete when each marker-group has at least one match in cumulative tool outputs.
4. **Primary metric:** cumulative tokens consumed until completion (`tokens-to-complete`).
5. **Tokenizer:** OpenAI `cl100k_base` when available (fallback: byte/4 approximation).

## Environment

- OS: `macOS-26.2-arm64-arm-64bit`
- cgrep commit: `b2d160d`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`
- Tokenizer: `tiktoken:cl100k_base`
- Baseline file tiers: `[2, 4, 6, 8, 12]`
- cgrep expand tiers: `[1, 2, 4, 6, 8]`

## Results

| Scenario | Representative coding task | Baseline done | cgrep done | Baseline attempts | cgrep attempts | Baseline tokens-to-complete | cgrep tokens-to-complete | Reduction | Baseline latency (ms) | cgrep latency (ms) |
|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| Find where autograd engine evaluate_function is implemented and inspected. | Patch autograd evaluate_function flow and verify the implementation file + autograd context. | yes | yes | 1 | 1 | 7,681 | 939 | 87.8% | 1853.05 | 20.75 |
| Find TensorIterator definition and major implementation usage points. | Prepare a TensorIterator behavior change by locating the core declaration and implementation paths. | yes | yes | 1 | 1 | 43,265 | 1,026 | 97.6% | 1151.19 | 20.03 |
| Locate PythonArgParser implementation and usage points. | Implement a parser-related fix by gathering PythonArgParser definition and source implementation. | yes | yes | 1 | 1 | 6,742 | 1,001 | 85.2% | 1081.20 | 21.84 |
| Understand DispatchKeySet representation and references. | Refactor DispatchKeySet logic with confidence by finding its representation and core references. | yes | yes | 1 | 1 | 43,738 | 1,028 | 97.6% | 1034.56 | 21.78 |
| Locate CUDAGraph implementation-related code quickly. | Make a CUDAGraph code-path update by collecting implementation and CUDA path context. | yes | yes | 1 | 1 | 11,361 | 1,018 | 91.0% | 1047.23 | 21.18 |
| Find addmm implementation and call sites. | Modify addmm behavior by locating native implementation and addmm_out call path. | yes | yes | 1 | 1 | 15,692 | 1,148 | 92.7% | 1538.34 | 21.11 |

## Aggregate

- One-time index build: **4.87s**
- Scenarios completed (baseline): **6/6**
- Scenarios completed (cgrep): **6/6**
- Baseline tokens-to-complete (total): **128,479**
- cgrep tokens-to-complete (total): **6,160**
- Token reduction (to completion): **95.2%**
- Token compression ratio (baseline/cgrep): **20.86x**
- Baseline total latency to completion: **7705.58ms**
- cgrep total latency to completion: **126.70ms**

## Coding Readiness Snapshot

The same scenarios can be interpreted as coding tasks where the agent must gather enough context to start an implementation change.

- Tasks ready (baseline): **6/6** (100.0%)
- Tasks ready (cgrep): **6/6** (100.0%)
- Baseline avg tokens to readiness: **21413**
- cgrep avg tokens to readiness: **1027**
- Token reduction to readiness: **95.2%**
- Baseline avg attempts: **1.00**
- cgrep avg attempts: **1.00**

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
