# PyTorch Search Option Performance Benchmark

Generated: 2026-02-22T05:52:24.172538+00:00

> Snapshot note: values depend on repository state, cache state, and hardware.
> Re-run with the same command before release/perf decisions.

## Scope

- Measures `cgrep search` latency and payload size for practical option/scenario pairs.
- Includes indexed keyword paths and scan-mode (`--no-index`, `--regex`) paths.
- Success means expected marker evidence appears in structured JSON results.

## Environment

- OS: `macOS-26.3-arm64-arm-64bit`
- Python: `3.12.4`
- cgrep commit: `be95ef6`
- pytorch commit: `66e77ae932c`
- PyTorch files (`git ls-files`): `21634`
- runs per case: `5` (warmup `1`)
- limit per search: `20`
- index build time: `143.39 ms`

## Case Results

| Case | Scenario | Options | Success | P50 latency (ms) | P95 latency (ms) | P50 payload tokens | P50 results |
|---|---|---|---:|---:|---:|---:|---:|
| `default_autograd` | autograd evaluate_function | `(default)` | 100.0% | 17.98 | 18.18 | 838 | 16 |
| `path_scoped_autograd` | autograd evaluate_function | `(default)` | 100.0% | 14.75 | 14.82 | 323 | 5 |
| `type_cpp_parser` | python arg parser | `--type cpp` | 100.0% | 16.58 | 17.02 | 998 | 20 |
| `glob_cpp_cuda` | cuda graph | `--glob *.cpp` | 100.0% | 15.78 | 15.94 | 335 | 5 |
| `context_addmm` | addmm call path | `-C 2` | 100.0% | 15.89 | 16.59 | 210 | 2 |
| `budget_tight_dispatch` | dispatch key set | `-B tight` | 100.0% | 14.77 | 15.24 | 1000 | 20 |
| `budget_full_dispatch` | dispatch key set | `-B full` | 100.0% | 14.68 | 14.92 | 1025 | 20 |
| `profile_fast_dispatch` | dispatch key set | `-P fast` | 100.0% | 15.04 | 15.33 | 1006 | 20 |
| `payload_compact_dispatch` | dispatch key set | `--path-alias --dedupe-context --suppress-boilerplate` | 100.0% | 14.58 | 15.14 | 983 | 20 |
| `fuzzy_tensor_iterator` | tensor iterator symbol lookup | `--fuzzy` | 100.0% | 20.17 | 20.64 | 1002 | 20 |
| `scan_no_index_autograd` | autograd evaluate_function | `--no-index` | 100.0% | 17.41 | 19.50 | 320 | 5 |
| `scan_regex_addmm` | addmm regex search | `--regex --no-index` | 100.0% | 45.05 | 46.22 | 1163 | 20 |

## Highlights

- Root search -> scoped search latency change: **-18.0%** (`default_autograd` vs `path_scoped_autograd`).
- Indexed scoped -> `--no-index` scan latency change: **18.0%**.
- Scan -> regex scan latency change: **158.8%**.
- `-B tight` -> `-B full` payload token change: **2.5%**.
- `-B tight` -> `-P fast` latency change: **1.8%**.

## Re-run

```bash
python3 scripts/benchmark_search_option_performance.py --repo /path/to/pytorch --cgrep-bin /path/to/cgrep
```
