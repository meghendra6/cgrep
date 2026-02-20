#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

import argparse
import json
import os
import signal
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Callable, Dict, Optional


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
    src.mkdir(parents=True, exist_ok=True)
    for i in range(file_count):
        body = "    let local = value + 1;\n" * 30
        marker = "first_query_probe" if i % 13 == 0 else "noise_marker"
        code = f"""
pub fn worker_{i}(value: i32) -> i32 {{
    // {marker}
{body}
    value + {i}
}}

pub fn invoke_{i}() -> i32 {{
    worker_{i}(41)
}}
"""
        (src / f"mod_{i}.rs").write_text(code.strip() + "\n", encoding="utf-8")


def read_status_file(work: Path) -> Optional[Dict]:
    status_path = work / ".cgrep" / "status.json"
    if not status_path.exists():
        return None
    try:
        return json.loads(status_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return None


def wait_for_status(work: Path, timeout_s: float = 20.0) -> Dict:
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < timeout_s:
        status = read_status_file(work)
        if status:
            return status
        time.sleep(0.05)
    raise TimeoutError("timed out waiting for .cgrep/status.json")


def wait_for_full_ready(work: Path, timeout_s: float = 120.0) -> Dict:
    t0 = time.perf_counter()
    while time.perf_counter() - t0 < timeout_s:
        status = wait_for_status(work, timeout_s=5.0)
        if status.get("full_ready") is True:
            return status
        time.sleep(0.1)
    raise TimeoutError("timed out waiting for background full index completion")


def stop_background_worker(work: Path) -> None:
    status = read_status_file(work)
    if not status:
        return
    pid = status.get("pid")
    if not isinstance(pid, int) or pid <= 0:
        return
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def median_metric(runs: int, warmup: int, worker: Callable[[], float]) -> float:
    for _ in range(max(0, warmup)):
        worker()
    values: list[float] = []
    for _ in range(runs):
        values.append(worker())
    return statistics.median(values)


def measure_cold_index(binary: Path, runs: int, warmup: int, file_count: int) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m3-cold-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            return timed_ms([str(binary), "index", "--embeddings", "off"], work)

    return median_metric(runs, warmup, worker)


def measure_first_keyword_no_index(binary: Path, runs: int, warmup: int, file_count: int) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m3-first-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
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

    return median_metric(runs, warmup, worker)


def measure_keyword_after_foreground_index(
    binary: Path, runs: int, warmup: int, file_count: int
) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m3-post-index-") as tmp:
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
                    "first_query_probe",
                    "--limit",
                    "200",
                ],
                work,
            )

    return median_metric(runs, warmup, worker)


def measure_keyword_during_background(
    binary: Path, runs: int, warmup: int, file_count: int
) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m3-bg-search-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            try:
                run([str(binary), "index", "--background", "--embeddings", "off"], work)
                wait_for_status(work)
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
            finally:
                stop_background_worker(work)

    return median_metric(runs, warmup, worker)


def measure_transition_to_full_ready(
    binary: Path, runs: int, warmup: int, file_count: int
) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-m3-transition-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            run([str(binary), "index", "--background", "--embeddings", "off"], work)
            wait_for_full_ready(work)
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

    return median_metric(runs, warmup, worker)


def supports_background_index(binary: Path) -> bool:
    proc = subprocess.run(
        [str(binary), "index", "--help"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    return "--background" in (proc.stdout + proc.stderr)


def measure(
    binary: Path, runs: int, warmup: int, file_count: int, background_supported: bool
) -> dict[str, float]:
    cold_index_ms = measure_cold_index(binary, runs, warmup, file_count)
    first_keyword_ms = measure_first_keyword_no_index(binary, runs, warmup, file_count)
    if background_supported:
        keyword_during_background_ms = measure_keyword_during_background(
            binary, runs, warmup, file_count
        )
        transition_ms = measure_transition_to_full_ready(binary, runs, warmup, file_count)
    else:
        keyword_during_background_ms = first_keyword_ms
        transition_ms = measure_keyword_after_foreground_index(
            binary, runs, warmup, file_count
        )

    return {
        "cold_index_ms": round(cold_index_ms, 2),
        "cold_index_throughput_fps": round(file_count / max(cold_index_ms / 1000.0, 0.001), 2),
        "first_keyword_no_index_ms": round(first_keyword_ms, 2),
        "keyword_during_background_ms": round(keyword_during_background_ms, 2),
        "transition_to_full_ready_ms": round(transition_ms, 2),
    }


def regression_pct(before: float, after: float) -> float:
    if before <= 0:
        return 0.0
    return ((after - before) / before) * 100.0


def throughput_drop_pct(before: float, after: float) -> float:
    if before <= 0:
        return 0.0
    return ((before - after) / before) * 100.0


def main() -> int:
    parser = argparse.ArgumentParser(description="M3 index/search performance gate")
    parser.add_argument("--baseline-bin", required=True, help="Path to baseline cgrep binary")
    parser.add_argument("--candidate-bin", required=True, help="Path to candidate cgrep binary")
    parser.add_argument("--runs", type=int, default=3, help="Measured runs per metric")
    parser.add_argument("--warmup", type=int, default=1, help="Warmup runs before measurements")
    parser.add_argument("--files", type=int, default=1200, help="Synthetic source files per run")
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

    background_supported = supports_background_index(baseline_bin) and supports_background_index(
        candidate_bin
    )

    baseline = measure(
        baseline_bin, args.runs, args.warmup, args.files, background_supported
    )
    candidate = measure(
        candidate_bin, args.runs, args.warmup, args.files, background_supported
    )

    regressions = {
        "first_keyword_no_index_ms": round(
            regression_pct(
                baseline["first_keyword_no_index_ms"], candidate["first_keyword_no_index_ms"]
            ),
            2,
        ),
        "keyword_during_background_ms": round(
            regression_pct(
                baseline["keyword_during_background_ms"],
                candidate["keyword_during_background_ms"],
            ),
            2,
        ),
        "transition_to_full_ready_ms": round(
            regression_pct(
                baseline["transition_to_full_ready_ms"],
                candidate["transition_to_full_ready_ms"],
            ),
            2,
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
        "first_keyword_no_index_ms": 5.0,
        "keyword_during_background_ms": 10.0,
        "cold_index_throughput_drop_pct": 10.0,
    }

    payload = {
        "runs": args.runs,
        "warmup": args.warmup,
        "files": args.files,
        "baseline": baseline,
        "candidate": candidate,
        "regression_pct": regressions,
        "limits": limits,
        "background_metrics_mode": "native" if background_supported else "fallback",
    }
    print(json.dumps(payload, indent=2))

    if args.json_out:
        out_path = Path(args.json_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    failed = []
    if regressions["first_keyword_no_index_ms"] > limits["first_keyword_no_index_ms"]:
        failed.append("first_keyword_no_index_ms")
    if (
        regressions["keyword_during_background_ms"]
        > limits["keyword_during_background_ms"]
    ):
        failed.append("keyword_during_background_ms")
    if (
        regressions["cold_index_throughput_drop_pct"]
        > limits["cold_index_throughput_drop_pct"]
    ):
        failed.append("cold_index_throughput_drop_pct")

    if failed:
        print(f"\nPerf gate failed: {', '.join(failed)}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
