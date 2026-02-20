#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

import argparse
import json
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def run(cmd: list[str], cwd: Path) -> None:
    subprocess.run(
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
    src.mkdir(parents=True, exist_ok=True)
    for i in range(file_count):
        code = f"""
pub fn worker_{i}(value: i32) -> i32 {{
    let needle = value + {i};
    needle
}}

pub fn invoke_{i}() -> i32 {{
    worker_{i}(41)
}}
"""
        (src / f"mod_{i}.rs").write_text(code.strip() + "\n", encoding="utf-8")


def mutate_files(root: Path, changed: int) -> None:
    src = root / "src"
    files = sorted(src.glob("mod_*.rs"))
    for idx, path in enumerate(files[:changed]):
        text = path.read_text(encoding="utf-8")
        text += f"\npub fn changed_{idx}() -> i32 {{ {idx} + 1 }}\n"
        path.write_text(text, encoding="utf-8")


def median_metric(runs: int, worker) -> float:
    values: list[float] = []
    for _ in range(runs):
        values.append(worker())
    return statistics.median(values)


def measure_cold_index(binary: Path, runs: int, file_count: int) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-perf-cold-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            return timed_ms([str(binary), "index", "--embeddings", "off"], work)

    return median_metric(runs, worker)


def measure_keyword_search(binary: Path, runs: int, file_count: int) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-perf-search-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            run([str(binary), "index", "--embeddings", "off"], work)
            return timed_ms(
                [
                    str(binary),
                    "--format",
                    "json2",
                    "--compact",
                    "search",
                    "needle",
                    "--limit",
                    "500",
                ],
                work,
            )

    return median_metric(runs, worker)


def measure_incremental(binary: Path, runs: int, file_count: int, changed: int) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix=f"cgrep-perf-inc-{changed}-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            run([str(binary), "index", "--embeddings", "off"], work)
            mutate_files(work, changed)
            return timed_ms([str(binary), "index", "--embeddings", "off"], work)

    return median_metric(runs, worker)


def measure(binary: Path, runs: int, file_count: int) -> dict[str, float]:
    return {
        "cold_index_ms": round(measure_cold_index(binary, runs, file_count), 2),
        "keyword_search_ms": round(measure_keyword_search(binary, runs, file_count), 2),
        "incremental_1_ms": round(measure_incremental(binary, runs, file_count, 1), 2),
        "incremental_10_ms": round(measure_incremental(binary, runs, file_count, 10), 2),
        "incremental_100_ms": round(measure_incremental(binary, runs, file_count, 100), 2),
    }


def regression_pct(before: float, after: float) -> float:
    if before <= 0:
        return 0.0
    return ((after - before) / before) * 100.0


def main() -> int:
    parser = argparse.ArgumentParser(description="M1 index performance gate")
    parser.add_argument("--baseline-bin", required=True, help="Path to baseline cgrep binary")
    parser.add_argument("--candidate-bin", required=True, help="Path to candidate cgrep binary")
    parser.add_argument("--runs", type=int, default=3, help="Measurement runs per metric")
    parser.add_argument("--files", type=int, default=2000, help="Synthetic source files per run")
    parser.add_argument("--json-out", default="", help="Optional path to write JSON report")
    args = parser.parse_args()

    baseline_bin = Path(args.baseline_bin).resolve()
    candidate_bin = Path(args.candidate_bin).resolve()
    if not baseline_bin.exists():
        print(f"baseline binary not found: {baseline_bin}")
        return 2
    if not candidate_bin.exists():
        print(f"candidate binary not found: {candidate_bin}")
        return 2

    baseline = measure(baseline_bin, args.runs, args.files)
    candidate = measure(candidate_bin, args.runs, args.files)

    regressions = {
        key: round(regression_pct(baseline[key], candidate[key]), 2) for key in baseline.keys()
    }

    payload = {
        "runs": args.runs,
        "files": args.files,
        "baseline": baseline,
        "candidate": candidate,
        "regression_pct": regressions,
        "limits": {
            "keyword_search_ms": 5.0,
            "cold_index_ms": 10.0,
        },
    }

    print(json.dumps(payload, indent=2))

    if args.json_out:
        out_path = Path(args.json_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    failed = []
    if regressions["keyword_search_ms"] > 5.0:
        failed.append("keyword_search_ms")
    if regressions["cold_index_ms"] > 10.0:
        failed.append("cold_index_ms")

    if failed:
        print(f"\nPerf gate failed: {', '.join(failed)}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
