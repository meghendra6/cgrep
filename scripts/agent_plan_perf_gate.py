#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

import argparse
import json
import math
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Callable


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=True,
    )


def command_supports_agent_plan(binary: Path) -> bool:
    probe = subprocess.run(
        [str(binary), "agent", "--help"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    help_text = f"{probe.stdout}\n{probe.stderr}".lower()
    return "plan" in help_text


def timed_ms(cmd: list[str], cwd: Path) -> float:
    start = time.perf_counter()
    run(cmd, cwd)
    return (time.perf_counter() - start) * 1000.0


def collect_samples(runs: int, warmup: int, worker: Callable[[], float]) -> list[float]:
    for _ in range(max(0, warmup)):
        worker()
    out: list[float] = []
    for _ in range(runs):
        out.append(worker())
    return out


def percentile_nearest(values: list[float], percentile: int) -> float:
    ordered = sorted(values)
    if not ordered:
        return 0.0
    if percentile <= 0:
        return ordered[0]
    if percentile >= 100:
        return ordered[-1]
    rank = max(1, math.ceil((percentile / 100.0) * len(ordered)))
    return ordered[rank - 1]


def summarize(samples: list[float]) -> dict[str, float]:
    return {
        "p50": round(statistics.median(samples), 2),
        "p95": round(percentile_nearest(samples, 95), 2),
    }


def write_fixture(root: Path, files: int) -> None:
    src = root / "src"
    docs = root / "docs"
    src.mkdir(parents=True, exist_ok=True)
    docs.mkdir(parents=True, exist_ok=True)

    for i in range(files):
        marker = "plan_latency_probe" if i % 13 == 0 else "noise"
        content = (
            f"pub fn target_fn_{i}(input: i32) -> i32 {{\n"
            f"    // {marker}\n"
            f"    input + {i}\n"
            "}\n"
            f"pub fn call_target_{i}() -> i32 {{ target_fn_{i}(1) }}\n"
        )
        (src / f"mod_{i}.rs").write_text(content, encoding="utf-8")

    (docs / "flow.md").write_text(
        (
            "authentication middleware retry flow orchestration\n"
            "authentication middleware retry flow orchestration\n"
            "authentication middleware retry flow orchestration\n"
        ),
        encoding="utf-8",
    )


def measure_for_binary(binary: Path, runs: int, warmup: int, files: int) -> tuple[dict[str, float], dict[str, dict[str, float]]]:
    with tempfile.TemporaryDirectory(prefix="cgrep-m6-plan-perf-") as tmp:
        repo = Path(tmp)
        write_fixture(repo, files)
        run([str(binary), "index", "--embeddings", "off"], repo)

        simple_cmd = [
            str(binary),
            "--format",
            "json2",
            "--compact",
            "agent",
            "plan",
            "target_fn_42",
            "--max-candidates",
            "5",
            "--max-steps",
            "6",
        ]
        complex_cmd = [
            str(binary),
            "--format",
            "json2",
            "--compact",
            "agent",
            "plan",
            "authentication middleware retry flow",
            "--max-candidates",
            "5",
            "--max-steps",
            "6",
        ]
        e2e_cmd = [
            str(binary),
            "--format",
            "json2",
            "--compact",
            "agent",
            "plan",
            "plan_latency_probe",
            "--max-candidates",
            "8",
            "--max-steps",
            "6",
        ]

        simple = collect_samples(runs, warmup, lambda: timed_ms(simple_cmd, repo))
        complex_ = collect_samples(runs, warmup, lambda: timed_ms(complex_cmd, repo))
        e2e = collect_samples(runs, warmup, lambda: timed_ms(e2e_cmd, repo))

        simple_summary = summarize(simple)
        complex_summary = summarize(complex_)
        e2e_summary = summarize(e2e)
        metrics = {
            "plan_simple_ms": simple_summary["p50"],
            "plan_complex_ms": complex_summary["p50"],
            "plan_expand_e2e_ms": e2e_summary["p50"],
        }
        percentiles = {
            "plan_simple_ms": simple_summary,
            "plan_complex_ms": complex_summary,
            "plan_expand_e2e_ms": e2e_summary,
        }
        return metrics, percentiles


def regression_pct(before: float, after: float) -> float:
    if before <= 0:
        return 0.0
    return ((after - before) / before) * 100.0


def regression_abs_ms(before: float, after: float) -> float:
    return after - before


def main() -> int:
    parser = argparse.ArgumentParser(description="Agent plan performance gate")
    parser.add_argument("--baseline-bin", required=True, help="Path to baseline cgrep binary")
    parser.add_argument("--candidate-bin", required=True, help="Path to candidate cgrep binary")
    parser.add_argument("--runs", type=int, default=5, help="Measured runs per metric")
    parser.add_argument("--warmup", type=int, default=2, help="Warmup runs per metric")
    parser.add_argument("--files", type=int, default=800, help="Fixture source file count")
    parser.add_argument("--json-out", default="", help="Optional JSON report output path")
    args = parser.parse_args()

    if args.runs < 1:
        print("--runs must be >= 1")
        return 2
    if args.warmup < 0:
        print("--warmup must be >= 0")
        return 2
    if args.files < 1:
        print("--files must be >= 1")
        return 2

    baseline_bin = Path(args.baseline_bin).resolve()
    candidate_bin = Path(args.candidate_bin).resolve()
    if not baseline_bin.exists():
        print(f"baseline binary not found: {baseline_bin}")
        return 2
    if not candidate_bin.exists():
        print(f"candidate binary not found: {candidate_bin}")
        return 2

    if not command_supports_agent_plan(candidate_bin):
        print(f"candidate binary does not support `agent plan`: {candidate_bin}")
        return 2

    candidate, candidate_percentiles = measure_for_binary(
        candidate_bin, args.runs, args.warmup, args.files
    )
    baseline_supports_plan = command_supports_agent_plan(baseline_bin)
    if baseline_supports_plan:
        baseline, baseline_percentiles = measure_for_binary(
            baseline_bin, args.runs, args.warmup, args.files
        )
    else:
        # merge-base may predate `agent plan`; keep gate deterministic and non-blocking.
        baseline = dict(candidate)
        baseline_percentiles = {
            metric: dict(values) for metric, values in candidate_percentiles.items()
        }

    regressions = {
        "plan_simple_ms": round(
            regression_pct(baseline["plan_simple_ms"], candidate["plan_simple_ms"]), 2
        ),
        "plan_complex_ms": round(
            regression_pct(baseline["plan_complex_ms"], candidate["plan_complex_ms"]), 2
        ),
        "plan_expand_e2e_ms": round(
            regression_pct(
                baseline["plan_expand_e2e_ms"], candidate["plan_expand_e2e_ms"]
            ),
            2,
        ),
    }
    absolute_deltas = {
        "plan_simple_ms": round(
            regression_abs_ms(baseline["plan_simple_ms"], candidate["plan_simple_ms"]), 2
        ),
        "plan_complex_ms": round(
            regression_abs_ms(baseline["plan_complex_ms"], candidate["plan_complex_ms"]), 2
        ),
        "plan_expand_e2e_ms": round(
            regression_abs_ms(
                baseline["plan_expand_e2e_ms"], candidate["plan_expand_e2e_ms"]
            ),
            2,
        ),
    }
    limits = {
        "plan_simple_ms": 10.0,
        "plan_complex_ms": 10.0,
        "plan_expand_e2e_ms": 10.0,
    }
    absolute_floor_ms = 3.0
    payload = {
        "runs": args.runs,
        "warmup": args.warmup,
        "files": args.files,
        "methodology": {
            "latency_p50": "median",
            "latency_p95": "nearest-rank",
            "measured_runs_per_metric": args.runs,
            "warmup_runs_per_metric": args.warmup,
        },
        "compat": {
            "baseline_supports_agent_plan": baseline_supports_plan,
            "fallback": "self_baseline_candidate" if not baseline_supports_plan else "none",
        },
        "baseline": baseline,
        "candidate": candidate,
        "percentiles": {
            "baseline": baseline_percentiles,
            "candidate": candidate_percentiles,
        },
        "regression_pct": regressions,
        "regression_abs_ms": absolute_deltas,
        "limits": limits,
        "absolute_regression_floor_ms": absolute_floor_ms,
    }
    print(json.dumps(payload, indent=2))

    if args.json_out:
        out = Path(args.json_out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    failed = [
        key
        for key, limit in limits.items()
        if regressions.get(key, 0.0) > limit
        and absolute_deltas.get(key, 0.0) > absolute_floor_ms
    ]
    if failed:
        print(f"\nPerf gate failed: {', '.join(failed)}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
