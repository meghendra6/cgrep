# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-21T11:15:12.835453+00:00

## What This Measures

- Real `codex exec` runs on a local PyTorch repository.
- Baseline mode: autonomous retrieval with cgrep disallowed.
- cgrep mode: cgrep command usage required.
- Primary metric: Codex provider-reported billable tokens (`input - cached_input + output`).

## Environment

- OS: `macOS-26.3-arm64-arm-64bit`
- Python: `3.12.4`
- codex model: `gpt-5-codex`
- reasoning effort: `medium`
- runs per scenario/mode: `1`
- cgrep commit: `2f3dd11`
- pytorch commit: `66e77ae932c`
- PyTorch files (`git ls-files`): `21634`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 6 | 83.3% | 15452 | 35525 | 29997 | 17726.7 | 2.0 |
| `cgrep` | 6 | 100.0% | 7858 | 16593 | 26592 | 11501.0 | 2.0 |

- Total billable tokens (baseline, no cgrep): **104,456**
- Total billable tokens (cgrep): **51,035**
- Billable token reduction: **51.1%**

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 3,334 | 17,414 | 6266.2 | 1 |
| 1 | `autograd_evaluate_function` | `cgrep` | yes | 10,495 | 25,599 | 11043.7 | 2 |
| 1 | `tensor_iterator_impl` | `baseline` | yes | 18,063 | 39,183 | 18803.2 | 3 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 5,965 | 35,405 | 16343.5 | 3 |
| 1 | `python_arg_parser_impl` | `baseline` | no | 39,306 | 49,674 | 20678.3 | 4 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 2,291 | 17,523 | 8656.0 | 1 |
| 1 | `dispatch_key_set` | `baseline` | yes | 6,731 | 20,811 | 16650.1 | 1 |
| 1 | `dispatch_key_set` | `cgrep` | yes | 18,626 | 27,586 | 21847.1 | 2 |
| 1 | `cuda_graph` | `baseline` | yes | 12,841 | 19,881 | 9676.8 | 1 |
| 1 | `cuda_graph` | `cgrep` | yes | 3,907 | 17,987 | 10471.6 | 1 |
| 1 | `addmm_path` | `baseline` | yes | 24,181 | 90,869 | 23418.2 | 6 |
| 1 | `addmm_path` | `cgrep` | yes | 9,751 | 30,871 | 11958.3 | 2 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
