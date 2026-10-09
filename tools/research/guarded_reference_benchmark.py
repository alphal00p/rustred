#!/usr/bin/env python3
"""Measure existing guarded-reference tests in fresh native test processes.

Build the Rust test executable separately, then run, for example::

    python3 tools/research/guarded_reference_benchmark.py \
        --binary /path/to/rustred-test-executable --output /tmp/reference.json \
        --shells 1 2 --repeats 3 --timeout 300

This runner uses only the Python standard library and requires POSIX wait4.
It does not build code. The probe reports verified and unresolved equations;
a successful test process does not imply that all its equations were proved.
Full harness output is preserved without reclassifying those outcomes.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import signal
import statistics
import subprocess
import sys
import tempfile
import time


CONVERSION_FILTER = "solver::guarded::reference_conversion"
PROBE_TEST = (
    "solver::guarded::reference_validation::"
    "bounded_supplied_reference_probe_reports_outcomes"
)
REFERENCE_ENV = (
    "RUSTRED_REFERENCE_SHELLS", "RUSTRED_REFERENCE_FILTER",
    "RUSTRED_REFERENCE_REQUIRE_CLOSURE",
)
TEST_RESULT = re.compile(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;")


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def signal_group(pid: int, signum: int) -> None:
    try:
        os.killpg(pid, signum)
    except ProcessLookupError:
        pass


def stop_and_reap(pid: int) -> tuple[int, object]:
    """Stop the entire session's process group, including stubborn descendants."""
    signal_group(pid, signal.SIGTERM)
    deadline = time.monotonic() + 0.5
    reaped = None
    while time.monotonic() < deadline:
        waited, status, usage = os.wait4(pid, os.WNOHANG)
        if waited:
            reaped = (status, usage)
            break
        time.sleep(0.02)
    # The direct child can exit while a descendant ignores SIGTERM.
    signal_group(pid, signal.SIGKILL)
    if reaped is not None:
        return reaped
    _, status, usage = os.wait4(pid, 0)
    return status, usage


def run_process(
    command: list[str], env: dict[str, str], timeout: float, cwd: Path
) -> dict:
    """Capture one process and its own wait4 resource usage, never cumulative RSS."""
    result = {
        "command": command,
        "cwd": str(cwd),
        "started_at": utc_now(),
        "timeout_seconds": timeout,
        "timed_out": False,
        "interrupted": False,
        "exit_code": None,
        "peak_rss_bytes": None,
    }
    started = time.monotonic()
    # Files avoid pipe-buffer deadlocks and preserve output from killed children.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        try:
            proc = subprocess.Popen(
                command, cwd=cwd, env=env, stdout=stdout, stderr=stderr,
                start_new_session=True,
            )
        except OSError as error:
            result["launch_error"] = str(error)
            result["process_outcome"] = "launch_error"
        else:
            try:
                while True:
                    waited, status, usage = os.wait4(proc.pid, os.WNOHANG)
                    if waited:
                        break
                    remaining = timeout - (time.monotonic() - started)
                    if remaining <= 0:
                        result["timed_out"] = True
                        status, usage = stop_and_reap(proc.pid)
                        break
                    # Keep collection latency small for millisecond-scale
                    # conversion checks; use a coarser poll for long probes.
                    interval = 0.002 if time.monotonic() - started < 1.0 else 0.01
                    time.sleep(min(interval, remaining))
            except KeyboardInterrupt:
                result["interrupted"] = True
                status, usage = stop_and_reap(proc.pid)
            except BaseException:
                status, _ = stop_and_reap(proc.pid)
                proc.returncode = os.waitstatus_to_exitcode(status)
                raise
            # wait4 already reaped the child: do not call Popen.poll/wait afterwards.
            proc.returncode = os.waitstatus_to_exitcode(status)
            result["exit_code"] = proc.returncode
            result["child_user_cpu_seconds"] = usage.ru_utime
            result["child_system_cpu_seconds"] = usage.ru_stime
            result["peak_rss_raw"] = usage.ru_maxrss
            if sys.platform == "darwin":
                result["peak_rss_raw_unit"] = "bytes"
                result["peak_rss_bytes"] = int(usage.ru_maxrss)
            elif sys.platform.startswith("linux"):
                result["peak_rss_raw_unit"] = "KiB"
                result["peak_rss_bytes"] = int(usage.ru_maxrss) * 1024
            else:
                result["peak_rss_raw_unit"] = "platform-dependent; see getrusage(2)"
            result["process_outcome"] = (
                "interrupted" if result["interrupted"] else
                "timeout" if result["timed_out"] else
                "completed" if proc.returncode == 0 else "nonzero_exit"
            )
        result["wall_seconds"] = time.monotonic() - started
        result["finished_at"] = utc_now()
        stdout.seek(0)
        stderr.seek(0)
        result["stdout"] = stdout.read().decode("utf-8", errors="replace")
        result["stderr"] = stderr.read().decode("utf-8", errors="replace")
    return result


def write_report(path: Path, report: dict) -> None:
    """Replace the report atomically after every sample, preserving partial runs."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w", encoding="utf-8", dir=path.parent,
            prefix=path.name + ".", suffix=".tmp", delete=False,
        ) as stream:
            temporary = Path(stream.name)
            json.dump(report, stream, indent=2, allow_nan=False)
            stream.write("\n")
        temporary.replace(path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def summarize(samples: list[dict]) -> list[dict]:
    summaries = []
    for case in dict.fromkeys(sample["case"] for sample in samples):
        selected = [sample for sample in samples if sample["case"] == case]
        # Keep timeouts/failures visible; do not fold censored times into a median.
        completed = [s for s in selected if s["process_outcome"] == "completed"]
        summary = {
            "case": case,
            "samples": len(selected),
            "completed_processes": len(completed),
            "process_outcomes": [s["process_outcome"] for s in selected],
        }
        if completed:
            walls = [s["wall_seconds"] for s in completed]
            summary["completed_wall_seconds"] = {
                "min": min(walls), "median": statistics.median(walls), "max": max(walls),
            }
            rss = [s["peak_rss_bytes"] for s in completed if s["peak_rss_bytes"] is not None]
            if rss:
                summary["completed_peak_rss_bytes"] = {
                    "min": min(rss), "median": statistics.median(rss), "max": max(rss),
                }
        summaries.append(summary)
    return summaries


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return parsed


def positive_seconds(value: str) -> float:
    parsed = float(value)
    if not math.isfinite(parsed) or parsed <= 0:
        raise argparse.ArgumentTypeError("must be a finite positive number")
    return parsed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--binary", required=True, type=Path, help="already-built native Rust libtest executable")
    parser.add_argument("--output", required=True, type=Path, help="JSON report (replaced atomically)")
    parser.add_argument("--shells", nargs="+", type=int, choices=(1, 2, 3), default=[1, 2])
    parser.add_argument("--repeats", type=positive_int, default=3)
    parser.add_argument("--timeout", type=positive_seconds, default=300.0, help="seconds per process (default: 300)")
    parser.add_argument("--filter", help="substring for RUSTRED_REFERENCE_FILTER in the bounded probe only")
    args = parser.parse_args()
    if not hasattr(os, "wait4") or not hasattr(os, "killpg"):
        parser.error("this runner requires POSIX wait4 and process groups")
    binary = args.binary.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        parser.error(f"binary is not an executable file: {binary}")
    output = args.output.resolve()
    if output == binary:
        parser.error("output must not overwrite the test executable")
    cwd = Path.cwd()
    env = dict(os.environ)
    for name in REFERENCE_ENV:
        env.pop(name, None)
    cases = [("reference_conversion", None, [CONVERSION_FILTER])]
    cases.extend(
        (f"bounded_reference_shells_{shells}", shells, [PROBE_TEST, "--exact", "--ignored"])
        for shells in dict.fromkeys(args.shells)
    )
    digest = hashlib.sha256()
    with binary.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    report = {
        "schema_version": 1,
        "started_at": utc_now(),
        "binary": str(binary),
        "binary_sha256": digest.hexdigest(),
        "platform": {
            "system": platform.system(), "release": platform.release(),
            "machine": platform.machine(), "python": sys.version,
            "logical_cpu_count": os.cpu_count(),
        },
        "configuration": {
            "shells": list(dict.fromkeys(args.shells)), "repeats": args.repeats,
            "timeout_seconds": args.timeout, "reference_filter": args.filter,
            "test_threads": 1, "fresh_process_per_sample": True,
        },
        "environment": {
            "inherits_parent": True,
            "removed_before_each_case": list(REFERENCE_ENV),
            "selected_inherited_values": {
                name: env[name] for name in (
                    "RAYON_NUM_THREADS", "OMP_NUM_THREADS", "RUST_BACKTRACE",
                    "RUST_TEST_THREADS", "MALLOC_CONF",
                ) if name in env
            },
        },
        "interpretation": (
            "Process completion is not a claim of equation validation or family closure. "
            "Read captured harness output for verified/unresolved reductions. Peak RSS "
            "is wait4 ru_maxrss for the test process, not summed concurrent tree memory. "
            "Completed-process summaries exclude timeout, interruption and failed runs."
        ),
        "samples": [],
        "summary": [],
        "complete": False,
    }
    write_report(output, report)

    def interrupted(_signum: int, _frame: object) -> None:
        raise KeyboardInterrupt

    previous_handler = signal.signal(signal.SIGTERM, interrupted)
    exit_code = 0
    try:
        for case, shells, test_args in cases:
            overrides = {}
            if shells is not None:
                overrides["RUSTRED_REFERENCE_SHELLS"] = str(shells)
                if args.filter is not None:
                    overrides["RUSTRED_REFERENCE_FILTER"] = args.filter
            case_env = dict(env, **overrides)
            command = [str(binary), *test_args, "--nocapture", "--test-threads=1"]
            for repeat in range(1, args.repeats + 1):
                print(f"{case}: sample {repeat}/{args.repeats}", file=sys.stderr, flush=True)
                sample = run_process(command, case_env, args.timeout, cwd)
                sample.update(case=case, shells=shells, repeat=repeat, environment_overrides=overrides)
                result = TEST_RESULT.search(sample["stdout"])
                if result:
                    sample["libtest_counts"] = dict(zip(("passed", "failed", "ignored"), map(int, result.groups())))
                if sample["process_outcome"] == "completed" and (
                    result is None or int(result.group(1)) + int(result.group(2)) == 0
                ):
                    sample["process_outcome"] = "missing_test_result"
                report["samples"].append(sample)
                report["summary"] = summarize(report["samples"])
                write_report(output, report)
                if sample["interrupted"]:
                    raise KeyboardInterrupt
                if sample["process_outcome"] != "completed":
                    exit_code = 1
        report["complete"] = True
    except KeyboardInterrupt:
        report["interrupted"] = True
        exit_code = 130
    finally:
        signal.signal(signal.SIGTERM, previous_handler)
        report["finished_at"] = utc_now()
        write_report(output, report)
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
