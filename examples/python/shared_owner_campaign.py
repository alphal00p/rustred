#!/usr/bin/env python3
"""Steer one generic Rust shared-owner finite-target campaign (Linux).

Python performs no algebra. Saved owners are reused, not regenerated. Results
are finite-target diagnostics, not universal R10 closure or work checkpoints.
The 15-hour objective is telemetry, NOT a timeout. No license is persisted.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import json
import math
import os
import resource
from pathlib import Path
import signal
import subprocess
import tempfile
import time

INNER_POOLS = ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
               "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")


def positive(text: str) -> int:
    value = int(text)
    if value <= 0:
        raise argparse.ArgumentTypeError("must be positive")
    return value


def nonnegative(text: str) -> int:
    value = int(text)
    if value < 0:
        raise argparse.ArgumentTypeError("must be nonnegative")
    return value


def process_table() -> dict[int, dict]:
    """Kernel PID/start identity, CPU ticks and resident bytes; no child sums."""
    result = {}
    page = os.sysconf("SC_PAGE_SIZE")
    ticks = os.sysconf("SC_CLK_TCK")
    for entry in Path("/proc").iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            stat = (entry / "stat").read_text()
            fields = stat[stat.rfind(")") + 2:].split()
            result[int(entry.name)] = {
                "ppid": int(fields[1]), "pgrp": int(fields[2]),
                "start": int(fields[19]),
                "cpu_seconds": (int(fields[11]) + int(fields[12])) / ticks,
                "rss_bytes": max(0, int(fields[21])) * page,
            }
        except (FileNotFoundError, ProcessLookupError, PermissionError, ValueError, IndexError):
            continue
    return result


def selected_tree(table: dict[int, dict], identities: dict[int, int]) -> dict[int, dict]:
    """Deduplicate overlapping roots; never follow a reused root PID."""
    selected = {pid for pid, start in identities.items()
                if pid in table and table[pid]["start"] == start}
    while True:
        children = {pid for pid, row in table.items() if row["ppid"] in selected}
        enlarged = selected | children
        if enlarged == selected:
            return {pid: table[pid] for pid in sorted(selected)}
        selected = enlarged


def append_record(stream, record: dict) -> None:
    stream.write(json.dumps(record, sort_keys=True) + "\n")
    stream.flush()


def address_space_envelope(hard_bytes: int, other_reserve: int, explicit: int | None,
                           external_rss: int, inherited: tuple[int, int]) -> tuple[int, int]:
    """Admission envelope, not enforcement of independent external jobs."""
    headroom = min(20_000_000_000, hard_bytes // 10)
    available = hard_bytes - other_reserve - headroom
    if other_reserve < external_rss or available <= 0:
        raise ValueError("external reserve is insufficient or leaves no child address space")
    if explicit is not None and not 0 < explicit <= available:
        raise ValueError("child address-space limit exceeds the admitted memory envelope")
    admitted = min([available if explicit is None else explicit]
                   + [limit for limit in inherited if limit != resource.RLIM_INFINITY])
    if admitted <= 0:
        raise ValueError("inherited address-space limit leaves no positive child allowance")
    return admitted, headroom


@contextmanager
def owned_process(command, env, cpus, request_stop, child_address_space=None):
    """Own the child's entire post-spawn lifetime, including receipt failures."""
    def configure_child():
        os.sched_setaffinity(0, cpus)
        if child_address_space is not None:
            resource.setrlimit(resource.RLIMIT_AS, (child_address_space, child_address_space))
    child = subprocess.Popen(command, env=env, start_new_session=True, preexec_fn=configure_child)
    try:
        yield child
    finally:
        # This five-second grace is only supervisor-error cleanup, not a solve
        # timeout. An unreaped direct child cannot have its PID reused.
        if child.poll() is None:
            try:
                request_stop("supervisor_failure")
            except OSError:
                pass
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--targets", type=Path, required=True)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--owner-base", type=Path, default=Path.cwd())
    parser.add_argument("--workers", type=positive, default=min(50, len(os.sched_getaffinity(0))))
    parser.add_argument("--cpus", help="comma-separated permitted CPU IDs; at most --workers")
    parser.add_argument("--registered-pid", type=positive, action="append", default=[])
    parser.add_argument("--other-workers", type=int, default=0,
                        help="all other concurrent compute workers, including builds")
    parser.add_argument("--max-memory-bytes", type=positive, default=500_000_000_000)
    parser.add_argument("--soft-memory-bytes", type=positive, default=450_000_000_000)
    parser.add_argument("--reserved-other-memory-bytes", type=nonnegative,
                        help="required envelope for all declared concurrent external jobs")
    parser.add_argument("--child-address-space-bytes", type=positive,
                        help="optional smaller OS RLIMIT_AS; default hard minus other reserve/headroom")
    parser.add_argument("--objective-hours", type=float, default=15.0,
                        help="performance objective only; never a termination timer")
    parser.add_argument("--sample-seconds", type=float, default=2.0)
    parser.add_argument("--tmp-root", type=Path, default=Path("TMP"))
    parser.add_argument("--max-nodes", type=positive, default=16_000_000)
    parser.add_argument("--max-input-targets", type=positive, default=100_000)
    parser.add_argument("--max-transport-operations", type=positive, default=4_096_000_000)
    parser.add_argument("--max-transport-endpoints", type=positive, default=1_024_000_000)
    parser.add_argument("--max-coalescing-additions", type=positive, default=256_000_000)
    parser.add_argument("--max-rule-applications", type=positive, default=16_000_000)
    parser.add_argument("--no-progress", action="store_true")
    args = parser.parse_args()
    if not 1 <= args.workers <= 50 or args.other_workers < 0 or args.workers + args.other_workers > 50:
        parser.error("aggregate configured compute workers must be between 1 and 50")
    if not 0 < args.soft_memory_bytes < args.max_memory_bytes <= 500_000_000_000:
        parser.error("require 0 < soft < hard <= 500 GB (decimal)")
    if not math.isfinite(args.sample_seconds) or args.sample_seconds < 0.1 or not 0 < args.objective_hours < float("inf"):
        parser.error("positive finite sampling interval/objective required")
    cpus = set(map(int, args.cpus.split(","))) if args.cpus else set(sorted(os.sched_getaffinity(0))[:args.workers])
    if len(cpus) != args.workers or not cpus <= os.sched_getaffinity(0):
        parser.error("CPU set must contain exactly --workers permitted CPU IDs")
    table = process_table()
    registered = {}
    for pid in args.registered_pid:
        if pid not in table:
            parser.error(f"registered PID {pid} is not alive")
        registered[pid] = table[pid]["start"]
    if (args.registered_pid or args.other_workers) and not args.reserved_other_memory_bytes:
        parser.error("concurrent jobs require positive --reserved-other-memory-bytes")
    external_rss = sum(row["rss_bytes"] for pid, row in selected_tree(table, registered).items()
                       if pid != os.getpid())
    try:
        child_as, monitor_headroom = address_space_envelope(args.max_memory_bytes,
            args.reserved_other_memory_bytes or 0, args.child_address_space_bytes,
            external_rss, resource.getrlimit(resource.RLIMIT_AS))
    except ValueError as error:
        parser.error(str(error))
    # Include the supervisor without treating it as an external reservation.
    registered[os.getpid()] = table[os.getpid()]["start"]
    os.sched_setaffinity(0, cpus)
    for path in (args.manifest, args.targets, args.executable):
        if not path.is_file():
            parser.error(f"not a file: {path}")
    args.tmp_root.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="shared-owner-campaign.", dir=args.tmp_root)).resolve()
    stop_file = output / "stop-request.json"
    command = [str(args.executable.resolve()), "routed-campaign", "--manifest", str(args.manifest.resolve()),
               "--targets", str(args.targets.resolve()), "--owner-base", str(args.owner_base.resolve()),
               "--output", str(output / "result.json"), "--events", str(output / "events.jsonl"),
               "--stop-file", str(stop_file), "--workers", str(args.workers)]
    for field in ("max_nodes", "max_input_targets", "max_transport_operations", "max_transport_endpoints",
                  "max_coalescing_additions", "max_rule_applications"):
        command += ["--" + field.replace("_", "-"), str(getattr(args, field))]
    if args.no_progress:
        command.append("--no-progress")
    # Never include the process environment or license in provenance.
    (output / "request.json").write_text(json.dumps({
        "command": command, "cpus": sorted(cpus), "registered_roots": registered,
        "workers": args.workers, "other_workers": args.other_workers,
        "hard_memory_bytes": args.max_memory_bytes, "soft_memory_bytes": args.soft_memory_bytes,
        "child_rlimit_as_bytes": child_as, "monitor_headroom_bytes": monitor_headroom,
        "reserved_other_memory_bytes": args.reserved_other_memory_bytes or 0,
        "supervisor_pid": os.getpid(), "address_space_scope": "owned_single_process",
        "objective_hours": args.objective_hours, "hard_timeout": None,
        "work_checkpoint": False, "family_closure_claim": False,
    }, indent=2) + "\n")
    env = dict(os.environ)
    env.update({name: "1" for name in INNER_POOLS})
    env["SYMBOLICA_HIDE_BANNER"] = "1"
    stop_reason = None

    def request_stop(reason: str) -> None:
        nonlocal stop_reason
        if stop_reason is None:
            stop_reason = reason
            try:
                with stop_file.open("x") as receipt:
                    json.dump({"reason": reason, "unix_time": time.time(), "family_closure_claim": False}, receipt)
            except FileExistsError:
                stop_reason = "existing_operator_stop_file"

    def operator_stop(signum, _frame):
        request_stop(f"operator_signal_{signum}")

    signal.signal(signal.SIGINT, operator_stop)
    signal.signal(signal.SIGTERM, operator_stop)
    started = time.monotonic()
    # preexec_fn is used in this single-threaded driver only, before Rust/native
    # pools exist; the new session makes hard-stop ownership unambiguous.
    with owned_process(command, env, cpus, request_stop, child_as) as child:
        (output / "run.pid").write_text(str(child.pid) + "\n")
        initial = process_table()
        if child.pid in initial:
            registered[child.pid] = initial[child.pid]["start"]
        print(f"Campaign receipts: {output}", flush=True)
        peak = 0
        seen_cpu = {(pid, row["start"]): row["cpu_seconds"]
                    for pid, row in selected_tree(initial, registered).items()}
        cumulative_cpu = 0.0
        last_time = started
        previous_rss = None
        hard_stopped = False
        with (output / "resources.jsonl").open("x") as resources:
            while child.poll() is None:
                if stop_file.exists() and stop_reason is None:
                    request_stop("existing_operator_stop_file")
                table = process_table()
                tree = selected_tree(table, registered)
                # Remember descendants after parent exit, retaining PID/start.
                registered.update({pid: row["start"] for pid, row in tree.items()})
                rss = sum(row["rss_bytes"] for row in tree.values())
                delta_cpu = 0.0
                for pid, row in tree.items():
                    identity = (pid, row["start"])
                    delta_cpu += max(0.0, row["cpu_seconds"] - seen_cpu.get(identity, row["cpu_seconds"]))
                    seen_cpu[identity] = row["cpu_seconds"]
                cumulative_cpu += delta_cpu
                now = time.monotonic()
                peak = max(peak, rss)
                append_record(resources, {"event": "resources", "elapsed_seconds": now-started,
                    "aggregate_rss_bytes": rss, "peak_observed_rss_bytes": peak,
                    "sampled_cpu_seconds_since_start": cumulative_cpu, "observed_busy_cores": delta_cpu/(now-last_time),
                    "rss_growth_bytes_per_second": None if previous_rss is None else (rss-previous_rss)/(now-last_time),
                    "processes": len(tree), "configured_workers": args.workers+args.other_workers,
                    "objective_hours": args.objective_hours, "past_objective": now-started > args.objective_hours*3600,
                    "cancel_reason": stop_reason, "registered_scope_only": True})
                last_time = now
                previous_rss = rss
                if rss >= args.soft_memory_bytes:
                    request_stop("aggregate_rss_soft_limit")
                if rss >= args.max_memory_bytes and not hard_stopped:
                    # Other registered jobs are measured, never signalled.
                    if child.pid in table and table[child.pid]["start"] == registered.get(child.pid):
                        request_stop("aggregate_rss_hard_limit")
                        try:
                            os.killpg(child.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        hard_stopped = True
                time.sleep(args.sample_seconds)
    status = child.wait()
    (output / "run.status").write_text(str(status) + "\n")
    (output / "supervisor-result.json").write_text(json.dumps({
        "exit_status": status, "elapsed_seconds": time.monotonic()-started,
        "peak_observed_aggregate_rss_bytes": peak, "operator_or_resource_stop": stop_reason,
        "hard_stopped": hard_stopped, "work_checkpoint": False, "family_closure_claim": False,
        "child_rlimit_as_bytes": child_as, "memory_failure_is_incomplete": True,
        "rust_result_present": (output / "result.json").is_file(),
    }, indent=2) + "\n")
    return status if status >= 0 else 128-status


if __name__ == "__main__":
    raise SystemExit(main())
