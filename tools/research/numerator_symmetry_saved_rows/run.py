#!/usr/bin/env python3
"""Resource-supervise an explicitly selected, immutable symmetry-row study.

This is a research driver, not a production campaign launcher/resume command.
The native executable creates the new output directory itself. Logs and the
measurement receipt are siblings, so they survive interruption of that process.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import time


def cpu_set(text: str) -> set[int]:
    result: set[int] = set()
    for item in text.split(","):
        if "-" in item:
            first, last = map(int, item.split("-", 1))
            if first < 0 or last < first:
                raise argparse.ArgumentTypeError("CPU ranges must be nonnegative and increasing")
            result.update(range(first, last + 1))
        else:
            cpu = int(item)
            if cpu < 0:
                raise argparse.ArgumentTypeError("CPU IDs must be nonnegative")
            result.add(cpu)
    if not result:
        raise argparse.ArgumentTypeError("at least one CPU is required")
    return result


def command(args: argparse.Namespace) -> list[str]:
    result = [str(args.executable), str(args.baseline), str(args.plan),
              str(args.publication), str(args.selection), str(args.output),
              "--workers", str(len(args.cpus))]
    if args.max_numerator_degree is not None:
        result += ["--max-numerator-degree", str(args.max_numerator_degree)]
    if args.source_key is not None:
        result += ["--source-key", str(args.source_key)]
    if args.fewest_active_lines_first:
        result += ["--fewest-active-lines-first"]
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("executable", "baseline", "plan", "publication", "selection", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--cpus", type=cpu_set, required=True)
    parser.add_argument("--max-memory-bytes", type=int, required=True)
    parser.add_argument("--max-numerator-degree", type=int)
    parser.add_argument("--source-key", type=Path,
                        help="optional JSON integer index tuple, never a topology-name dispatch")
    parser.add_argument("--fewest-active-lines-first", action="store_true")
    args = parser.parse_args()
    if args.max_memory_bytes <= 0:
        parser.error("the caller must supply a positive memory allowance")
    if args.max_numerator_degree is not None and args.max_numerator_degree < 0:
        parser.error("numerator degree must be nonnegative")
    if args.output.exists():
        parser.error("the output directory must be new; this experimental driver has no resume command")
    prefix = args.output.with_name(args.output.name + "-run")
    stdout = prefix.with_name(prefix.name + ".stdout")
    stderr = prefix.with_name(prefix.name + ".stderr")
    receipt = prefix.with_name(prefix.name + ".receipt.json")
    argv = command(args)
    environment = dict(os.environ)
    environment.update(RAYON_NUM_THREADS=str(len(args.cpus)),
                       OMP_NUM_THREADS=str(len(args.cpus)), OPENBLAS_NUM_THREADS="1")
    os.sched_setaffinity(0, args.cpus)
    started = time.monotonic()
    peak_rss = 0
    reason = "completed"
    last_event = started - 60
    with stdout.open("x") as out, stderr.open("x") as err:
        child = subprocess.Popen(argv, stdout=out, stderr=err, env=environment,
                                 start_new_session=True)
        print(json.dumps({"pid": child.pid, "command": argv, "stdout": str(stdout),
                          "stderr": str(stderr), "cpus": sorted(args.cpus)}, indent=2), flush=True)
        try:
            while child.poll() is None:
                rss = 0
                try:
                    for line in Path(f"/proc/{child.pid}/status").read_text().splitlines():
                        if line.startswith(("VmRSS:", "VmHWM:")):
                            rss = max(rss, int(line.split()[1]) * 1024)
                except FileNotFoundError:
                    pass
                peak_rss = max(peak_rss, rss)
                if rss > args.max_memory_bytes:
                    reason = "censored_explicit_caller_memory_allowance"
                    try:
                        os.killpg(child.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass  # The owned child may have completed this instant.
                    child.wait()
                    break
                now = time.monotonic()
                if now - last_event >= 60:
                    print(json.dumps({"elapsed_seconds": now-started, "rss_bytes": rss,
                                      "peak_rss_bytes": peak_rss}), flush=True)
                    last_event = now
                time.sleep(0.5)
        except KeyboardInterrupt:
            reason = "censored_manual_interrupt"
            try:
                os.killpg(child.pid, signal.SIGINT)
            except ProcessLookupError:
                pass
            child.wait()
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    peak_rss = max(peak_rss, int(usage.ru_maxrss) * 1024)
    if reason == "completed" and child.returncode != 0:
        reason = "native_error_or_external_termination"
    result = {"command": argv, "pid": child.pid, "exit_code": child.returncode,
              "reason": reason, "elapsed_seconds": time.monotonic()-started,
              "peak_rss_bytes": peak_rss, "cpu_user_seconds": usage.ru_utime,
              "cpu_system_seconds": usage.ru_stime, "cpus": sorted(args.cpus),
              "max_memory_bytes": args.max_memory_bytes,
              "profile": "optimized external driver with release app/core dependencies",
              "compilation_included": False, "production_campaign_mutated": False}
    receipt.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2), flush=True)
    return child.returncode if child.returncode >= 0 else 128-child.returncode


if __name__ == "__main__":
    raise SystemExit(main())
