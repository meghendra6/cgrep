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


def run(cmd: list[str], cwd: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=True,
    )


def timed_ms(cmd: list[str], cwd: Path) -> float:
    t0 = time.perf_counter()
    run(cmd, cwd)
    return (time.perf_counter() - t0) * 1000.0


def generate_repo(root: Path, file_count: int) -> None:
    src = root / "src"
    scoped = src / "scoped"
    other = src / "other"
    scoped.mkdir(parents=True, exist_ok=True)
    other.mkdir(parents=True, exist_ok=True)

    for i in range(file_count):
        body = "    let local = value + 1;\n" * 24
        marker = "first_query_probe" if i % 13 == 0 else "noise_marker"
        ident = f"worker_{i}"
        scoped_marker = "scoped_probe" if i % 7 == 0 else "scoped_noise"
        target_dir = scoped if i % 2 == 0 else other
        code = f"""
pub fn {ident}(value: i32) -> i32 {{
    // {marker}
    // {scoped_marker}
{body}
    value + {i}
}}

pub fn invoke_{i}() -> i32 {{
    {ident}(41)
}}
"""
        (target_dir / f"mod_{i}.rs").write_text(code.strip() + "\n", encoding="utf-8")


def write_ranking_config(root: Path, enabled: bool) -> None:
    if not enabled:
        cfg = "[ranking]\nenabled = false\n"
    else:
        cfg = """
[ranking]
enabled = true
path_weight = 1.2
symbol_weight = 1.8
language_weight = 1.0
changed_weight = 1.2
kind_weight = 2.0
weak_signal_penalty = 1.4
""".strip() + "\n"
    (root / ".cgreprc.toml").write_text(cfg, encoding="utf-8")


def collect_metric_samples(runs: int, warmup: int, worker: Callable[[], float]) -> list[float]:
    for _ in range(max(0, warmup)):
        worker()
    values: list[float] = []
    for _ in range(runs):
        values.append(worker())
    return values


def percentile_nearest_rank(values: list[float], percentile: int) -> float:
    ordered = sorted(values)
    if not ordered:
        return 0.0
    if percentile <= 0:
        return ordered[0]
    if percentile >= 100:
        return ordered[-1]
    rank = max(1, math.ceil((percentile / 100.0) * len(ordered)))
    return ordered[rank - 1]


def summarize_latency_ms(samples: list[float]) -> dict[str, float]:
    return {
        "p50": round(statistics.median(samples), 2),
        "p95": round(percentile_nearest_rank(samples, 95), 2),
    }


def measure_cold_index(binary: Path, runs: int, warmup: int, file_count: int) -> list[float]:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m4-cold-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            return timed_ms([str(binary), "index", "--embeddings", "off"], work)

    return collect_metric_samples(runs, warmup, worker)


def measure_keyword_legacy(binary: Path, runs: int, warmup: int, file_count: int) -> list[float]:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m4-legacy-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            write_ranking_config(work, enabled=False)
            run([str(binary), "index", "--embeddings", "off"], work)
            return timed_ms(
                [
                    str(binary),
                    "--format",
                    "json2",
                    "--compact",
                    "search",
                    "first_query_probe",
                    "--limit",
                    "200",
                ],
                work,
            )

    return collect_metric_samples(runs, warmup, worker)


def measure_keyword_ranking(binary: Path, runs: int, warmup: int, file_count: int) -> list[float]:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m4-ranking-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            write_ranking_config(work, enabled=True)
            run([str(binary), "index", "--embeddings", "off"], work)
            return timed_ms(
                [
                    str(binary),
                    "--format",
                    "json2",
                    "--compact",
                    "search",
                    "first_query_probe",
                    "--limit",
                    "200",
                ],
                work,
            )

    return collect_metric_samples(runs, warmup, worker)


def measure_identifier_ranking(binary: Path, runs: int, warmup: int, file_count: int) -> list[float]:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m4-ident-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            write_ranking_config(work, enabled=True)
            run([str(binary), "index", "--embeddings", "off"], work)
            return timed_ms(
                [
                    str(binary),
                    "--format",
                    "json2",
                    "--compact",
                    "search",
                    "worker_77",
                    "--limit",
                    "200",
                ],
                work,
            )

    return collect_metric_samples(runs, warmup, worker)


def measure_scoped_ranking(binary: Path, runs: int, warmup: int, file_count: int) -> list[float]:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m4-scope-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            write_ranking_config(work, enabled=True)
            run([str(binary), "index", "--embeddings", "off"], work)
            return timed_ms(
                [
                    str(binary),
                    "--format",
                    "json2",
                    "--compact",
                    "search",
                    "scoped_probe",
                    "-p",
                    "src/scoped",
                    "--limit",
                    "200",
                ],
                work,
            )

    return collect_metric_samples(runs, warmup, worker)


def measure(
    binary: Path, runs: int, warmup: int, file_count: int
) -> tuple[dict[str, float], dict[str, dict[str, float]]]:
    cold_samples = measure_cold_index(binary, runs, warmup, file_count)
    legacy_samples = measure_keyword_legacy(binary, runs, warmup, file_count)
    ranking_samples = measure_keyword_ranking(binary, runs, warmup, file_count)
    identifier_samples = measure_identifier_ranking(binary, runs, warmup, file_count)
    scoped_samples = measure_scoped_ranking(binary, runs, warmup, file_count)

    cold_summary = summarize_latency_ms(cold_samples)
    legacy_summary = summarize_latency_ms(legacy_samples)
    ranking_summary = summarize_latency_ms(ranking_samples)
    identifier_summary = summarize_latency_ms(identifier_samples)
    scoped_summary = summarize_latency_ms(scoped_samples)

    metrics = {
        "cold_index_ms": cold_summary["p50"],
        "cold_index_throughput_fps": round(file_count / max(cold_summary["p50"] / 1000.0, 0.001), 2),
        "keyword_legacy_ms": legacy_summary["p50"],
        "keyword_ranking_ms": ranking_summary["p50"],
        "identifier_ranking_ms": identifier_summary["p50"],
        "scoped_ranking_ms": scoped_summary["p50"],
    }
    percentiles = {
        "cold_index_ms": cold_summary,
        "keyword_legacy_ms": legacy_summary,
        "keyword_ranking_ms": ranking_summary,
        "identifier_ranking_ms": identifier_summary,
        "scoped_ranking_ms": scoped_summary,
    }
    return metrics, percentiles


def regression_pct(before: float, after: float) -> float:
    if before <= 0:
        return 0.0
    return ((after - before) / before) * 100.0


def throughput_drop_pct(before: float, after: float) -> float:
    if before <= 0:
        return 0.0
    return ((before - after) / before) * 100.0


def main() -> int:
    parser = argparse.ArgumentParser(description="M4 keyword ranking performance gate")
    parser.add_argument("--baseline-bin", required=True, help="Path to baseline cgrep binary")
    parser.add_argument("--candidate-bin", required=True, help="Path to candidate cgrep binary")
    parser.add_argument("--runs", type=int, default=3, help="Measured runs per metric")
    parser.add_argument("--warmup", type=int, default=1, help="Warmup runs before measurements")
    parser.add_argument("--files", type=int, default=1200, help="Synthetic source files per run")
    parser.add_argument("--json-out", default="", help="Optional path to write JSON report")
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

    baseline, baseline_percentiles = measure(baseline_bin, args.runs, args.warmup, args.files)
    candidate, candidate_percentiles = measure(candidate_bin, args.runs, args.warmup, args.files)

    regressions = {
        "keyword_legacy_ms": round(
            regression_pct(baseline["keyword_legacy_ms"], candidate["keyword_legacy_ms"]), 2
        ),
        "keyword_ranking_ms": round(
            regression_pct(baseline["keyword_ranking_ms"], candidate["keyword_ranking_ms"]), 2
        ),
        "identifier_ranking_ms": round(
            regression_pct(
                baseline["identifier_ranking_ms"], candidate["identifier_ranking_ms"]
            ),
            2,
        ),
        "scoped_ranking_ms": round(
            regression_pct(baseline["scoped_ranking_ms"], candidate["scoped_ranking_ms"]), 2
        ),
        "cold_index_throughput_drop_pct": round(
            throughput_drop_pct(
                baseline["cold_index_throughput_fps"],
                candidate["cold_index_throughput_fps"],
            ),
            2,
        ),
    }

    limits = {
        "keyword_legacy_ms": 5.0,
        "keyword_ranking_ms": 10.0,
        "identifier_ranking_ms": 10.0,
        "scoped_ranking_ms": 10.0,
        "cold_index_throughput_drop_pct": 10.0,
    }

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
        "baseline": baseline,
        "candidate": candidate,
        "percentiles": {
            "baseline": baseline_percentiles,
            "candidate": candidate_percentiles,
        },
        "regression_pct": regressions,
        "limits": limits,
    }
    print(json.dumps(payload, indent=2))

    if args.json_out:
        out_path = Path(args.json_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    failed = []
    for key, limit in limits.items():
        if regressions.get(key, 0.0) > limit:
            failed.append(key)

    if failed:
        print(f"\nPerf gate failed: {', '.join(failed)}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
