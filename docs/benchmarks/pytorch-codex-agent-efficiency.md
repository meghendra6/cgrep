# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-22T05:47:57.997467+00:00

## What This Measures

- Real `codex exec` runs on a local PyTorch repository.
- Baseline mode: autonomous retrieval with cgrep disallowed.
- cgrep mode: cgrep command usage required.
- Primary metric: Codex provider-reported billable tokens (`input - cached_input + output`).

## Scenario Set

- `autograd_evaluate_function`: autograd engine `evaluate_function` trace.
- `tensor_iterator_impl`: `TensorIterator` declaration and implementation path.
- `python_arg_parser_impl`: `PythonArgParser` declaration/implementation path.
- `dispatch_key_set`: `DispatchKeySet` representation and references.
- `cuda_graph`: `CUDAGraph` implementation-related path.
- `addmm_path`: `addmm` implementation and call path.

For each scenario:
- Success requires all marker groups to be satisfied from returned evidence.
- Baseline allows only `grep/rg/sed/cat/head/tail/git`.
- cgrep mode requires `cgrep search|s` or `cgrep definition|d` only.
- Disallowed command usage or missing required tool usage marks the run as failed.

## Environment

- OS: `macOS-26.3-arm64-arm-64bit`
- Python: `3.12.4`
- codex model: `gpt-5-codex`
- reasoning effort: `medium`
- runs per scenario/mode: `1`
- cgrep commit: `be95ef6`
- pytorch commit: `66e77ae932c`
- PyTorch files (`git ls-files`): `21634`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 6 | 83.3% | 19476 | 56329 | 24499 | 12492.2 | 1.5 |
| `cgrep` | 6 | 100.0% | 17712 | 31822 | 29756 | 17556.4 | 2.0 |

- Total billable tokens (baseline, no cgrep): **158,242**
- Total billable tokens (cgrep): **107,990**
- Billable token reduction: **31.8%**

## Observed Run Variance (Same Day, Same Env)

- Run A (2026-02-22 05:43 UTC): baseline **74,786** vs cgrep **176,057** (one baseline timeout + one cgrep long-loop outlier).
- Run B (2026-02-22 05:47 UTC, shown above): baseline **158,242** vs cgrep **107,990**.
- Practical guidance: use `--runs >= 2` and compare medians, not single-run totals, for release decisions.

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 17,098 | 17,098 | 11317.2 | 1 |
| 1 | `autograd_evaluate_function` | `cgrep` | yes | 17,176 | 17,176 | 6631.0 | 1 |
| 1 | `tensor_iterator_impl` | `baseline` | yes | 61,439 | 95,615 | 31028.1 | 7 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 34,314 | 112,778 | 43715.0 | 10 |
| 1 | `python_arg_parser_impl` | `baseline` | yes | 21,854 | 28,894 | 13437.5 | 2 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 2,035 | 17,139 | 5454.1 | 1 |
| 1 | `dispatch_key_set` | `baseline` | no | 40,998 | 90,534 | 47148.2 | 5 |
| 1 | `dispatch_key_set` | `cgrep` | yes | 11,870 | 26,974 | 13510.2 | 2 |
| 1 | `cuda_graph` | `baseline` | yes | 4,872 | 20,104 | 11547.0 | 1 |
| 1 | `cuda_graph` | `cgrep` | yes | 24,346 | 32,538 | 21602.6 | 2 |
| 1 | `addmm_path` | `baseline` | yes | 11,981 | 20,045 | 9175.4 | 1 |
| 1 | `addmm_path` | `cgrep` | yes | 18,249 | 76,105 | 33242.8 | 6 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
