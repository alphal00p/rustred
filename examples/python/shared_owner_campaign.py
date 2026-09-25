#!/usr/bin/env python3
"""Steer one generic Rust shared-owner campaign (Linux).

Python performs no algebra. Saved owners are reused, not regenerated. Results
are finite-target traces (--targets) or symbolic-domain walks (--queries),
not universal R10 closure claims. Only Rust's durable checkpoint is resumable.
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
import shlex
import subprocess
import sys
import tempfile
import time
import uuid

INNER_POOLS = ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
               "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")

# Reuse the thin domain driver's option whitelist without depending on the
# caller's working directory or Python module search path.
_DOMAIN_SPEC = importlib.util.spec_from_file_location(
    "owner_domain_steering", Path(__file__).with_name("match_shared_owner_domains.py"))
DOMAIN = importlib.util.module_from_spec(_DOMAIN_SPEC)
_DOMAIN_SPEC.loader.exec_module(DOMAIN)
_MONITOR_SPEC = importlib.util.spec_from_file_location(
    "campaign_monitor", Path(__file__).with_name("campaign_monitor.py"))
MONITOR = importlib.util.module_from_spec(_MONITOR_SPEC)
_MONITOR_SPEC.loader.exec_module(MONITOR)
SYMBOLIC_ALLOWANCES = (*DOMAIN.ALLOWANCES, DOMAIN.REFINEMENT,
                      *(name for name in DOMAIN.WALK_ALLOWANCES if name != "workers"),
                      "max-route-masks-per-query")
SYMBOLIC_POLICIES = (DOMAIN.REFINEMENT_AXES, DOMAIN.TRANSFER_LOOKAHEAD,
                    DOMAIN.PUBLICATION_POLICY, DOMAIN.INSPECTION_WORKERS, DOMAIN.APPLICATION_REFINEMENT)
FINITE_ALLOWANCES = {
    "max-nodes": 16_000_000,
    "max-input-targets": 100_000,
    "max-transport-operations": 4_096_000_000,
    "max-transport-endpoints": 1_024_000_000,
    "max-coalescing-additions": 256_000_000,
    "max-rule-applications": 16_000_000,
}


def restart_command(args, output: Path, checkpoint: str, cpus: set[int]) -> list[str]:
    """Replay all resolved launch policy, with fresh receipts and native resume."""
    command = [sys.executable, str(Path(__file__).resolve())]
    for name, value in vars(args).items():
        if name in ("checkpoint", "resume", "run_directory", "cpus") or value is None or value is False:
            continue
        option = "--" + name.replace("_", "-")
        if value is True:
            command.append(option)
        else:
            for item in value if isinstance(value, list) else [value]:
                command += [option, str(item.resolve() if isinstance(item, Path) else item)]
    destination = output.with_name(output.name + ".resume-" + uuid.uuid4().hex[:12])
    command += ["--cpus", ",".join(map(str, sorted(cpus))), "--resume", checkpoint,
                "--run-directory", str(destination)]
    return command


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
    """Per-process deltas; an unknown interval establishes the first baseline."""
    rows = []
    current = {}
    delta = 0.0
    for pid, row in sorted(table.items()):
        identity = (pid, row["start"])
        cpu = row["cpu_seconds"]
        sampled_delta = (max(0.0, cpu - previous[identity])
                         if elapsed is not None and identity in previous else None)
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
                           external_rss: int, inherited: tuple[int, int]) -> tuple[int | None, int]:
    """Optional diagnostic AS cap; production retains the inherited limit."""
    available = hard_bytes - other_reserve
    if other_reserve < external_rss or available <= 0:
        raise ValueError("external reserve is insufficient or leaves no child address space")
    if explicit is not None and not 0 < explicit <= available:
        raise ValueError("child address-space limit exceeds the admitted memory envelope")
    if explicit is None:
        return None, 0
    admitted = min([explicit]
                   + [limit for limit in inherited if limit != resource.RLIM_INFINITY])
    if admitted <= 0:
        raise ValueError("inherited address-space limit leaves no positive child allowance")
    return admitted, 0


def host_memory(proc_root=Path("/proc"), cgroup_root=Path("/sys/fs/cgroup")) -> dict:
    """Observe host availability and any readable enclosing cgroup-v2 limits."""
    fields = {}
    for line in (proc_root / "meminfo").read_text().splitlines():
        name, _, value = line.partition(":")
        if name in ("MemTotal", "MemAvailable"):
            fields[name] = int(value.split()[0]) * 1024
    if not fields.get("MemTotal") or "MemAvailable" not in fields:
        raise ValueError("host MemTotal/MemAvailable are required for RAM protection")
    cgroup_remaining = []
    cgroup_limits = []
    try:
        memberships = (proc_root / "self" / "cgroup").read_text().splitlines()
        relative = next(row[3:] for row in memberships if row.startswith("0::"))
        parts = Path(relative).parts
        if ".." not in parts:
            directory = cgroup_root.joinpath(*[part for part in parts if part != "/"])
            while directory == cgroup_root or cgroup_root in directory.parents:
                try:
                    maximum = (directory / "memory.max").read_text().strip()
                    if maximum != "max":
                        current = int((directory / "memory.current").read_text())
                        cgroup_limits.append(int(maximum))
                        cgroup_remaining.append(max(0, int(maximum) - current))
                except (OSError, ValueError):
                    pass
                if directory == cgroup_root:
                    break
                directory = directory.parent
    except (OSError, StopIteration):
        pass
    available = min([fields["MemAvailable"], *cgroup_remaining])
    return {"host_total_bytes": fields["MemTotal"],
            "host_available_bytes": fields["MemAvailable"],
            "cgroup_remaining_bytes": min(cgroup_remaining) if cgroup_remaining else None,
            "cgroup_capacity_bytes": min(cgroup_limits) if cgroup_limits else None,
            "available_bytes": available}


def memory_admission(hard: int, soft: int | None, snapshot: dict, reserve: int | None,
                     margin_percent: float = 5.0):
    capacity = min(snapshot["host_total_bytes"], snapshot.get("cgroup_capacity_bytes") or snapshot["host_total_bytes"])
    reserve = min(20_000_000_000, capacity // 20) if reserve is None else reserve
    admitted_hard = min(hard, snapshot["available_bytes"] - reserve)
    if admitted_hard <= 0:
        raise ValueError("host/cgroup RAM availability leaves no campaign headroom")
    admitted_soft = int(admitted_hard * (1 - margin_percent / 100)) if soft is None else min(soft, int(admitted_hard * (1 - margin_percent / 100)))
    if not 0 < admitted_soft < admitted_hard:
        raise ValueError("host/cgroup RAM allowance and margin must leave a positive soft limit below hard")
    return admitted_hard, admitted_soft, reserve


@contextmanager
def owned_process(command, env, cpus, request_stop, child_address_space=None, stdout=None, stderr=None):
    """Own the child's entire post-spawn lifetime, including receipt failures."""
    def configure_child():
        os.sched_setaffinity(0, cpus)
        if child_address_space is not None:
            resource.setrlimit(resource.RLIMIT_AS, (child_address_space, child_address_space))
    child = subprocess.Popen(command, env=env, start_new_session=True, preexec_fn=configure_child,
                             stdout=stdout, stderr=stderr)
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
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
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
    parser.add_argument("--soft-memory-bytes", type=positive,
                        help="optional earlier cooperative RAM stop; default 95%% of effective hard limit")
    parser.add_argument("--ram-guard-margin-percent", type=float, default=5.0,
                        help="checkpoint-and-stop margin below the hard RAM ceiling (default: 5%%)")
    parser.add_argument("--reserved-other-memory-bytes", type=nonnegative,
                        help="required envelope for all declared concurrent external jobs")
    parser.add_argument("--child-address-space-bytes", type=positive,
                        help="diagnostic OS RLIMIT_AS opt-in; default preserves inherited limits without adding a cap")
    parser.add_argument("--host-memory-reserve-bytes", type=positive,
                        help="RAM left available to host/cgroup; default min(20 GB, 5%% host RAM)")
    parser.add_argument("--objective-hours", type=float, default=15.0,
                        help="performance objective only; never a termination timer")
    parser.add_argument("--sample-seconds", type=float, default=2.0)
    parser.add_argument("--tmp-root", type=Path, default=Path("TMP"))
    parser.add_argument("--run-directory", type=Path,
                        help="new named receipt directory; must not already exist")
    checkpoint = parser.add_mutually_exclusive_group()
    checkpoint.add_argument("--checkpoint", type=Path, help="create a Rust resumable checkpoint directory")
    checkpoint.add_argument("--resume", type=Path, help="resume Rust's latest durable checkpoint generation")
    parser.add_argument("--checkpoint-interval-seconds", type=positive)
    parser.add_argument("--unbounded-work", action=DOMAIN.StoreTrueOnce, nargs=0, default=False,
                        help="remove cumulative native work stops; retain memory, scratch and algebra admission")
    parser.add_argument("--apply-subdivision-axis", type=nonnegative, action=DOMAIN.StoreOnce)
    parser.add_argument("--apply-subdivision-cut", type=nonnegative, action=DOMAIN.StoreOnce)
    parser.add_argument("--plain-progress-seconds", type=float, default=30.0,
                        help="interval between plain non-TTY status lines")
    for option in FINITE_ALLOWANCES:
        parser.add_argument("--" + option, type=positive, help="concrete-target campaign allowance")
    for option in SYMBOLIC_ALLOWANCES:
        parser.add_argument("--" + option,
                            type=DOMAIN.query_allowance if option in DOMAIN.QUERY_ALLOWANCES else
                            DOMAIN.nonnegative if option == DOMAIN.REFINEMENT else
                            DOMAIN.containment_limit if option == "max-containment-checks" else DOMAIN.positive,
                            action=DOMAIN.StoreOnce if option in DOMAIN.QUERY_ALLOWANCES else "store",
                            help="symbolic-domain work allowance; requires --queries")
    parser.add_argument("--" + DOMAIN.REFINEMENT_AXES, choices=DOMAIN.REFINEMENT_AXIS_CHOICES,
                        help="local bounded refinement only (native default: inactive-only); requires --queries; does not change routing or closure")
    parser.add_argument("--" + DOMAIN.TRANSFER_LOOKAHEAD, type=DOMAIN.positive,
                        help="opt into unreserved containment delegation with fixed logical dispatch lookahead; requires --queries and unlimited containment checks")
    parser.add_argument("--" + DOMAIN.INITIAL_D_REUSE, action=DOMAIN.StoreTrueOnce, nargs=0, default=False,
                        help="reuse an exact initial same-owner D band, retaining its obligation; requires --queries and unreserved delegation")
    parser.add_argument("--" + DOMAIN.PUBLICATION_POLICY, choices=DOMAIN.PUBLICATION_POLICIES,
                        help="symbolic publication policy; requires --queries; ready requires unreserved delegation; nonordered modes may change diagnostic traversal order")
    parser.add_argument("--" + DOMAIN.INSPECTION_WORKERS, type=DOMAIN.positive, action=DOMAIN.StoreOnce,
                        help="explicit symbolic compute partition: N inspectors, workers-1-N admission helpers and one coordinator; requires --queries")
    parser.add_argument("--" + DOMAIN.APPLICATION_REFINEMENT, type=DOMAIN.application_cardinality, action=DOMAIN.StoreOnce,
                        help="opt into one finite selected Apply-cell axis singleton refinement; positive cardinality, default off, not a cumulative work cap; requires --queries")
    parser.add_argument("--route-domain-overcover", action="store_true",
                        help="share admitted symbolic route covers; requires --queries")
    parser.add_argument("--" + DOMAIN.JOINT_SUPPORT_PRUNING, action=DOMAIN.StoreTrueOnce, nargs=0, default=False,
                        help="opt into joint source-support mask pruning; requires --queries and route overcover; default off")
    parser.add_argument("--no-progress", action="store_true")
    args = parser.parse_args()
    symbolic = args.queries is not None
    if not symbolic and (args.checkpoint is not None or args.resume is not None
                         or args.checkpoint_interval_seconds is not None or args.unbounded_work
                         or args.apply_subdivision_axis is not None or args.apply_subdivision_cut is not None):
        parser.error("checkpoint, unbounded work and subdivision require --queries")
    if args.checkpoint_interval_seconds is not None and args.checkpoint is None and args.resume is None:
        parser.error("checkpoint interval requires --checkpoint or --resume")
    if args.checkpoint is not None or args.resume is not None:
        args.checkpoint_interval_seconds = args.checkpoint_interval_seconds or 3600
    if (args.apply_subdivision_axis is None) != (args.apply_subdivision_cut is None):
        parser.error("subdivision requires both --apply-subdivision-axis and --apply-subdivision-cut")
    if symbolic and (args.entry_domains is not None or args.expansion_limits is not None or any(
            getattr(args, option.replace("-", "_")) is not None for option in FINITE_ALLOWANCES)):
        parser.error("concrete-target/expansion allowances require --targets")
    if not symbolic and (args.route_domain_overcover or args.route_joint_source_support_pruning or args.reuse_initial_d_bands or any(
            getattr(args, option.replace("-", "_")) is not None
            for option in (*SYMBOLIC_ALLOWANCES, *SYMBOLIC_POLICIES))):
        parser.error("symbolic-domain allowances require --queries")
    if args.max_route_masks_per_query is not None and not args.route_domain_overcover:
        parser.error("route mask allowance requires --route-domain-overcover")
    if args.route_joint_source_support_pruning and not args.route_domain_overcover:
        parser.error("joint source-support pruning requires --route-domain-overcover")
    if args.transfer_unreserved_lookahead is not None and args.max_containment_checks not in (None, "unlimited"):
        parser.error("--transfer-unreserved-lookahead requires unlimited containment checks")
    if args.reuse_initial_d_bands and args.transfer_unreserved_lookahead is None:
        parser.error("--reuse-initial-d-bands requires --transfer-unreserved-lookahead")
    DOMAIN.validate_publication_policy(parser, args.publication_policy, args.transfer_unreserved_lookahead,
                                       args.checkpoint is not None or args.resume is not None,
                                       args.apply_subdivision_axis is not None)
    if not 1 <= args.workers <= 50 or args.other_workers < 0 or args.workers + args.other_workers > 50:
        parser.error("aggregate configured compute workers must be between 1 and 50")
    DOMAIN.validate_inspection_workers(parser, args.workers, args.inspection_workers,
                                       args.max_containment_checks)
    if args.max_memory_bytes <= 0 or args.soft_memory_bytes is not None and not 0 < args.soft_memory_bytes < args.max_memory_bytes:
        parser.error("require a positive hard RAM limit and any explicit soft allowance below hard")
    if not math.isfinite(args.ram_guard_margin_percent) or not 0 < args.ram_guard_margin_percent < 100:
        parser.error("RAM guard margin must be finite and strictly between 0 and 100 percent")
    if not math.isfinite(args.sample_seconds) or args.sample_seconds < 0.1 or not 0 < args.objective_hours < float("inf"):
        parser.error("positive finite sampling interval/objective required")
    if not math.isfinite(args.plain_progress_seconds) or args.plain_progress_seconds < 0.1:
        parser.error("plain progress interval must be finite and at least 0.1 seconds")
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
        initial_host = host_memory()
        effective_hard, effective_soft, host_reserve = memory_admission(
            args.max_memory_bytes, args.soft_memory_bytes, initial_host, args.host_memory_reserve_bytes,
            args.ram_guard_margin_percent)
        child_as, monitor_headroom = address_space_envelope(effective_hard,
            args.reserved_other_memory_bytes or 0, args.child_address_space_bytes,
            external_rss, resource.getrlimit(resource.RLIMIT_AS))
        monitor_headroom = effective_hard - effective_soft
    except (OSError, ValueError) as error:
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
    if args.run_directory is not None:
        output = args.run_directory.resolve()
        try:
            output.mkdir(parents=True, exist_ok=False)
        except OSError as error:
            parser.error(f"new run directory: {error}")
    else:
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
        if args.route_joint_source_support_pruning:
            command.append("--" + DOMAIN.JOINT_SUPPORT_PRUNING)
        if args.reuse_initial_d_bands:
            command.append("--" + DOMAIN.INITIAL_D_REUSE)
        for option in ("checkpoint", "resume", "checkpoint_interval_seconds",
                       "apply_subdivision_axis", "apply_subdivision_cut"):
            if (value := getattr(args, option)) is not None:
                command += ["--" + option.replace("_", "-"),
                            str(value.resolve()) if isinstance(value, Path) else str(value)]
        if args.unbounded_work:
            command.append("--unbounded-work")
    else:
        command += ["--targets", str(args.targets.resolve())]
        for option, default in FINITE_ALLOWANCES.items():
            value = getattr(args, option.replace("-", "_"))
            command += ["--" + option, str(default if value is None else value)]
    if args.expansion_limits is not None:
        command += ["--expansion-limits", str(args.expansion_limits.resolve())]
    if args.entry_domains is not None:
        command += ["--entry-domains", str(args.entry_domains.resolve())]
    # One supervisor display owns the terminal; native JSONL heartbeats continue.
    command.append("--no-progress")
    checkpoint_directory = args.checkpoint or args.resume
    checkpoint_directory = str(checkpoint_directory.resolve()) if checkpoint_directory is not None else None
    # Never include the process environment or license in provenance.
    (output / "request.json").write_text(json.dumps({
        "command": command, "cpus": sorted(cpus), "registered_roots": collector.identities,
        "input_scope": "symbolic_domains" if symbolic else "concrete_targets",
        "reuse_initial_d_bands": args.reuse_initial_d_bands,
        "publication_policy": (args.publication_policy or "ordered") if symbolic else None,
        "requested_inspection_workers": args.inspection_workers,
        "requested_max_queries": args.max_queries,
        "requested_max_query_bytes": args.max_query_bytes,
        "workers": args.workers, "other_workers": args.other_workers,
        "hard_memory_bytes": args.max_memory_bytes, "soft_memory_bytes": args.soft_memory_bytes,
        "effective_hard_memory_bytes": effective_hard, "effective_soft_memory_bytes": effective_soft,
        "host_memory_reserve_bytes": host_reserve, "initial_host_memory": initial_host,
        "ram_guard_margin_percent": args.ram_guard_margin_percent,
        "child_rlimit_as_bytes": child_as, "monitor_headroom_bytes": monitor_headroom,
        "inherited_rlimit_as_bytes": [None if value == resource.RLIM_INFINITY else value
                                     for value in resource.getrlimit(resource.RLIMIT_AS)],
        "reserved_other_memory_bytes": args.reserved_other_memory_bytes or 0,
        "supervisor_pid": os.getpid(), "address_space_scope": "owned_single_process",
        "objective_hours": args.objective_hours, "hard_timeout": None,
        "checkpoint_directory": checkpoint_directory, "resume_requested": args.resume is not None,
        "checkpoint_interval_seconds": args.checkpoint_interval_seconds,
        "unbounded_work": args.unbounded_work,
        "apply_cell_refinement_max_cardinality": args.apply_cell_refinement_max_cardinality,
        "apply_subdivision": None if args.apply_subdivision_axis is None else {
            "axis": args.apply_subdivision_axis, "cut": args.apply_subdivision_cut},
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
    tail = MONITOR.EventTail(output / "events.jsonl")
    presenter = MONITOR.Presenter(enabled=not args.no_progress,
                                  plain_seconds=args.plain_progress_seconds)
    last_checkpoint = {"state": "awaiting_first_save", "directory": checkpoint_directory} if checkpoint_directory else None
    resume_command = None
    identities = {"supervisor": {"pid": os.getpid(), "start_ticks": collector.identities.get(os.getpid())}}
    try:
        identities["boot_id"] = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except OSError:
        identities["boot_id"] = None

    def publish_status(resources, state, exit_status=None):
        nonlocal last_checkpoint
        now = time.monotonic()
        read_error = None
        try:
            tail.poll(now)
        except OSError as error:
            read_error = str(error)
        progress = MONITOR.progress_summary(tail.latest, tail.observed_at, now)
        for candidate in (progress["checkpoint"], tail.saved_checkpoint):
            generation = None if candidate is None else MONITOR.number(candidate.get("generation"))
            if isinstance(generation, int) and (last_checkpoint is None or
                    generation >= last_checkpoint.get("generation", 0)):
                last_checkpoint = candidate
        writings = [candidate for candidate in (progress.get("checkpoint_write"), tail.checkpoint_write)
                    if candidate is not None and isinstance(MONITOR.number(candidate.get("generation")), int)]
        writing = max(writings, key=lambda candidate: candidate["generation"], default=None)
        if writing and last_checkpoint and writing["generation"] <= last_checkpoint.get("generation", 0):
            writing = None
        if exit_status is not None:
            writing = None
        status = {"schema": "rustred.campaign-status.v1", "heartbeat_unix_time": time.time(),
                  "heartbeat_age_seconds": 0.0, "heartbeat_stale": False,
                  "state": state, "elapsed_seconds": now-started, "sample_seconds": args.sample_seconds,
                  "run_directory": str(output), "process_identity": identities,
                  "workers": args.workers, "other_workers": args.other_workers,
                  "hard_memory_bytes": effective_hard, "soft_memory_bytes": effective_soft,
                  "requested_hard_memory_bytes": args.max_memory_bytes,
                  "host_memory_reserve_bytes": host_reserve,
                  "ram_guard_margin_percent": args.ram_guard_margin_percent,
                  "progress": progress, "resources": resources, "checkpoint": last_checkpoint,
                  "checkpoint_write": writing,
                  "checkpoint_milestones": list(tail.milestones),
                  "resume_command": resume_command,
                  "stop_reason": stop_reason, "exit_status": exit_status,
                  "event_reader": tail.diagnostics(), "event_read_error": read_error,
                  "hard_timeout_seconds": None, "closure_eta_seconds": None,
                  "native_stderr": str(output / "native.stderr"), "family_closure_claim": False}
        MONITOR.atomic_json(output / "status.json", status)
        try:
            presenter.render(status, force=exit_status is not None)
        except OSError:
            # A detached/closed terminal does not invalidate durable monitoring.
            presenter.enabled = False
        return status

    publish_status({}, "starting")
    # preexec_fn is used in this single-threaded driver only, before Rust/native
    # pools exist; the new session makes hard-stop ownership unambiguous.
    with (output / "native.stdout").open("x") as native_stdout, (output / "native.stderr").open("x") as native_stderr, owned_process(
            command, env, cpus, request_stop, child_as, native_stdout, native_stderr) as child:
        (output / "run.pid").write_text(str(child.pid) + "\n")
        try:
            collector.register(child.pid)
        except (OSError, ValueError, IndexError, StopIteration):
            if child.poll() is None:
                raise
        identities["native"] = {"pid": child.pid, "start_ticks": collector.identities.get(child.pid)}
        MONITOR.atomic_json(output / "processes.json", identities)
        print(f"Campaign receipts: {output}", flush=True)
        peak = 0
        seen_cpu = {}
        cumulative_cpu = 0.0
        # The first observation is a baseline, not a sub-millisecond interval:
        # /proc CPU counters are tick-quantized. Subsequent observations follow
        # the validated >=0.1s sleep; do not clamp genuine over-budget activity.
        last_time = None
        previous_rss = None
        hard_stopped = False
        last_resources = {}
        with (output / "resources.jsonl").open("x") as resources:
            while child.poll() is None:
                if stop_file.exists() and stop_reason is None:
                    request_stop("existing_operator_stop_file")
                tree, collection = collector.sample()
                rss = sum(row["rss_bytes"] for row in tree.values())
                now = time.monotonic()
                sample_interval = None if last_time is None else now-last_time
                delta_cpu, seen_cpu, process_rows = process_cpu_sample(
                    tree, seen_cpu, sample_interval, os.getpid(), child.pid)
                cumulative_cpu += delta_cpu
                peak = max(peak, rss)
                try:
                    host = host_memory()
                except (OSError, ValueError):
                    host = {"host_available_bytes": None, "available_bytes": None}
                    request_stop("host_memory_monitor_unavailable")
                available = host["available_bytes"]
                if rss >= effective_soft:
                    request_stop("aggregate_rss_soft_limit")
                if available is not None and available <= host_reserve:
                    request_stop("host_memory_reserve")
                native_cpu = next((row["observed_busy_cores"] for row in process_rows
                                   if row["role"] == "owned_native"), None)
                last_resources = {"event": "resources", "elapsed_seconds": now-started,
                    "aggregate_rss_bytes": rss, "peak_observed_rss_bytes": peak,
                    "sampled_cpu_seconds_since_start": cumulative_cpu,
                    "cpu_sample_interval_seconds": sample_interval,
                    "observed_busy_cores": None if sample_interval is None else delta_cpu/sample_interval,
                    "native_busy_cores": native_cpu, **host,
                    "rss_growth_bytes_per_second": None if previous_rss is None else (rss-previous_rss)/(now-last_time),
                    "processes": len(tree), "process_cpu": process_rows, "collection": collection,
                    "configured_workers": args.workers+args.other_workers,
                    "objective_hours": args.objective_hours, "past_objective": now-started > args.objective_hours*3600,
                    "cancel_reason": stop_reason, "registered_scope_only": True}
                append_record(resources, last_resources)
                last_time = now
                previous_rss = rss
                if (rss >= effective_hard or available is not None and available <= host_reserve // 4) and not hard_stopped:
                    # Other registered jobs are measured, never signalled.
                    if child.pid in tree and tree[child.pid]["start"] == collector.identities.get(child.pid):
                        request_stop("aggregate_rss_hard_limit" if rss >= effective_hard else "host_memory_emergency")
                        try:
                            os.killpg(child.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        hard_stopped = True
                publish_status(last_resources, "stopping" if stop_reason else "running")
                time.sleep(args.sample_seconds)
    status = child.wait()
    tail.poll()
    terminal_progress = MONITOR.progress_summary(tail.latest, tail.observed_at, time.monotonic())
    for candidate in (terminal_progress["checkpoint"], tail.saved_checkpoint):
        generation = None if candidate is None else MONITOR.number(candidate.get("generation"))
        if isinstance(generation, int) and (last_checkpoint is None or
                generation >= last_checkpoint.get("generation", 0)):
            last_checkpoint = candidate
    paused = (status == 4 and terminal_progress.get("native_status") == "paused"
              and last_checkpoint is not None and last_checkpoint.get("state") == "saved")
    if last_checkpoint and last_checkpoint.get("state") == "saved" and checkpoint_directory:
        resume_command = restart_command(args, output, checkpoint_directory, cpus)
    state = "completed" if status == 0 else "paused" if paused else "stopped" if stop_reason else "failed"
    publish_status(last_resources, state, status)
    (output / "run.status").write_text(str(status) + "\n")
    (output / "supervisor-result.json").write_text(json.dumps({
        "exit_status": status, "elapsed_seconds": time.monotonic()-started,
        "reuse_initial_d_bands": args.reuse_initial_d_bands,
        "publication_policy": (args.publication_policy or "ordered") if symbolic else None,
        "requested_inspection_workers": args.inspection_workers,
        "requested_max_queries": args.max_queries,
        "requested_max_query_bytes": args.max_query_bytes,
        "peak_observed_aggregate_rss_bytes": peak, "operator_or_resource_stop": stop_reason,
        "hard_stopped": hard_stopped,
        "work_checkpoint": bool(last_checkpoint and last_checkpoint.get("state") == "saved"
                                and not last_checkpoint.get("bootstrap", False)),
        "resume_checkpoint_available": bool(last_checkpoint and last_checkpoint.get("state") == "saved"),
        "checkpoint": last_checkpoint, "state": state, "resume_requested": args.resume is not None,
        "resume_command": resume_command,
        "unbounded_work": args.unbounded_work, "family_closure_claim": False,
        "child_rlimit_as_bytes": child_as, "memory_failure_is_incomplete": True,
        "requested_hard_memory_bytes": args.max_memory_bytes,
        "effective_hard_memory_bytes": effective_hard, "effective_soft_memory_bytes": effective_soft,
        "host_memory_reserve_bytes": host_reserve,
        "ram_guard_margin_percent": args.ram_guard_margin_percent,
        "rust_result_present": (output / "result.json").is_file(),
    }, indent=2) + "\n")
    if resume_command:
        print(f"Durable checkpoint: {checkpoint_directory}", flush=True)
        print("Resume with fresh receipts: " + shlex.join(resume_command), flush=True)
    return status if status >= 0 else 128-status


if __name__ == "__main__":
    raise SystemExit(main())
