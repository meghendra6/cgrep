# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-22T07:00:58.654036+00:00

## What This Measures

- Real `codex exec` runs on a local PyTorch repository.
- Baseline mode: autonomous retrieval with cgrep disallowed.
- cgrep mode: cgrep command usage required.
- Primary metric: Codex provider-reported billable tokens (`input - cached_input + output`).

## Scenario Set

- `autograd_evaluate_function`
- `tensor_iterator_impl`
- `python_arg_parser_impl`
- `dispatch_key_set`
- `cuda_graph`
- `addmm_path`

For each scenario:
- Success requires all marker groups to be satisfied from returned evidence.
- Baseline allows only `grep/rg/sed/cat/head/tail/git` commands.
- cgrep mode requires `cgrep search|s` or `cgrep definition|d` commands.
- Disallowed command usage or missing required tool usage marks the run as failed.

> Single-run variance can be high. Prefer `--runs >= 2` and compare medians for release decisions.

## Environment

- OS: `macOS-26.3-arm64-arm-64bit`
- Python: `3.12.4`
- codex model: `gpt-5-codex`
- reasoning effort: `medium`
- runs per scenario/mode: `2`
- cgrep commit: `b24ca61`
- pytorch commit: `66e77ae932c`
- PyTorch files (`git ls-files`): `21634`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 12 | 91.7% | 6315 | 25702 | 31937 | 16087.4 | 2.5 |
| `cgrep` | 12 | 100.0% | 5632 | 21236 | 28288 | 14495.0 | 2.0 |

- Total billable tokens (baseline, no cgrep): **120,851**
- Total billable tokens (cgrep): **100,007**
- Billable token reduction: **17.2%**

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 7,012 | 37,860 | 18714.4 | 3 |
| 1 | `autograd_evaluate_function` | `cgrep` | yes | 3,005 | 26,429 | 15473.4 | 2 |
| 1 | `tensor_iterator_impl` | `baseline` | yes | 20,822 | 80,982 | 25657.8 | 5 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 6,672 | 36,624 | 11530.9 | 3 |
| 1 | `python_arg_parser_impl` | `baseline` | yes | 22,315 | 97,835 | 24319.0 | 7 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 5,978 | 28,250 | 13516.5 | 2 |
| 1 | `dispatch_key_set` | `baseline` | yes | 4,471 | 19,703 | 8930.1 | 1 |
| 1 | `dispatch_key_set` | `cgrep` | yes | 7,205 | 28,325 | 12502.3 | 2 |
| 1 | `cuda_graph` | `baseline` | yes | 5,618 | 19,698 | 8390.6 | 1 |
| 1 | `cuda_graph` | `cgrep` | yes | 2,276 | 17,508 | 7487.5 | 1 |
| 1 | `addmm_path` | `baseline` | yes | 4,836 | 20,068 | 11201.8 | 1 |
| 1 | `addmm_path` | `cgrep` | yes | 23,739 | 75,579 | 35276.3 | 6 |
| 2 | `autograd_evaluate_function` | `cgrep` | yes | 4,218 | 27,002 | 15608.4 | 2 |
| 2 | `autograd_evaluate_function` | `baseline` | yes | 3,742 | 26,014 | 13460.3 | 2 |
| 2 | `tensor_iterator_impl` | `cgrep` | yes | 14,756 | 45,604 | 16940.6 | 4 |
| 2 | `tensor_iterator_impl` | `baseline` | yes | 7,049 | 48,137 | 25005.4 | 4 |
| 2 | `python_arg_parser_impl` | `cgrep` | yes | 3,997 | 27,549 | 9057.5 | 2 |
| 2 | `python_arg_parser_impl` | `baseline` | yes | 1,861 | 16,965 | 6256.5 | 1 |
| 2 | `dispatch_key_set` | `cgrep` | yes | 5,286 | 29,094 | 17925.4 | 2 |
| 2 | `dispatch_key_set` | `baseline` | no | 29,841 | 142,737 | 41785.0 | 9 |
| 2 | `cuda_graph` | `cgrep` | yes | 3,687 | 17,767 | 7570.6 | 1 |
| 2 | `cuda_graph` | `baseline` | yes | 5,133 | 20,365 | 12455.7 | 1 |
| 2 | `addmm_path` | `cgrep` | yes | 19,188 | 41,588 | 24247.9 | 3 |
| 2 | `addmm_path` | `baseline` | yes | 8,151 | 46,295 | 20121.7 | 3 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
