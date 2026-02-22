# PyTorch Codex Agent Efficiency Benchmark

Generated: 2026-02-22T08:23:14.041127+00:00

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
- Baseline prompt includes a single focused `rg` starter hint (`grep_pattern`) per scenario.
- cgrep mode requires `cgrep search|s` or `cgrep definition|d` commands.
- cgrep prompt includes scenario-specific high-signal starter commands (`cgrep_commands`) and recommends scoped compact output (`--format json2 --compact`).
- Disallowed command usage or missing required tool usage marks the run as failed.

> Single-run variance can be high. Prefer `--runs >= 2` and compare medians for release decisions.

## Environment

- OS: `macOS-26.3-arm64-arm-64bit`
- Python: `3.12.4`
- codex model: `gpt-5-codex`
- reasoning effort: `medium`
- runs per scenario/mode: `2`
- cgrep commit: `7445b45`
- pytorch commit: `66e77ae932c`
- PyTorch files (`git ls-files`): `21634`

## Aggregate (All Cases)

| Mode | Cases | Success rate | Median billable tokens | P95 billable tokens | Median total tokens | Median duration (ms) | Median commands |
|---|---:|---:|---:|---:|---:|---:|---:|
| `baseline` | 12 | 91.7% | 6234 | 34876 | 29722 | 15574.8 | 2.0 |
| `cgrep` | 12 | 100.0% | 3858 | 14537 | 26789 | 14450.5 | 2.0 |

- Total billable tokens (baseline, no cgrep): **151,466**
- Total billable tokens (cgrep): **69,874**
- Billable token reduction: **53.9%**

## Per Scenario

| Run | Scenario | Mode | Success | Billable tokens | Total tokens | Duration (ms) | Commands |
|---:|---|---|---|---:|---:|---:|---:|
| 1 | `autograd_evaluate_function` | `baseline` | yes | 3,371 | 17,451 | 8637.0 | 1 |
| 1 | `autograd_evaluate_function` | `cgrep` | yes | 2,833 | 16,913 | 5620.8 | 1 |
| 1 | `tensor_iterator_impl` | `baseline` | yes | 14,514 | 74,162 | 19344.6 | 5 |
| 1 | `tensor_iterator_impl` | `cgrep` | yes | 21,936 | 70,576 | 42736.2 | 6 |
| 1 | `python_arg_parser_impl` | `baseline` | yes | 6,424 | 28,568 | 8865.1 | 2 |
| 1 | `python_arg_parser_impl` | `cgrep` | yes | 3,910 | 26,182 | 11144.9 | 2 |
| 1 | `dispatch_key_set` | `baseline` | yes | 13,112 | 35,256 | 16284.7 | 2 |
| 1 | `dispatch_key_set` | `cgrep` | yes | 4,716 | 27,116 | 14905.3 | 2 |
| 1 | `cuda_graph` | `baseline` | yes | 5,491 | 19,571 | 5445.0 | 1 |
| 1 | `cuda_graph` | `cgrep` | yes | 3,312 | 17,392 | 9290.4 | 1 |
| 1 | `addmm_path` | `baseline` | yes | 5,583 | 20,815 | 16093.1 | 1 |
| 1 | `addmm_path` | `cgrep` | yes | 7,932 | 45,564 | 22769.3 | 4 |
| 2 | `autograd_evaluate_function` | `cgrep` | yes | 1,838 | 17,198 | 7214.6 | 1 |
| 2 | `autograd_evaluate_function` | `baseline` | yes | 3,352 | 27,800 | 14883.7 | 2 |
| 2 | `tensor_iterator_impl` | `cgrep` | yes | 3,806 | 26,462 | 13995.8 | 2 |
| 2 | `tensor_iterator_impl` | `baseline` | yes | 22,832 | 55,472 | 26557.8 | 4 |
| 2 | `python_arg_parser_impl` | `cgrep` | yes | 5,755 | 36,859 | 18409.4 | 3 |
| 2 | `python_arg_parser_impl` | `baseline` | yes | 6,044 | 30,876 | 15056.5 | 2 |
| 2 | `dispatch_key_set` | `cgrep` | yes | 3,417 | 27,353 | 16441.8 | 2 |
| 2 | `dispatch_key_set` | `baseline` | no | 16,601 | 51,929 | 19693.5 | 3 |
| 2 | `cuda_graph` | `cgrep` | yes | 1,936 | 17,168 | 7189.8 | 1 |
| 2 | `cuda_graph` | `baseline` | yes | 4,545 | 19,649 | 6658.0 | 1 |
| 2 | `addmm_path` | `cgrep` | yes | 8,483 | 48,035 | 21711.1 | 4 |
| 2 | `addmm_path` | `baseline` | yes | 49,597 | 223,165 | 51549.5 | 14 |

## Re-run

```bash
python3 scripts/benchmark_codex_agent_efficiency.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
