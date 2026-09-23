#!/usr/bin/env python3
"""Steer one generic Rust shared-owner campaign (Linux).

Python performs no algebra. Saved owners are reused, not regenerated. Results
are finite-target traces (--targets) or symbolic-domain walks (--queries),
not universal R10 closure claims or work checkpoints.
The 15-hour objective is telemetry, NOT a timeout. No license is persisted.
"""
from __future__ import annotations

import argparse
import importlib.util
from collections import deque
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

# Reuse the thin domain driver's option whitelist without depending on the
# caller's working directory or Python module search path.
_DOMAIN_SPEC = importlib.util.spec_from_file_location(
    "owner_domain_steering", Path(__file__).with_name("match_shared_owner_domains.py"))
DOMAIN = importlib.util.module_from_spec(_DOMAIN_SPEC)
_DOMAIN_SPEC.loader.exec_module(DOMAIN)
SYMBOLIC_ALLOWANCES = (*DOMAIN.ALLOWANCES, DOMAIN.REFINEMENT,
                      *(name for name in DOMAIN.WALK_ALLOWANCES if name != "workers"),
                      "max-route-masks-per-query")
SYMBOLIC_POLICIES = (DOMAIN.REFINEMENT_AXES, DOMAIN.TRANSFER_LOOKAHEAD)
FINITE_ALLOWANCES = {
    "max-nodes": 16_000_000,
    "max-input-targets": 100_000,
    "max-transport-operations": 4_096_000_000,
    "max-transport-endpoints": 1_024_000_000,
    "max-coalescing-additions": 256_000_000,
    "max-rule-applications": 16_000_000,
}


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


class ProcessTreeCollector:
    """Sample explicit roots and observed descendants, never the whole host.

    Known descendants survive reparenting; never-observed children that reparent
    between samples may be missed. This is sampled accounting, not a cgroup.
    """
    def __init__(self, proc_root=Path("/proc")):
        self.proc_root = proc_root
        self.identities = {}
        self.page = os.sysconf("SC_PAGE_SIZE")
        self.ticks = os.sysconf("SC_CLK_TCK")

    def _stat(self, pid):
        stat = (self.proc_root / str(pid) / "stat").read_text()
        fields = stat[stat.rfind(")") + 2:].split()
        return {"ppid": int(fields[1]), "pgrp": int(fields[2]), "start": int(fields[19]),
                "cpu_seconds": (int(fields[11]) + int(fields[12])) / self.ticks,
                "rss_bytes": max(0, int(fields[21])) * self.page}

    def _leader(self, pid):
        status = (self.proc_root / str(pid) / "status").read_text()
        return next(int(line.split()[1]) for line in status.splitlines()
                    if line.startswith("Tgid:")) == pid

    def register(self, pid):
        first = self._stat(pid)
        if not self._leader(pid) or self._stat(pid)["start"] != first["start"]:
            raise ValueError(f"PID {pid} is not a stable process leader")
        self.identities[pid] = first["start"]

    def sample(self):
        diagnostics = {"stat_reads": 0, "task_directories": 0, "children_files": 0,
                       "read_races": 0, "reused_pids": 0}
        result = {}
        pending = deque((pid, start, None) for pid, start in self.identities.items())
        considered = set()
        read_errors = (OSError, ValueError, IndexError, StopIteration)

        def read_stat(pid):
            diagnostics["stat_reads"] += 1
            return self._stat(pid)

        while pending:
            pid, expected_start, parent = pending.popleft()
            if pid in considered:
                continue
            considered.add(pid)
            try:
                row = read_stat(pid)
                if expected_start is not None and row["start"] != expected_start:
                    self.identities.pop(pid, None)
                    diagnostics["reused_pids"] += 1
                    continue
                if parent is not None:
                    parent_pid, parent_start = parent
                    if (row["ppid"] != parent_pid or not self._leader(pid)
                            or read_stat(parent_pid)["start"] != parent_start):
                        continue
                    confirmed = read_stat(pid)
                    if (confirmed["start"] != row["start"]
                            or confirmed["ppid"] != parent_pid):
                        continue
                    row = confirmed
                self.identities[pid] = row["start"]
                result[pid] = row
                diagnostics["task_directories"] += 1
                for task in (self.proc_root / str(pid) / "task").iterdir():
                    if not task.name.isdecimal():
                        continue
                    try:
                        diagnostics["children_files"] += 1
                        children = (task / "children").read_text().split()
                        for child in children:
                            pending.append((int(child), None, (pid, row["start"])))
                    except read_errors:
                        diagnostics["read_races"] += 1
            except read_errors:
                diagnostics["read_races"] += 1
                # Preserve unreadable live identities for a later sample.
                # Only confirmed disappearance or a changed start time prunes.
                try:
                    (self.proc_root / str(pid)).stat()
                except (FileNotFoundError, ProcessLookupError):
                    self.identities.pop(pid, None)
                except OSError:
                    pass
        diagnostics["known_identities"] = len(self.identities)
        return result, diagnostics


def process_cpu_sample(table, previous, elapsed, supervisor_pid, owned_pid):
    """Per-process deltas expose supervisor cost without another proc read."""
    rows = []
    current = {}
    delta = 0.0
    for pid, row in sorted(table.items()):
        identity = (pid, row["start"])
        cpu = row["cpu_seconds"]
        sampled_delta = max(0.0, cpu - previous[identity]) if identity in previous else None
        current[identity] = cpu
        delta += sampled_delta or 0.0
        rows.append({"pid": pid, "start": row["start"], "ppid": row["ppid"],
                     "role": "supervisor" if pid == supervisor_pid else
                             "owned_native" if pid == owned_pid else "registered_descendant",
                     "rss_bytes": row["rss_bytes"], "cpu_seconds": cpu,
                     "sampled_cpu_delta_seconds": sampled_delta,
                     "observed_busy_cores": None if sampled_delta is None else sampled_delta / elapsed})
    return delta, current, rows


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
    scope = parser.add_mutually_exclusive_group(required=True)
    scope.add_argument("--targets", type=Path, help="concrete integer targets (CSV)")
    parser.add_argument("--entry-domains", type=Path,
                        help="explicit finite starting-domain union for --targets; does not limit descendants")
    scope.add_argument("--queries", type=Path, help="symbolic owner domains (JSON); follow successors")
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--owner-base", type=Path, default=Path.cwd())
    parser.add_argument("--expansion-limits", type=Path,
                        help="optional per-call native expansion JSON policy; parsed by Rust")
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
    for option in FINITE_ALLOWANCES:
        parser.add_argument("--" + option, type=positive, help="concrete-target campaign allowance")
    for option in SYMBOLIC_ALLOWANCES:
        parser.add_argument("--" + option,
                            type=DOMAIN.nonnegative if option == DOMAIN.REFINEMENT else
                            DOMAIN.containment_limit if option == "max-containment-checks" else DOMAIN.positive,
                            help="symbolic-domain work allowance; requires --queries")
    parser.add_argument("--" + DOMAIN.REFINEMENT_AXES, choices=DOMAIN.REFINEMENT_AXIS_CHOICES,
                        help="local bounded refinement only (native default: inactive-only); requires --queries; does not change routing or closure")
    parser.add_argument("--" + DOMAIN.TRANSFER_LOOKAHEAD, type=DOMAIN.positive,
                        help="opt into unreserved containment delegation with fixed logical dispatch lookahead; requires --queries and unlimited containment checks")
    parser.add_argument("--route-domain-overcover", action="store_true",
                        help="share admitted symbolic route covers; requires --queries")
    parser.add_argument("--no-progress", action="store_true")
    args = parser.parse_args()
    symbolic = args.queries is not None
    if symbolic and (args.entry_domains is not None or args.expansion_limits is not None or any(
            getattr(args, option.replace("-", "_")) is not None for option in FINITE_ALLOWANCES)):
        parser.error("concrete-target/expansion allowances require --targets")
    if not symbolic and (args.route_domain_overcover or any(
            getattr(args, option.replace("-", "_")) is not None
            for option in (*SYMBOLIC_ALLOWANCES, *SYMBOLIC_POLICIES))):
        parser.error("symbolic-domain allowances require --queries")
    if args.max_route_masks_per_query is not None and not args.route_domain_overcover:
        parser.error("route mask allowance requires --route-domain-overcover")
    if args.transfer_unreserved_lookahead is not None and args.max_containment_checks not in (None, "unlimited"):
        parser.error("--transfer-unreserved-lookahead requires unlimited containment checks")
    if not 1 <= args.workers <= 50 or args.other_workers < 0 or args.workers + args.other_workers > 50:
        parser.error("aggregate configured compute workers must be between 1 and 50")
    if not 0 < args.soft_memory_bytes < args.max_memory_bytes <= 500_000_000_000:
        parser.error("require 0 < soft < hard <= 500 GB (decimal)")
    if not math.isfinite(args.sample_seconds) or args.sample_seconds < 0.1 or not 0 < args.objective_hours < float("inf"):
        parser.error("positive finite sampling interval/objective required")
    cpus = set(map(int, args.cpus.split(","))) if args.cpus else set(sorted(os.sched_getaffinity(0))[:args.workers])
    if len(cpus) != args.workers or not cpus <= os.sched_getaffinity(0):
        parser.error("CPU set must contain exactly --workers permitted CPU IDs")
    collector = ProcessTreeCollector()
    for pid in args.registered_pid:
        try:
            collector.register(pid)
        except (OSError, ValueError, IndexError, StopIteration):
            parser.error(f"registered PID {pid} is not a readable stable process")
    if (args.registered_pid or args.other_workers) and not args.reserved_other_memory_bytes:
        parser.error("concurrent jobs require positive --reserved-other-memory-bytes")
    external, _ = collector.sample()
    external_rss = sum(row["rss_bytes"] for pid, row in external.items()
                       if pid != os.getpid())
    try:
        child_as, monitor_headroom = address_space_envelope(args.max_memory_bytes,
            args.reserved_other_memory_bytes or 0, args.child_address_space_bytes,
            external_rss, resource.getrlimit(resource.RLIMIT_AS))
    except ValueError as error:
        parser.error(str(error))
    # Include the supervisor without treating it as an external reservation.
    collector.register(os.getpid())
    os.sched_setaffinity(0, cpus)
    inputs = [args.manifest, args.queries if symbolic else args.targets, args.executable]
    if args.expansion_limits is not None:
        inputs.append(args.expansion_limits)
    if args.entry_domains is not None:
        inputs.append(args.entry_domains)
    for path in inputs:
        if not path.is_file():
            parser.error(f"not a file: {path}")
    args.tmp_root.mkdir(parents=True, exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="shared-owner-campaign.", dir=args.tmp_root)).resolve()
    stop_file = output / "stop-request.json"
    command = [str(args.executable.resolve()), "owner-domain-match" if symbolic else "routed-campaign",
               "--manifest", str(args.manifest.resolve()),
               "--owner-base", str(args.owner_base.resolve()),
               "--output", str(output / "result.json"), "--events", str(output / "events.jsonl"),
               "--stop-file", str(stop_file), "--workers", str(args.workers)]
    if symbolic:
        command += ["--queries", str(args.queries.resolve()), "--follow-successors"]
        for option in (*SYMBOLIC_ALLOWANCES, *SYMBOLIC_POLICIES):
            if (value := getattr(args, option.replace("-", "_"))) is not None:
                command += ["--" + option, str(value)]
        if args.route_domain_overcover:
            command.append("--route-domain-overcover")
    else:
        command += ["--targets", str(args.targets.resolve())]
        for option, default in FINITE_ALLOWANCES.items():
            value = getattr(args, option.replace("-", "_"))
            command += ["--" + option, str(default if value is None else value)]
    if args.expansion_limits is not None:
        command += ["--expansion-limits", str(args.expansion_limits.resolve())]
    if args.entry_domains is not None:
        command += ["--entry-domains", str(args.entry_domains.resolve())]
    if args.no_progress:
        command.append("--no-progress")
    # Never include the process environment or license in provenance.
    (output / "request.json").write_text(json.dumps({
        "command": command, "cpus": sorted(cpus), "registered_roots": collector.identities,
        "input_scope": "symbolic_domains" if symbolic else "concrete_targets",
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
        try:
            collector.register(child.pid)
        except (OSError, ValueError, IndexError, StopIteration):
            if child.poll() is None:
                raise
        initial, _ = collector.sample()
        print(f"Campaign receipts: {output}", flush=True)
        peak = 0
        seen_cpu = {(pid, row["start"]): row["cpu_seconds"]
                    for pid, row in initial.items()}
        cumulative_cpu = 0.0
        last_time = time.monotonic()
        previous_rss = None
        hard_stopped = False
        with (output / "resources.jsonl").open("x") as resources:
            while child.poll() is None:
                if stop_file.exists() and stop_reason is None:
                    request_stop("existing_operator_stop_file")
                tree, collection = collector.sample()
                rss = sum(row["rss_bytes"] for row in tree.values())
                now = time.monotonic()
                delta_cpu, seen_cpu, process_rows = process_cpu_sample(
                    tree, seen_cpu, now-last_time, os.getpid(), child.pid)
                cumulative_cpu += delta_cpu
                peak = max(peak, rss)
                append_record(resources, {"event": "resources", "elapsed_seconds": now-started,
                    "aggregate_rss_bytes": rss, "peak_observed_rss_bytes": peak,
                    "sampled_cpu_seconds_since_start": cumulative_cpu, "observed_busy_cores": delta_cpu/(now-last_time),
                    "rss_growth_bytes_per_second": None if previous_rss is None else (rss-previous_rss)/(now-last_time),
                    "processes": len(tree), "process_cpu": process_rows, "collection": collection,
                    "configured_workers": args.workers+args.other_workers,
                    "objective_hours": args.objective_hours, "past_objective": now-started > args.objective_hours*3600,
                    "cancel_reason": stop_reason, "registered_scope_only": True})
                last_time = now
                previous_rss = rss
                if rss >= args.soft_memory_bytes:
                    request_stop("aggregate_rss_soft_limit")
                if rss >= args.max_memory_bytes and not hard_stopped:
                    # Other registered jobs are measured, never signalled.
                    if child.pid in tree and tree[child.pid]["start"] == collector.identities.get(child.pid):
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
