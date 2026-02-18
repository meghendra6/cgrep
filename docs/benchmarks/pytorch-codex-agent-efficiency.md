# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-18T08:46:48.528540+00:00

> Snapshot note: these numbers were collected at cgrep commit `47fc4cc`. They are historical benchmark results, not a guarantee for every later release.

## What This Measures

- Real `codex exec` runs on a local PyTorch repository.
- Baseline mode: grep/sed/cat style retrieval.
- cgrep mode: cgrep-based retrieval commands.
- Primary metric: Codex provider-reported billable tokens (`input - cached_input + output`).

## Environment

- OS: `macOS-26.2-arm64-arm-64bit`
- Python: `3.12.4`
- codex model: `gpt-5-codex`
- reasoning effort: `medium`
- runs per scenario/mode: `2`
- cgrep commit: `47fc4cc`
- pytorch commit: `b7abe8e3ab9`
- PyTorch files (`git ls-files`): `20437`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 12 | 100.0% | 19066 | 30550 | 30172 | 15759.7 | 2.0 |
| `cgrep` | 12 | 100.0% | 12470 | 16483 | 16103 | 6862.0 | 1.0 |

- Total billable tokens (baseline): **233,825**
- Total billable tokens (cgrep): **134,432**
- Billable token reduction: **42.5%**

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 26,970 | 26,970 | 14307.2 | 2 |
| 1 | `autograd_evaluate_function` | `cgrep` | yes | 8,326 | 16,134 | 9896.3 | 1 |
| 1 | `tensor_iterator_impl` | `baseline` | yes | 14,210 | 32,898 | 27401.4 | 2 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 16,397 | 24,333 | 14532.5 | 2 |
| 1 | `python_arg_parser_impl` | `baseline` | yes | 19,127 | 28,599 | 10939.4 | 2 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 15,921 | 15,921 | 5898.0 | 1 |
| 1 | `dispatch_key_set` | `baseline` | yes | 25,786 | 32,826 | 13560.9 | 2 |
| 1 | `dispatch_key_set` | `cgrep` | yes | 16,185 | 16,185 | 6333.1 | 1 |
| 1 | `cuda_graph` | `baseline` | yes | 18,689 | 18,689 | 9282.9 | 1 |
| 1 | `cuda_graph` | `cgrep` | yes | 8,376 | 16,184 | 6499.9 | 1 |
| 1 | `addmm_path` | `baseline` | yes | 19,354 | 19,354 | 9652.7 | 1 |
| 1 | `addmm_path` | `cgrep` | yes | 2,180 | 16,260 | 7766.9 | 1 |
| 2 | `autograd_evaluate_function` | `cgrep` | yes | 15,940 | 15,940 | 7947.3 | 1 |
| 2 | `autograd_evaluate_function` | `baseline` | yes | 10,448 | 27,728 | 17212.3 | 2 |
| 2 | `tensor_iterator_impl` | `cgrep` | yes | 16,589 | 24,397 | 9113.2 | 2 |
| 2 | `tensor_iterator_impl` | `baseline` | yes | 19,006 | 33,086 | 31912.3 | 2 |
| 2 | `python_arg_parser_impl` | `cgrep` | yes | 16,053 | 16,053 | 7224.2 | 1 |
| 2 | `python_arg_parser_impl` | `baseline` | yes | 14,209 | 31,745 | 35123.0 | 2 |
| 2 | `dispatch_key_set` | `cgrep` | yes | 1,182 | 16,030 | 3932.0 | 1 |
| 2 | `dispatch_key_set` | `baseline` | yes | 34,926 | 34,926 | 22417.2 | 2 |
| 2 | `cuda_graph` | `cgrep` | yes | 9,019 | 16,059 | 4861.5 | 1 |
| 2 | `cuda_graph` | `baseline` | yes | 4,873 | 18,953 | 9574.6 | 1 |
| 2 | `addmm_path` | `cgrep` | yes | 8,264 | 16,072 | 4765.6 | 1 |
| 2 | `addmm_path` | `baseline` | yes | 26,227 | 33,267 | 18991.2 | 2 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
