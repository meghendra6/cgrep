#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

import argparse
import json
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

SEARCH_REGRESSION_LIMIT_PCT = 5.0
INDEX_REGRESSION_LIMIT_PCT = 10.0

LIMITS = {
    "cold_index_ms": INDEX_REGRESSION_LIMIT_PCT,
    "incremental_1_ms": INDEX_REGRESSION_LIMIT_PCT,
    "incremental_10_ms": INDEX_REGRESSION_LIMIT_PCT,
    "incremental_100_ms": INDEX_REGRESSION_LIMIT_PCT,
    "branch_switch_ms": INDEX_REGRESSION_LIMIT_PCT,
    "keyword_search_ms": SEARCH_REGRESSION_LIMIT_PCT,
    "identifier_search_ms": SEARCH_REGRESSION_LIMIT_PCT,
    "scoped_search_ms": SEARCH_REGRESSION_LIMIT_PCT,
}


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


def timed_avg_ms(cmd: list[str], cwd: Path, repeats: int) -> float:
    t0 = time.perf_counter()
    for _ in range(max(1, repeats)):
        run(cmd, cwd)
    elapsed_ms = (time.perf_counter() - t0) * 1000.0
    return elapsed_ms / max(1, repeats)


def generate_repo(root: Path, file_count: int) -> None:
    src = root / "src"
    core = src / "core"
    scoped = src / "scoped"
    core.mkdir(parents=True, exist_ok=True)
    scoped.mkdir(parents=True, exist_ok=True)

    for i in range(file_count):
        dst = scoped if i % 5 == 0 else core
        identifier = f"WorkerSymbol{i:04d}"
        keyword = "needle_keyword"
        scoped_marker = "scoped_marker" if dst is scoped else "non_scoped_marker"
        code = f"""
pub struct {identifier};

pub fn keyword_{i}(value: i32) -> i32 {{
    let marker = value + {i};
    // {keyword}
    marker
}}

pub fn ident_{i}(input: i32) -> i32 {{
    let typed: {identifier} = {identifier};
    let _ = typed;
    // {scoped_marker}
    keyword_{i}(input)
}}
"""
        (dst / f"mod_{i:04d}.rs").write_text(code.strip() + "\n", encoding="utf-8")


def all_source_files(root: Path) -> list[Path]:
    return sorted((root / "src").rglob("mod_*.rs"))


def mutate_files(root: Path, changed: int, marker: str) -> None:
    files = all_source_files(root)
    for idx, path in enumerate(files[:changed]):
        text = path.read_text(encoding="utf-8")
        text += f"\npub fn {marker}_{idx}() -> usize {{ {idx} + 1 }}\n"
        path.write_text(text, encoding="utf-8")


def switch_to_branch_b(root: Path) -> tuple[dict[Path, str], list[Path]]:
    files = all_source_files(root)
    target = files[: min(120, len(files))]
    originals = {path: path.read_text(encoding="utf-8") for path in target}

    for idx, path in enumerate(target[:80]):
        text = path.read_text(encoding="utf-8")
        text += f"\npub fn branch_b_update_{idx}() -> usize {{ {idx} + 11 }}\n"
        path.write_text(text, encoding="utf-8")

    for path in target[80:100]:
        if path.exists():
            path.unlink()

    created: list[Path] = []
    for idx in range(20):
        path = root / "src" / "core" / f"branch_new_{idx:04d}.rs"
        code = f"""
pub fn branch_new_{idx}() -> usize {{
    let marker = {idx} + 1;
    marker
}}
"""
        path.write_text(code.strip() + "\n", encoding="utf-8")
        created.append(path)

    return originals, created


def restore_branch_a(originals: dict[Path, str], created: list[Path]) -> None:
    for path in created:
        if path.exists():
            path.unlink()
    for path, content in originals.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def median_metric(runs: int, warmup: int, worker) -> float:
    for _ in range(max(0, warmup)):
        worker()

    values: list[float] = []
    for _ in range(runs):
        values.append(worker())
    return statistics.median(values)


def measure_cold_index(binary: Path, runs: int, warmup: int, file_count: int) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-perf-cold-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            return timed_ms([str(binary), "index", "--embeddings", "off"], work)

    return median_metric(runs, warmup, worker)


def measure_incremental(
    binary: Path,
    runs: int,
    warmup: int,
    file_count: int,
    changed: int,
) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix=f"cgrep-perf-inc-{changed}-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            run([str(binary), "index", "--embeddings", "off"], work)
            mutate_files(work, changed, marker=f"inc_{changed}")
            return timed_ms([str(binary), "index", "--embeddings", "off"], work)

    return median_metric(runs, warmup, worker)


def measure_branch_switch(binary: Path, runs: int, warmup: int, file_count: int) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-perf-branch-switch-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            run([str(binary), "index", "--embeddings", "off"], work)

            originals, created = switch_to_branch_b(work)
            to_branch_b = timed_ms([str(binary), "index", "--embeddings", "off"], work)

            restore_branch_a(originals, created)
            to_branch_a = timed_ms([str(binary), "index", "--embeddings", "off"], work)
            return statistics.median([to_branch_b, to_branch_a])

    return median_metric(runs, warmup, worker)


def measure_search(
    binary: Path,
    runs: int,
    warmup: int,
    file_count: int,
    query: str,
    scope: str | None = None,
    repeats: int = 3,
) -> float:
    def worker() -> float:
        with tempfile.TemporaryDirectory(prefix="cgrep-perf-search-") as tmp:
            work = Path(tmp)
            generate_repo(work, file_count)
            run([str(binary), "index", "--embeddings", "off"], work)

            cmd = [
                str(binary),
                "--format",
                "json2",
                "--compact",
                "search",
                query,
                "--limit",
                "500",
            ]
            if scope:
                cmd.extend(["-p", scope])
            return timed_avg_ms(cmd, work, repeats)

    return median_metric(runs, warmup, worker)


def measure(binary: Path, runs: int, warmup: int, file_count: int) -> dict[str, float]:
    return {
        "cold_index_ms": round(measure_cold_index(binary, runs, warmup, file_count), 2),
        "incremental_1_ms": round(
            measure_incremental(binary, runs, warmup, file_count, 1), 2
        ),
        "incremental_10_ms": round(
            measure_incremental(binary, runs, warmup, file_count, 10), 2
        ),
        "incremental_100_ms": round(
            measure_incremental(binary, runs, warmup, file_count, 100), 2
        ),
        "branch_switch_ms": round(
            measure_branch_switch(binary, runs, warmup, file_count), 2
        ),
        "keyword_search_ms": round(
            measure_search(binary, runs, warmup, file_count, "needle_keyword"), 2
        ),
        "identifier_search_ms": round(
            measure_search(binary, runs, warmup, file_count, "WorkerSymbol0500"), 2
        ),
        "scoped_search_ms": round(
            measure_search(
                binary,
                runs,
                warmup,
                file_count,
                "scoped_marker",
                scope="src/scoped",
            ),
            2,
        ),
    }


def regression_pct(before: float, after: float) -> float:
    if before <= 0:
        return 0.0
    return ((after - before) / before) * 100.0


def main() -> int:
    parser = argparse.ArgumentParser(description="M2 index/search performance comparator")
    parser.add_argument("--baseline-bin", required=True, help="Path to baseline cgrep binary")
    parser.add_argument("--candidate-bin", required=True, help="Path to candidate cgrep binary")
    parser.add_argument("--runs", type=int, default=5, help="Measured repetitions")
    parser.add_argument(
        "--warmup", type=int, default=1, help="Warmup repetitions before measured runs"
    )
    parser.add_argument(
        "--files", type=int, default=2000, help="Synthetic source files per benchmark run"
    )
    parser.add_argument("--json-out", default="", help="Optional JSON report output path")
    args = parser.parse_args()

    baseline_bin = Path(args.baseline_bin).resolve()
    candidate_bin = Path(args.candidate_bin).resolve()
    if not baseline_bin.exists():
        print(f"baseline binary not found: {baseline_bin}")
        return 2
    if not candidate_bin.exists():
        print(f"candidate binary not found: {candidate_bin}")
        return 2

    baseline = measure(baseline_bin, args.runs, args.warmup, args.files)
    candidate = measure(candidate_bin, args.runs, args.warmup, args.files)
    regressions = {
        key: round(regression_pct(baseline[key], candidate[key]), 2) for key in baseline.keys()
    }

    failed = [
        key
        for key, regression in regressions.items()
        if regression > LIMITS.get(key, INDEX_REGRESSION_LIMIT_PCT)
    ]

    payload = {
        "measurement": {
            "runs": args.runs,
            "warmup": args.warmup,
            "files": args.files,
            "rule": "median(measured_runs) after warmup",
        },
        "noise_handling": {
            "warmup_enabled": args.warmup > 0,
            "median_rule": "use median across measured repetitions",
            "reproducible_fixture": True,
        },
        "baseline": baseline,
        "candidate": candidate,
        "regression_pct": regressions,
        "limits_pct": LIMITS,
        "failed_metrics": failed,
    }

    print(json.dumps(payload, indent=2))

    if args.json_out:
        out_path = Path(args.json_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    if failed:
        print(f"\nPerf comparator failed: {', '.join(failed)}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
