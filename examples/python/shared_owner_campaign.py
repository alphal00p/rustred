#!/usr/bin/env python3
"""Steer one generic Rust shared-owner campaign (Linux).

Python performs no algebra. Saved owners are reused, not regenerated. Results
are finite-target traces (--targets) or symbolic-domain walks (--queries),
not universal R10 closure claims. Only Rust's durable checkpoint is resumable.
The 15-hour objective is telemetry, NOT a timeout. No license is persisted.
"""
from __future__ import annotations

import argparse
import hashlib
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
# The native walk's correctness-gate pause seam (ready_resume_control.py runs
# the executable directly) and the epoch engine's lockstep-size override (a
# diagnostic that changes the walk, W2 S2 note section 4); a supervised
# campaign never inherits either from the operator's shell.
DIAGNOSTIC_ONLY_ENVIRONMENT = ("RUSTRED_WALK_DIAGNOSTIC_PAUSE", "RUSTRED_EPOCH_LOCKSTEP_B")

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
_HEARTBEAT_SPEC = importlib.util.spec_from_file_location(
    "heartbeat_metrics", Path(__file__).with_name("heartbeat_metrics.py"))
HEARTBEAT = importlib.util.module_from_spec(_HEARTBEAT_SPEC)
_HEARTBEAT_SPEC.loader.exec_module(HEARTBEAT)
_ROLES_SPEC = importlib.util.spec_from_file_location(
    "owner_query_roles", Path(__file__).with_name("owner_query_roles.py"))
ROLES = importlib.util.module_from_spec(_ROLES_SPEC)
_ROLES_SPEC.loader.exec_module(ROLES)
# Aggregate outer compute workers admitted by the Python drivers; the native
# CLI has its own cap, and the permitted CPU affinity always bounds this.
MAX_WORKERS = 256
SYMBOLIC_ALLOWANCES = (*DOMAIN.ALLOWANCES, DOMAIN.REFINEMENT,
                      *(name for name in DOMAIN.WALK_ALLOWANCES if name != "workers"),
                      "max-route-masks-per-query")
SYMBOLIC_POLICIES = (DOMAIN.REFINEMENT_AXES, DOMAIN.TRANSFER_LOOKAHEAD,
                    DOMAIN.PUBLICATION_POLICY, DOMAIN.EPOCH_INSPECTOR_LOOKUP,
                    *DOMAIN.EPOCH_DATA_OPTIONS,
                    DOMAIN.INSPECTION_WORKERS, DOMAIN.APPLICATION_REFINEMENT,
                    DOMAIN.FRONTIER_POLICY)
G2_RESIDUAL_ANCHORS = "g2-residual-anchors"
G2_RESIDUAL_MODES = ("off", "union")
# Host-aware RAM guard defaults (see RamGuard). The MemAvailable floor is the
# host memory reserve: 50 GB unless --host-memory-reserve-bytes says otherwise
# (owner decision 2026-09-27; formerly min(20 GB, 5% of host/cgroup RAM)).
# Admission keeps the whole floor free (ADMISSION_RULE), so the effective hard
# cap falls below the requested one whenever MemAvailable < requested + floor
# at a (re)start. Sustained growth of the supervised tree's OWN swapped-out
# memory (VmSwap summed over its processes) at or above the rate for the whole
# window also saves and stops; 0 disables it. Host-wide swap-in
# (/proc/vmstat pswpin) counts every user's swap-in on a shared host and is
# telemetry only.
DEFAULT_HOST_MEMORY_RESERVE_BYTES = 50_000_000_000
DEFAULT_SWAP_GROWTH_STOP_BYTES_PER_SECOND = 32 * 1024 * 1024
DEFAULT_SWAP_GROWTH_STOP_SECONDS = 120.0
ADMISSION_RULE = ("effective hard = min(requested hard, available_bytes - host_memory_reserve_bytes) and "
                  "effective soft = effective hard x (1 - margin), evaluated once at every (re)start from "
                  "host/cgroup MemAvailable (which excludes a shrinkable ZFS ARC)")
# A host-wide stop (MemAvailable floor or emergency) carries an attribution:
# own_memory_signal is true when the supervised tree's RSS growth over the
# trailing window explains at least this share of the MemAvailable drop over
# the same window, or its own swap is growing at or above the stop rate.
# The production liveness guard counts host-wide stops only with the signal.
OWN_MEMORY_SHARE = 0.5
HOST_WIDE_STOPS = ("host_memory_reserve", "host_memory_emergency")
FINITE_ALLOWANCES = {
    "max-nodes": 16_000_000,
    "max-input-targets": 100_000,
    "max-transport-operations": 4_096_000_000,
    "max-transport-endpoints": 1_024_000_000,
    "max-coalescing-additions": 256_000_000,
    "max-rule-applications": 16_000_000,
}


def resumable_checkpoint(checkpoint):
    """CP6 explicitly attests durability/usability; retain the CP5 contract."""
    return (isinstance(checkpoint, dict) and checkpoint.get("state") == "saved"
            and (checkpoint.get("format") != "RUSTRED-WALK-CP6"
                 or checkpoint.get("resumable") is True))


def advance_checkpoint(current, candidate):
    """CP6 terminal invalidation survives replay of cached saved events."""
    if (isinstance(current, dict) and current.get("format") == "RUSTRED-WALK-CP6"
            and current.get("state") in ("poisoned", "unconfirmed_after_process_exit")):
        return current
    if isinstance(candidate, dict):
        if candidate.get("format") == "RUSTRED-WALK-CP6" and candidate.get("state") == "poisoned":
            return candidate
        generation = MONITOR.number(candidate.get("generation"))
        if isinstance(generation, int) and (current is None or generation >= current.get("generation", 0)):
            return candidate
    return current


def terminal_checkpoint(policy, status, event, checkpoint):
    """A stale heartbeat is insufficient after a missing/failed CP6 terminal handoff."""
    if policy != "epoch" or not isinstance(checkpoint, dict) or checkpoint.get("format") != "RUSTRED-WALK-CP6":
        return checkpoint
    if checkpoint.get("state") == "poisoned":
        return checkpoint
    native = event.get("progress", event)
    native = native if isinstance(native, dict) else {}
    native = native.get("snapshot", native)
    final = native.get("checkpoint") if isinstance(native, dict) else None
    if (status == 4 and isinstance(native, dict) and native.get("event") == "finished"
            and native.get("full_result_in_output_document") is False
            and native.get("full_state_in_checkpoint") is True
            and resumable_checkpoint(final) and final.get("format") == "RUSTRED-WALK-CP6"
            and final.get("generation") == checkpoint.get("generation")):
        return checkpoint
    return dict(checkpoint, state="unconfirmed_after_process_exit", resumable=False,
                reason="no complete CP6 terminal handoff; inspect the raw checkpoint before resuming")


def terminal_state(status, policy, event, progress, checkpoint, operator_stop):
    """CP6 checkpoint-only output never masquerades as solver completion."""
    saved = resumable_checkpoint(checkpoint)
    if policy == "epoch" and isinstance(checkpoint, dict) and checkpoint.get("format") == "RUSTRED-WALK-CP6":
        if not saved:
            return "failed"
        native = event.get("progress", event)
        native = native if isinstance(native, dict) else {}
        native = native.get("snapshot", native)
        if status == 4 and isinstance(native, dict) and native.get("full_result_in_output_document") is False:
            if (progress.get("native_status") == "incomplete"
                    and native.get("recursive_worklist_exhausted") is True
                    and native.get("all_scheduled_domains_resolved") is False
                    and native.get("finalization") == "not_evaluated"):
                return "checkpoint_only"
            if progress.get("native_status") == "stopped":
                return ("paused" if progress.get("native_stop_reason") == "paused"
                        else "stopped")
        return "stopped" if operator_stop else "failed"
    paused = status == 4 and progress.get("native_status") == "paused" and saved
    return "completed" if status == 0 else "paused" if paused else "stopped" if operator_stop else "failed"


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
    # Flat names: an automatic rescue loop resumes many times, and nested
    # `.resume-` suffixes would outgrow the file-name limit.
    base = output.name.split(".resume-", 1)[0]
    destination = output.with_name(base + ".resume-" + uuid.uuid4().hex[:12])
    command += ["--cpus", ",".join(map(str, sorted(cpus))), "--resume", checkpoint,
                "--run-directory", str(destination)]
    return command


def positive(text: str) -> int:
    value = int(text)
    if value <= 0:
        raise argparse.ArgumentTypeError("must be positive")
    return value


def validate_g2_residual_anchors(mode, lookahead, publication, subdivision):
    """Mirror native G2 admission; checkpoint activation is not this workflow."""
    if mode in (None, "off"):
        return
    if mode != "union":
        raise ValueError("G2 residual anchors must be off or union")
    if lookahead is None or lookahead <= 0:
        raise ValueError("--g2-residual-anchors union requires --transfer-unreserved-lookahead")
    if publication not in (None, "ordered", "ready", "epoch"):
        raise ValueError("--g2-residual-anchors union requires ordered, ready or epoch publication")
    if subdivision:
        raise ValueError("--g2-residual-anchors union does not support physical Apply subdivision")


def parse_cpu_set(text: str) -> set[int]:
    """CPU IDs from a taskset-style list: "128-177", "0-3,8", "1,2,3"; no duplicates."""
    if not isinstance(text, str) or not text.strip():
        raise ValueError("CPU set must be a nonempty comma-separated list of IDs or ID ranges")
    cpus = set()
    for part in text.split(","):
        part = part.strip()
        if not part.isascii():
            raise ValueError(f"invalid CPU specification: {part!r}")
        lower, separator, upper = part.partition("-")
        lower = lower.strip()
        upper = upper.strip() if separator else lower
        if not lower.isdecimal() or not upper.isdecimal():
            raise ValueError(f"invalid CPU specification: {part!r}")
        first, last = int(lower), int(upper)
        if last < first:
            raise ValueError(f"reversed CPU range: {part!r}")
        for cpu in range(first, last + 1):
            if cpu in cpus:
                raise ValueError(f"duplicate CPU ID: {cpu}")
            cpus.add(cpu)
    return cpus


def format_cpu_set(cpus) -> str:
    return ",".join(map(str, sorted(cpus)))


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


def read_zfs_arc_bytes(proc_root=Path("/proc")) -> int | None:
    """Current ZFS ARC size (arcstats `size`); None without ZFS or when unreadable.

    The ARC is counted as used memory by the kernel and shrinks under anonymous
    pressure (down to c_min), so every resource receipt records it next to
    MemAvailable."""
    try:
        for line in (proc_root / "spl" / "kstat" / "zfs" / "arcstats").read_text().splitlines():
            fields = line.split()
            if len(fields) == 3 and fields[0] == "size":
                return int(fields[2])
    except (OSError, ValueError):
        return None
    return None


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
            "zfs_arc_bytes": read_zfs_arc_bytes(proc_root),
            "cgroup_remaining_bytes": min(cgroup_remaining) if cgroup_remaining else None,
            "cgroup_capacity_bytes": min(cgroup_limits) if cgroup_limits else None,
            "available_bytes": available}


def memory_admission(hard: int, soft: int | None, snapshot: dict, reserve: int | None,
                     margin_percent: float = 5.0):
    reserve = DEFAULT_HOST_MEMORY_RESERVE_BYTES if reserve is None else reserve
    admitted_hard = min(hard, snapshot["available_bytes"] - reserve)
    if admitted_hard <= 0:
        raise ValueError(f"host/cgroup MemAvailable {snapshot['available_bytes']} B leaves no campaign headroom "
                         f"above the {reserve} B floor (--host-memory-reserve-bytes)")
    admitted_soft = int(admitted_hard * (1 - margin_percent / 100)) if soft is None else min(soft, int(admitted_hard * (1 - margin_percent / 100)))
    if not 0 < admitted_soft < admitted_hard:
        raise ValueError("host/cgroup RAM allowance and margin must leave a positive soft limit below hard")
    return admitted_hard, admitted_soft, reserve


def memory_admission_record(requested: int, snapshot: dict, reserve: int, hard: int, soft: int) -> dict:
    """What admission did at this (re)start, for request/status/summary receipts."""
    return {"rule": ADMISSION_RULE, "requested_hard_memory_bytes": requested,
            "available_bytes": snapshot.get("available_bytes"),
            "host_available_bytes": snapshot.get("host_available_bytes"),
            "zfs_arc_bytes": snapshot.get("zfs_arc_bytes"),
            "host_memory_reserve_bytes": reserve,
            "effective_hard_memory_bytes": hard, "effective_soft_memory_bytes": soft,
            "hard_capped_by_available_memory": hard < requested}


def read_swap_in_pages(proc_root=Path("/proc")) -> int | None:
    """Host-wide pages swapped in since boot (/proc/vmstat pswpin); None if unreadable.

    Telemetry only: on a shared host it counts every user's swap-in."""
    try:
        for line in (proc_root / "vmstat").read_text().splitlines():
            name, _, value = line.partition(" ")
            if name == "pswpin":
                return int(value.strip())
    except (OSError, ValueError):
        return None
    return None


def process_swap_bytes(pid, proc_root=Path("/proc")) -> int | None:
    """Swapped-out bytes of one process (VmSwap in /proc/PID/status); None if unreadable."""
    try:
        for line in (proc_root / str(pid) / "status").read_text().splitlines():
            if line.startswith("VmSwap:"):
                return int(line.split()[1]) * 1024
    except (OSError, ValueError, IndexError):
        return None
    return None


def tree_swap_bytes(pids, proc_root=Path("/proc")) -> int | None:
    """VmSwap summed over the supervised tree's processes; None if none is readable."""
    values = [value for value in (process_swap_bytes(pid, proc_root) for pid in pids) if value is not None]
    return sum(values) if values else None


class RamGuard:
    """Host-aware RAM guard: cooperative save-and-stop and hard-stop decisions.

    Cooperative stops (the native saves its checkpoint and exits 4), first
    reason wins:
      - aggregate_rss_soft_limit: the supervised tree's RSS reaches the soft
        limit (the effective hard cap minus the guard margin);
      - host_memory_reserve: host/cgroup MemAvailable falls to the floor;
      - own_swap_growth_sustained: the supervised tree's swapped-out memory
        (VmSwap summed over its processes) grows at or above
        swap_growth_bytes_per_second in every sample interval for at least
        swap_growth_seconds (a missing reading or one slower interval resets
        it). This is the campaign's own pages being evicted, not host-wide
        swap-in, which on a shared host counts other users' processes and is
        only recorded (host_swap_in_bytes_per_second).
    Hard stops (SIGKILL of the owned native, unchanged): RSS reaches the hard
    cap, or MemAvailable falls to a quarter of the floor.
    Host-wide decisions (host_memory_reserve, host_memory_emergency) carry an
    attribution over the trailing window (attribution_seconds, default the
    swap window): own_memory_signal is true when the tree's RSS growth
    explains at least OWN_MEMORY_SHARE of the MemAvailable drop, or its own
    swap is growing at or above the stop rate.
    Pure state machine over readings; callers supply /proc values, so tests
    drive it with fake readers.
    """
    def __init__(self, hard_bytes, soft_bytes, floor_bytes,
                 swap_growth_bytes_per_second=DEFAULT_SWAP_GROWTH_STOP_BYTES_PER_SECOND,
                 swap_growth_seconds=DEFAULT_SWAP_GROWTH_STOP_SECONDS, page_bytes=None,
                 attribution_seconds=None):
        if not 0 < soft_bytes < hard_bytes or floor_bytes < 0:
            raise ValueError("RAM guard requires 0 < soft < hard and a nonnegative host floor")
        if (swap_growth_bytes_per_second < 0 or not math.isfinite(swap_growth_seconds)
                or swap_growth_seconds <= 0):
            raise ValueError("swap-growth guard requires a nonnegative rate and a positive finite window")
        self.hard_bytes, self.soft_bytes, self.floor_bytes = hard_bytes, soft_bytes, floor_bytes
        self.swap_growth_bytes_per_second = swap_growth_bytes_per_second
        self.swap_growth_seconds = swap_growth_seconds
        self.attribution_seconds = swap_growth_seconds if attribution_seconds is None else attribution_seconds
        self.page_bytes = os.sysconf("SC_PAGE_SIZE") if page_bytes is None else page_bytes
        self.last_swap = None
        self.high_since = None
        self.last_host_swap = None
        self.history = deque()

    def policy(self) -> dict:
        return {"soft_rss_bytes": self.soft_bytes, "hard_rss_bytes": self.hard_bytes,
                "host_available_floor_bytes": self.floor_bytes,
                "host_available_emergency_bytes": self.floor_bytes // 4,
                "swap_growth_stop_bytes_per_second": self.swap_growth_bytes_per_second,
                "swap_growth_stop_seconds": self.swap_growth_seconds,
                "swap_scope": "supervised_tree_vmswap_growth",
                "host_swap_in_scope": "telemetry_only_host_wide_proc_vmstat_pswpin",
                "host_stop_attribution": {"window_seconds": self.attribution_seconds,
                                          "own_rss_share_of_available_drop": OWN_MEMORY_SHARE},
                "cooperative": "save_and_stop_exit_4", "hard": "sigkill_owned_native"}

    def swap_growing(self, rate) -> bool:
        return rate is not None and self.swap_growth_bytes_per_second > 0 and rate >= self.swap_growth_bytes_per_second

    def observe(self, now: float, rss: int, available: int | None, own_swap_bytes: int | None,
                host_swap_in_pages: int | None = None) -> dict:
        rate = None
        if own_swap_bytes is not None and self.last_swap is not None and now > self.last_swap[0]:
            rate = max(0, own_swap_bytes - self.last_swap[1]) / (now - self.last_swap[0])
        previous = self.last_swap
        self.last_swap = None if own_swap_bytes is None else (now, own_swap_bytes)
        if self.swap_growing(rate):
            if self.high_since is None:
                self.high_since = previous[0]
        else:
            self.high_since = None
        sustained = 0.0 if self.high_since is None else now - self.high_since
        host_rate = None
        if (host_swap_in_pages is not None and self.last_host_swap is not None
                and now > self.last_host_swap[0]):
            host_rate = (max(0, host_swap_in_pages - self.last_host_swap[1]) * self.page_bytes
                         / (now - self.last_host_swap[0]))
        self.last_host_swap = None if host_swap_in_pages is None else (now, host_swap_in_pages)
        self.history.append((now, rss, available))
        while len(self.history) > 1 and self.history[1][0] <= now - self.attribution_seconds:
            self.history.popleft()
        cooperative = None
        if rss >= self.soft_bytes:
            cooperative = "aggregate_rss_soft_limit"
        elif available is not None and available <= self.floor_bytes:
            cooperative = "host_memory_reserve"
        elif self.high_since is not None and sustained >= self.swap_growth_seconds:
            cooperative = "own_swap_growth_sustained"
        hard = None
        if rss >= self.hard_bytes:
            hard = "aggregate_rss_hard_limit"
        elif available is not None and available <= self.floor_bytes // 4:
            hard = "host_memory_emergency"
        decision = {"cooperative": cooperative, "hard": hard, "own_swap_bytes": own_swap_bytes,
                    "own_swap_growth_bytes_per_second": rate, "own_swap_growth_sustained_seconds": sustained,
                    "host_swap_in_pages": host_swap_in_pages, "host_swap_in_bytes_per_second": host_rate}
        if cooperative in HOST_WIDE_STOPS or hard in HOST_WIDE_STOPS:
            decision["attribution"] = self.attribution(now, rss, available, rate)
        return decision

    def attribution(self, now, rss, available, rate) -> dict:
        """Who used the memory over the trailing window (host-wide decisions only)."""
        then, then_rss, then_available = self.history[0]
        growth = rss - then_rss
        drop = None if available is None or then_available is None else then_available - available
        share = growth / drop if drop is not None and drop > 0 else None
        swapping = self.swap_growing(rate)
        return {"window_seconds": now - then, "own_rss_growth_bytes": growth,
                "host_available_drop_bytes": drop, "own_share_of_available_drop": share,
                "own_swap_growing": swapping,
                "own_memory_signal": bool(swapping or (share is not None and share >= OWN_MEMORY_SHARE))}


# Resume-time frontier rescue (owner requirement 2026-09-28: a frontier with a
# known rescue never ends the campaign). On a native frontier stop the
# supervisor runs `rustred walk-rescue-plan`; a known class yields the next
# digest-chained amendment (or none when no physics query is blocked) and an
# automatic resume with every amendment; an unknown class, an exhausted rescue
# or the attempt bound stops and waits for the owner. Receipts: the run
# directory's rescue.json and the amendments directory's rescues.jsonl.
FRONTIER_STOP_REASON = "frontier_policy"
DEFAULT_MAX_RESCUES = 32
DEFAULT_HELPER_ID_PREFIX = "anchor"
RESCUE_LOG = "rescues.jsonl"
RESCUE_RESUME_VERDICTS = ("rescue", "no_amendment_needed")


def default_amendments_directory(checkpoint) -> Path:
    """Amendments live beside the checkpoint (never inside the native's directory)."""
    checkpoint = Path(checkpoint).resolve()
    return checkpoint.with_name(checkpoint.name + ".amendments")


def rescue_attempts(directory: Path) -> int:
    """Automatic resumes already taken (rescues.jsonl rows with action resume)."""
    log = directory / RESCUE_LOG
    if not log.is_file():
        return 0
    count = 0
    for line in log.read_text().splitlines():
        try:
            count += json.loads(line).get("action") == "resume"
        except (ValueError, AttributeError):
            continue
    return count


def native_frontier_stop(output: Path, status: int) -> bool:
    """Whether the native session paused on the A10 frontier stop (exit 4)."""
    return native_rescue_trigger(output, status) == "frontier_stop"


def native_rescue_trigger(output: Path, status: int):
    """Why a rescue plan is due after this native session, or None.

    "frontier_stop": paused on the A10 frontier stop. "drained_with_frontiers":
    the worklist drained (exit 4, status incomplete) with frontiers left, so a
    physics query may still be blocked (e.g. a taint that no later stop saw).
    """
    if status != 4:
        return None
    try:
        document = json.loads((output / "result.json").read_text())
    except (OSError, ValueError):
        return None
    cp6 = document.get("checkpoint", {})
    cp6 = isinstance(cp6, dict) and cp6.get("format") == "RUSTRED-WALK-CP6"
    if cp6:
        # CP6 keeps the complete state in its checkpoint, not this summary.
        # An unconfirmed/poisoned save must never trigger automatic amendment.
        if (not resumable_checkpoint(document["checkpoint"])
                or document.get("full_state_in_checkpoint") is not True
                or document.get("full_result_in_output_document") is not False):
            return None
        if document.get("status") == "stopped" and document.get("stop_reason") == "frontier_stop":
            return "frontier_stop"
    if document.get("status") == "paused" and document.get("stop_reason") == FRONTIER_STOP_REASON:
        return "frontier_stop"
    if (document.get("status") == "incomplete" and document.get("recursive_worklist_exhausted") is True
            and ((document.get("frontiers") or 0) > 0
                 or (cp6 and (document.get("input_frontiers_count") or 0) > 0))
            and document.get("error") is None):
        return "drained_with_frontiers"
    return None


def reconciled_rescue_trigger(output, status, policy, checkpoint):
    """Automatic CP6 resume also requires the final event/result reconciliation."""
    if policy == "epoch" and (not resumable_checkpoint(checkpoint)
                              or checkpoint.get("format") != "RUSTRED-WALK-CP6"):
        return None
    return native_rescue_trigger(output, status)


def plan_rescue(executable: Path, output: Path, amendments_directory: Path, helper_id_prefix: str,
                rescue_helpers, env, max_rescues: int, attempts: int, runner=subprocess.run,
                trigger: str = "frontier_stop", scope: str = "class", stop_requested=lambda: None) -> dict:
    """Classify the frontier stop of the run in `output` and prepare the next resume.

    Returns the rescue receipt: action "resume" (with the new amendment path,
    if any) or "wait_for_owner" with the reason. Never touches the checkpoint.
    """
    receipt = {"schema": "rustred.frontier-rescue-receipt.v1", "unix_time": time.time(),
               "run_directory": str(output), "trigger": trigger, "attempt": attempts + 1, "max_rescues": max_rescues,
               "helper_id_prefix": helper_id_prefix, "rescue_helpers": None if rescue_helpers is None else str(rescue_helpers),
               "family_closure_claim": False}
    if attempts >= max_rescues:
        receipt.update(action="wait_for_owner", reason=f"automatic rescue attempts exhausted ({attempts} of {max_rescues})")
        return receipt
    if rescue_cancelled(receipt, stop_requested):
        return receipt
    amendments_directory.mkdir(parents=True, exist_ok=True)
    pending = amendments_directory / ".pending-amendment.json"
    pending.unlink(missing_ok=True)
    plan_path = output / "rescue-plan.json"
    command = [str(executable), "walk-rescue-plan", "--command", str(output / "request.json"),
               "--helper-id-prefix", helper_id_prefix, "--output", str(plan_path),
               "--amendment-output", str(pending), "--rescue-scope", scope]
    if rescue_helpers is not None:
        command += ["--rescue-helpers", str(Path(rescue_helpers).resolve())]
    receipt["planner_command"] = command
    started = time.monotonic()
    completed = runner(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    receipt["planner_seconds"] = time.monotonic() - started
    receipt["planner_exit_status"] = completed.returncode
    if hasattr(completed, "planner_guard"):
        receipt["planner_guard"] = completed.planner_guard
    if rescue_cancelled(receipt, stop_requested):
        return receipt
    try:
        plan = json.loads(plan_path.read_text())
    except (OSError, ValueError) as error:
        receipt.update(action="wait_for_owner", reason=f"rescue planner produced no plan ({error}); "
                       f"stderr: {completed.stderr.strip()[-2000:]}")
        return receipt
    receipt["plan"] = {key: plan.get(key) for key in (
        "verdict", "reason", "frontier_nodes", "frontier_records", "classes", "tainted_nodes",
        "helper_roots_tainted", "physics_queries", "physics_blocked", "unknown_examples", "rescue_level",
        "rescue_scope", "superseded")}
    receipt["plan_path"] = str(plan_path)
    verdict = plan.get("verdict")
    # The CLI returns zero for all successful planning verdicts. A stale or
    # partially written plan must never authorize an unsuccessful child.
    if completed.returncode != 0:
        receipt.update(action="wait_for_owner", reason=f"rescue planner failed (exit {completed.returncode}, "
                       f"verdict {verdict}); stderr: {completed.stderr.strip()[-2000:]}")
        return receipt
    if trigger == "drained_with_frontiers" and verdict == "no_amendment_needed":
        # Drained: every physics query has an untainted (hence closed) root.
        receipt.update(action="complete", reason="drained; every physics query has an untainted containing "
                       "root; the remaining frontiers lie in quarantined helper cones")
        return receipt
    if verdict not in RESCUE_RESUME_VERDICTS:
        receipt.update(action="wait_for_owner",
                       reason=f"frontier rescue refused: {verdict}: {plan.get('reason')}")
        return receipt
    receipt["action"] = "resume"
    receipt["amendment"] = None
    if verdict == "rescue":
        amendment = plan["amendment"]
        text = pending.read_bytes()
        digest = hashlib.blake2b(text, digest_size=16).hexdigest()  # file identity for the receipt only
        final = amendments_directory / f"amendment-{amendment['sequence']:04d}.json"
        if rescue_cancelled(receipt, stop_requested):
            return receipt
        if final.exists():
            if final.read_bytes() != text:
                receipt.update(action="wait_for_owner",
                               reason=f"{final} already exists with other content; refusing to rewrite an amendment")
                return receipt
            pending.unlink()
        else:
            os.replace(pending, final)
            final.chmod(0o444)
        receipt["amendment"] = {"path": str(final), "sequence": amendment["sequence"],
                                "digest_blake3": amendment["digest"], "parent": amendment["parent"],
                                "queries": amendment["queries"], "file_blake2b_128": digest,
                                "helpers": amendment.get("helpers")}
    return receipt


def rescue_cancelled(receipt, stop_requested):
    """Recheck cancellation at the planner/publication/resume handoffs."""
    reason = stop_requested()
    if reason is not None:
        receipt.update(action="wait_for_owner", reason=f"rescue planning stopped: {reason}")
        return True
    return False


def guarded_rescue_planner(command, *, env, output, args, cpus, collector,
                           request_stop, stop_requested, child_address_space=None,
                           sample_observer=lambda sample: None, **_):
    """Reuse the native ownership/RAM guards for the read-only planner.

    Unlike the walk, the planner has no cooperative checkpoint interface:
    terminate its owned group on any stop, then allow at most five seconds
    before killing it. The already durable native checkpoint is untouched.
    File-backed output avoids both pipe deadlock and unbounded capture RAM.
    """
    completed = subprocess.CompletedProcess(command, 1, "", "")
    details = completed.planner_guard = {"stop_reason": None, "peak_observed_aggregate_rss_bytes": 0}
    started = time.monotonic()
    stderr_path = output / "planner.stderr"
    try:
        if stop_requested() is not None:
            return completed
        initial_host = host_memory()
        hard, soft, floor = memory_admission(args.max_memory_bytes, args.soft_memory_bytes,
            initial_host, args.host_memory_reserve_bytes, args.ram_guard_margin_percent)
        guard = RamGuard(hard, soft, floor, args.swap_growth_stop_bytes_per_second,
                         args.swap_growth_stop_seconds)
        details["memory_admission"] = memory_admission_record(args.max_memory_bytes, initial_host, floor, hard, soft)
        details["policy"] = dict(guard.policy(), cooperative="sigterm_owned_read_only_planner",
                                 hard="sigkill_owned_read_only_planner")
        with (output / "planner.stdout").open("x") as stdout, stderr_path.open("x") as stderr, \
                (output / "planner-resources.jsonl").open("x") as resources, owned_process(
                    command, env, cpus, request_stop, child_address_space, stdout, stderr) as child:
            details["pid"] = child.pid
            try:
                collector.register(child.pid)
            except (OSError, ValueError, IndexError, StopIteration):
                if child.poll() is None:
                    raise
            details["start_ticks"] = collector.identities.get(child.pid)
            while True:
                reason = stop_requested()
                tree, collection = collector.sample()
                rss = sum(row["rss_bytes"] for row in tree.values())
                now = time.monotonic()
                try:
                    host = host_memory()
                except (OSError, ValueError):
                    host = {"available_bytes": None}
                    reason = reason or "host_memory_monitor_unavailable"
                decision = guard.observe(now, rss, host["available_bytes"], tree_swap_bytes(tree), read_swap_in_pages())
                reason = reason or decision["cooperative"] or decision["hard"]
                details["peak_observed_aggregate_rss_bytes"] = max(details["peak_observed_aggregate_rss_bytes"], rss)
                sample = {"phase": "rescue_planning", "elapsed_seconds": now-started, "aggregate_rss_bytes": rss,
                    "peak_observed_rss_bytes": details["peak_observed_aggregate_rss_bytes"],
                    "available_bytes": host["available_bytes"], "decision": decision, "collection": collection,
                    "planner_pid": child.pid, "planner_guard_policy": details["policy"], "cancel_reason": reason}
                append_record(resources, sample)
                sample_observer(sample)
                if reason is not None:
                    request_stop(reason)
                    details["stop_reason"] = stop_requested() or reason
                    if child.poll() is None:
                        try:
                            os.killpg(child.pid, signal.SIGKILL if decision["hard"] else signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                        try:
                            child.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            try:
                                os.killpg(child.pid, signal.SIGKILL)
                            except ProcessLookupError:
                                pass
                            child.wait()
                    break
                if child.poll() is not None:
                    break
                time.sleep(args.sample_seconds)
            completed.returncode = child.wait()
    except (OSError, ValueError) as error:
        completed.stderr = str(error)
        request_stop("rescue_planner_supervision_failure")
        details["stop_reason"] = stop_requested()
    finally:
        details["elapsed_seconds"] = time.monotonic()-started
        if stderr_path.is_file():
            with stderr_path.open("rb") as stderr:
                stderr.seek(max(0, stderr_path.stat().st_size-2000))
                completed.stderr += stderr.read().decode("utf-8", errors="replace")
    return completed


def record_rescue(amendments_directory: Path, output: Path, receipt: dict) -> None:
    """Write the run's rescue receipt and append it to the campaign rescue log."""
    (output / "rescue.json").write_text(json.dumps(receipt, indent=2) + "\n")
    amendments_directory.mkdir(parents=True, exist_ok=True)
    with (amendments_directory / RESCUE_LOG).open("a") as log:
        log.write(json.dumps(receipt, sort_keys=True) + "\n")
        log.flush()
        os.fsync(log.fileno())


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
    parser.add_argument("--cpus", help="permitted CPU IDs as a comma list or ranges (128-177, 0-3,8); exactly --workers IDs")
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
    parser.add_argument("--host-memory-reserve-bytes", "--host-available-floor-bytes",
                        dest="host_memory_reserve_bytes", type=positive,
                        help="host MemAvailable floor: the cooperative save-and-stop fires when host (or "
                             "enclosing cgroup) MemAvailable falls to it, the hard stop at a quarter of it, and "
                             "admission leaves it free; default "
                             f"{DEFAULT_HOST_MEMORY_RESERVE_BYTES} (50 GB)")
    parser.add_argument("--swap-growth-stop-bytes-per-second", type=nonnegative,
                        default=DEFAULT_SWAP_GROWTH_STOP_BYTES_PER_SECOND,
                        help="cooperative save-and-stop when the supervised tree's own swapped-out memory "
                             "(VmSwap summed over its processes) grows at or above this rate for "
                             "--swap-growth-stop-seconds; 0 disables (default: 32 MiB/s). Host-wide "
                             "swap-in is recorded only")
    parser.add_argument("--swap-growth-stop-seconds", type=float, default=DEFAULT_SWAP_GROWTH_STOP_SECONDS,
                        help="window of sustained own swap growth before the cooperative stop (default: 120)")
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
    parser.add_argument("--" + DOMAIN.EPOCH_INSPECTOR_LOOKUP, choices=DOMAIN.EPOCH_INSPECTOR_LOOKUP_MODES,
                        action=DOMAIN.StoreOnce,
                        help="CP6 Epoch comparison control; default all-miss; resume must retain its mode")
    parser.add_argument("--" + DOMAIN.EPOCH_ROLLING, action=DOMAIN.StoreTrueOnce, nargs=0, default=False,
                        help="opt into bounded rolling CP6 execution; frozen on resume")
    parser.add_argument("--" + DOMAIN.EPOCH_DISPATCH, choices=DOMAIN.EPOCH_DISPATCH_POLICIES,
                        action=DOMAIN.StoreOnce, help="pending-job dispatch; adaptive requires rolling CP6")
    DOMAIN.add_epoch_batch_arguments(parser)
    DOMAIN.add_epoch_preparation_arguments(parser)
    parser.add_argument("--" + G2_RESIDUAL_ANCHORS, choices=G2_RESIDUAL_MODES, action=DOMAIN.StoreOnce,
                        help="fresh symbolic walk opt-in (default off); union requires unreserved delegation, "
                             "ordered/ready publication and no physical subdivision; resume must retain its original mode")
    parser.add_argument("--" + DOMAIN.INSPECTION_WORKERS, type=DOMAIN.positive, action=DOMAIN.StoreOnce,
                        help="explicit symbolic compute partition: N inspectors, workers-1-N admission helpers and one coordinator; requires --queries")
    parser.add_argument("--" + DOMAIN.FRONTIER_POLICY, choices=DOMAIN.FRONTIER_POLICIES, action=DOMAIN.StoreOnce,
                        help="record (native default) keeps walking past explicit frontiers; stop saves the checkpoint "
                             "and stops at the first new frontier (exit 4); requires --queries and a checkpoint")
    parser.add_argument("--" + DOMAIN.APPLICATION_REFINEMENT, type=DOMAIN.application_cardinality, action=DOMAIN.StoreOnce,
                        help="opt into one finite selected Apply-cell axis singleton refinement; positive cardinality, default off, not a cumulative work cap; requires --queries")
    parser.add_argument("--route-domain-overcover", action="store_true",
                        help="share admitted symbolic route covers; requires --queries")
    parser.add_argument("--" + DOMAIN.JOINT_SUPPORT_PRUNING, action=DOMAIN.StoreTrueOnce, nargs=0, default=False,
                        help="opt into joint source-support mask pruning; requires --queries and route overcover; default off")
    parser.add_argument("--amend-queries", type=Path, action="append", default=[],
                        help="resume-time rescue amendment (repeatable, chain order; requires --resume and --queries): "
                             "append-only, digest-chained protected queries")
    parser.add_argument("--auto-rescue", action="store_true",
                        help="on a native frontier stop, classify the frontiers (rustred walk-rescue-plan) and, for a "
                             "known class, write the next amendment and resume automatically; an unknown class stops "
                             "and waits for the owner; requires --frontier-policy stop and a checkpoint")
    parser.add_argument("--rescue-helpers", type=Path,
                        help="optional query document of preferred rescue helpers (e.g. plan-v3); requires --auto-rescue")
    parser.add_argument("--helper-id-prefix", default=DEFAULT_HELPER_ID_PREFIX,
                        help=f"cosmetic prefix for new auxiliary IDs (default: {DEFAULT_HELPER_ID_PREFIX}); "
                             "scope comes only from the immutable query_roles declaration")
    parser.add_argument("--rescue-scope", choices=("class", "tainted"), default="class",
                        help="class (default): a known-class frontier supersedes every open helper root unbounded in "
                             "that class's dimension and re-covers its physics queries with bounded helpers (no "
                             "cascade); tainted: re-cover only the physics queries whose roots reach the frontier")
    parser.add_argument("--max-rescues", type=positive, default=DEFAULT_MAX_RESCUES,
                        help=f"automatic rescue resumes per campaign (default: {DEFAULT_MAX_RESCUES}); counted in the "
                             "amendments directory")
    parser.add_argument("--amendments-directory", type=Path,
                        help="where rescue amendments and rescues.jsonl live (default: <checkpoint>.amendments)")
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
    if not symbolic and (args.route_domain_overcover or args.route_joint_source_support_pruning or args.reuse_initial_d_bands
                        or args.g2_residual_anchors is not None or any(
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
    try:
        DOMAIN.validate_epoch_inspector_lookup(args.epoch_inspector_lookup, symbolic,
                                               args.publication_policy, args.checkpoint is not None or args.resume is not None)
        DOMAIN.validate_epoch_rolling(args.epoch_rolling, symbolic,
                                      args.publication_policy, args.checkpoint is not None or args.resume is not None)
        DOMAIN.validate_epoch_dispatch(args.epoch_dispatch, args.epoch_rolling, symbolic,
                                       args.publication_policy, args.checkpoint is not None or args.resume is not None)
        DOMAIN.validate_epoch_batch(args.epoch_publication_order, args.epoch_cut_size, args.epoch_window,
                                    args.epoch_rolling, symbolic, args.publication_policy,
                                    args.checkpoint is not None or args.resume is not None,
                                    args.epoch_result_escrow_jobs, args.epoch_result_escrow_bytes)
        DOMAIN.validate_epoch_preparation(vars(args), symbolic, args.publication_policy,
                                          args.workers, args.inspection_workers, args.max_containment_checks)
        validate_g2_residual_anchors(args.g2_residual_anchors, args.transfer_unreserved_lookahead,
                                     args.publication_policy, args.apply_subdivision_axis is not None)
    except ValueError as error:
        parser.error(str(error))
    # Omit Off from both native argv and generated restart argv, including
    # when explicitly selected, so historical flag-off invocations stay exact.
    if args.g2_residual_anchors == "off":
        args.g2_residual_anchors = None
    DOMAIN.validate_publication_policy(parser, args.publication_policy, args.transfer_unreserved_lookahead,
                                       args.checkpoint is not None or args.resume is not None,
                                       args.apply_subdivision_axis is not None)
    DOMAIN.validate_frontier_policy(parser, args.frontier_policy,
                                    args.checkpoint is not None or args.resume is not None)
    if args.amend_queries and (not symbolic or args.resume is None):
        parser.error("--amend-queries requires --queries and --resume")
    if args.auto_rescue and (args.frontier_policy != "stop" or not symbolic
                             or (args.checkpoint is None and args.resume is None)):
        parser.error("--auto-rescue requires --queries, --frontier-policy stop and --checkpoint or --resume")
    if args.rescue_helpers is not None and not args.auto_rescue:
        parser.error("--rescue-helpers requires --auto-rescue")
    if not args.helper_id_prefix:
        parser.error("--helper-id-prefix must be nonempty")
    for path in [*args.amend_queries, *([args.rescue_helpers] if args.rescue_helpers else [])]:
        if not path.is_file():
            parser.error(f"not a file: {path}")
    if not math.isfinite(args.swap_growth_stop_seconds) or args.swap_growth_stop_seconds <= 0:
        parser.error("swap-growth stop window must be positive and finite")
    affinity = os.sched_getaffinity(0)
    worker_cap = min(MAX_WORKERS, len(affinity))
    if not 1 <= args.workers <= MAX_WORKERS or args.other_workers < 0 or args.workers + args.other_workers > worker_cap:
        parser.error(f"aggregate configured compute workers must be between 1 and {MAX_WORKERS}"
                     f" and within the {len(affinity)} permitted CPUs")
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
    try:
        cpus = parse_cpu_set(args.cpus) if args.cpus else set(sorted(affinity)[:args.workers])
    except ValueError as error:
        parser.error(str(error))
    if len(cpus) != args.workers or not cpus <= affinity:
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
        guard = RamGuard(effective_hard, effective_soft, host_reserve, args.swap_growth_stop_bytes_per_second,
                         args.swap_growth_stop_seconds)
        admission = memory_admission_record(args.max_memory_bytes, initial_host, host_reserve,
                                            effective_hard, effective_soft)
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
    if args.auto_rescue or args.amend_queries:
        try:
            ROLES.query_roles(ROLES.loads_document(args.queries.read_text()), require_explicit=True)
        except (OSError, ValueError, KeyError, TypeError) as error:
            parser.error(str(error))
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
        if args.g2_residual_anchors == "union":
            command += ["--" + G2_RESIDUAL_ANCHORS, "union"]
        if args.epoch_rolling:
            command.append("--" + DOMAIN.EPOCH_ROLLING)
        if args.epoch_dispatch == "adaptive":
            command += ["--" + DOMAIN.EPOCH_DISPATCH, "adaptive"]
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
        for amendment in args.amend_queries:
            command += ["--amend-queries", str(amendment.resolve())]
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
    epoch_policy = ({"epoch_inspector_lookup": args.epoch_inspector_lookup or "all-miss",
                     "epoch_rolling": args.epoch_rolling, "epoch_dispatch": args.epoch_dispatch or "fifo",
                     **{name.replace("-", "_"): getattr(args, name.replace("-", "_"))
                        for name in DOMAIN.EPOCH_DATA_OPTIONS
                        if getattr(args, name.replace("-", "_")) is not None}}
                    if symbolic and args.publication_policy == "epoch" else {})
    checkpoint_directory = str(checkpoint_directory.resolve()) if checkpoint_directory is not None else None
    amendments_directory = None
    if args.auto_rescue:
        amendments_directory = (args.amendments_directory.resolve() if args.amendments_directory is not None
                                else default_amendments_directory(checkpoint_directory))
    # Never include the process environment or license in provenance.
    (output / "request.json").write_text(json.dumps({
        **epoch_policy,
        "command": command, "cpus": sorted(cpus), "registered_roots": collector.identities,
        "input_scope": "symbolic_domains" if symbolic else "concrete_targets",
        "reuse_initial_d_bands": args.reuse_initial_d_bands,
        "publication_policy": (args.publication_policy or "ordered") if symbolic else None,
        "g2_residual_anchors": (args.g2_residual_anchors or "off") if symbolic else None,
        "requested_inspection_workers": args.inspection_workers,
        "requested_max_queries": args.max_queries,
        "requested_max_query_bytes": args.max_query_bytes,
        "frontier_policy": (args.frontier_policy or "record") if symbolic else None,
        "ram_guard": guard.policy(),
        "workers": args.workers, "other_workers": args.other_workers,
        "hard_memory_bytes": args.max_memory_bytes, "soft_memory_bytes": args.soft_memory_bytes,
        "effective_hard_memory_bytes": effective_hard, "effective_soft_memory_bytes": effective_soft,
        "host_memory_reserve_bytes": host_reserve, "initial_host_memory": initial_host,
        "memory_admission": admission,
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
        "amend_queries": [str(path.resolve()) for path in args.amend_queries],
        "auto_rescue": None if not args.auto_rescue else {
            "helper_id_prefix": args.helper_id_prefix, "max_rescues": args.max_rescues,
            "rescue_scope": args.rescue_scope,
            "rescue_helpers": None if args.rescue_helpers is None else str(args.rescue_helpers.resolve()),
            "amendments_directory": str(amendments_directory) if amendments_directory else None},
        "work_checkpoint": False, "family_closure_claim": False,
    }, indent=2) + "\n")
    env = dict(os.environ)
    for name in DIAGNOSTIC_ONLY_ENVIRONMENT:
        env.pop(name, None)
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
    ram_guard_stop = None
    if admission["hard_capped_by_available_memory"]:
        print(f"RAM admission: effective hard cap {effective_hard} B = available {initial_host['available_bytes']} B"
              f" - floor {host_reserve} B (requested {args.max_memory_bytes} B); soft {effective_soft} B",
              file=sys.stderr, flush=True)
    started = time.monotonic()
    tail = MONITOR.EventTail(output / "events.jsonl")
    # Derived rates over the last hour of native heartbeats (two hours retained);
    # an additive status block, never an ETA.
    metrics = HEARTBEAT.HeartbeatWindow()
    tail.observers.append(metrics.observe)
    presenter = MONITOR.Presenter(enabled=not args.no_progress,
                                  plain_seconds=args.plain_progress_seconds)
    telemetry = MONITOR.TELEMETRY.TelemetryStream(output / "telemetry.jsonl")
    last_checkpoint = {"state": "awaiting_first_save", "directory": checkpoint_directory} if checkpoint_directory else None
    checkpoint_sizes = MONITOR.CheckpointSizeCache()
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
        if progress.get("query_admission") is None:
            progress["query_admission"] = tail.query_admission
        for candidate in (progress["checkpoint"], tail.saved_checkpoint):
            last_checkpoint = advance_checkpoint(last_checkpoint, candidate)
        last_checkpoint = checkpoint_sizes.enrich(last_checkpoint)
        writings = [candidate for candidate in (progress.get("checkpoint_write"), tail.checkpoint_write)
                    if candidate is not None and isinstance(MONITOR.number(candidate.get("generation")), int)]
        writing = max(writings, key=lambda candidate: candidate["generation"], default=None)
        if writing and last_checkpoint and writing["generation"] <= (last_checkpoint.get("generation") or 0):
            writing = None
        if exit_status is not None:
            writing = None
        status = {"schema": "rustred.campaign-status.v1", "heartbeat_unix_time": time.time(),
                  **epoch_policy,
                  "heartbeat_age_seconds": 0.0, "heartbeat_stale": False,
                  "state": state, "elapsed_seconds": now-started, "sample_seconds": args.sample_seconds,
                  "run_directory": str(output), "process_identity": identities,
                  "workers": args.workers, "other_workers": args.other_workers,
                  "hard_memory_bytes": effective_hard, "soft_memory_bytes": effective_soft,
                  "requested_hard_memory_bytes": args.max_memory_bytes,
                  "host_memory_reserve_bytes": host_reserve, "memory_admission": admission,
                  "ram_guard_margin_percent": args.ram_guard_margin_percent,
                  "ram_guard": guard.policy(), "ram_guard_stop": ram_guard_stop,
                  "native_stop_reason": progress.get("native_stop_reason"),
                  "progress": progress, "resources": resources, "checkpoint": last_checkpoint,
                  "checkpoint_write": writing, "derived": metrics.derived(),
                  "checkpoint_milestones": list(tail.milestones),
                  "resume_command": resume_command,
                  "stop_reason": stop_reason, "exit_status": exit_status,
                  "event_reader": tail.diagnostics(), "event_read_error": read_error,
                  "hard_timeout_seconds": None, "closure_eta_seconds": None,
                  "native_stderr": str(output / "native.stderr"), "family_closure_claim": False}
        frame = telemetry.emit(status)
        status["telemetry_stream"] = telemetry.diagnostics()
        MONITOR.atomic_json(output / "status.json", status)
        try:
            presenter.render_frame(frame, force=exit_status is not None)
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
        min_available = max_arc = None
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
                value = host.get("host_available_bytes")
                if value is not None:
                    min_available = value if min_available is None else min(min_available, value)
                value = host.get("zfs_arc_bytes")
                if value is not None:
                    max_arc = value if max_arc is None else max(max_arc, value)
                decision = guard.observe(now, rss, available, tree_swap_bytes(tree), read_swap_in_pages())

                def guard_record(reason):
                    return {"reason": reason, "elapsed_seconds": now-started, "aggregate_rss_bytes": rss,
                            "available_bytes": available,
                            "own_swap_growth_bytes_per_second": decision["own_swap_growth_bytes_per_second"],
                            "own_swap_growth_sustained_seconds": decision["own_swap_growth_sustained_seconds"],
                            "host_wide": reason in HOST_WIDE_STOPS, **decision.get("attribution", {})}
                if decision["cooperative"]:
                    request_stop(decision["cooperative"])
                    # The decision that set the stop reason (not an operator stop).
                    if ram_guard_stop is None and stop_reason == decision["cooperative"]:
                        ram_guard_stop = guard_record(decision["cooperative"])
                native_cpu = next((row["observed_busy_cores"] for row in process_rows
                                   if row["role"] == "owned_native"), None)
                last_resources = {"event": "resources", "elapsed_seconds": now-started,
                    "aggregate_rss_bytes": rss, "peak_observed_rss_bytes": peak,
                    "sampled_cpu_seconds_since_start": cumulative_cpu,
                    "cpu_sample_interval_seconds": sample_interval,
                    "observed_busy_cores": None if sample_interval is None else delta_cpu/sample_interval,
                    "native_busy_cores": native_cpu, **host,
                    "zfs_arc_bytes": host.get("zfs_arc_bytes"),
                    "own_swap_bytes": decision["own_swap_bytes"],
                    "own_swap_growth_bytes_per_second": decision["own_swap_growth_bytes_per_second"],
                    "own_swap_growth_sustained_seconds": decision["own_swap_growth_sustained_seconds"],
                    "host_swap_in_pages": decision["host_swap_in_pages"],
                    "host_swap_in_bytes_per_second": decision["host_swap_in_bytes_per_second"],
                    "ram_guard_attribution": decision.get("attribution"),
                    "native_swap_bytes": process_swap_bytes(child.pid),
                    "rss_growth_bytes_per_second": None if previous_rss is None else (rss-previous_rss)/(now-last_time),
                    "processes": len(tree), "process_cpu": process_rows, "collection": collection,
                    "configured_workers": args.workers+args.other_workers,
                    "objective_hours": args.objective_hours, "past_objective": now-started > args.objective_hours*3600,
                    "cancel_reason": stop_reason, "registered_scope_only": True}
                append_record(resources, last_resources)
                last_time = now
                previous_rss = rss
                if decision["hard"] and not hard_stopped:
                    # Other registered jobs are measured, never signalled.
                    if child.pid in tree and tree[child.pid]["start"] == collector.identities.get(child.pid):
                        request_stop(decision["hard"])
                        ram_guard_stop = dict(ram_guard_stop or {}, hard_stop=guard_record(decision["hard"]))
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
    if terminal_progress.get("query_admission") is None:
        terminal_progress["query_admission"] = tail.query_admission
    for candidate in (terminal_progress["checkpoint"], tail.saved_checkpoint):
        last_checkpoint = advance_checkpoint(last_checkpoint, candidate)
    last_checkpoint = terminal_checkpoint(args.publication_policy, status, tail.latest, last_checkpoint)
    if resumable_checkpoint(last_checkpoint) and checkpoint_directory:
        resume_command = restart_command(args, output, checkpoint_directory, cpus)
    state = terminal_state(status, args.publication_policy, tail.latest, terminal_progress,
                           last_checkpoint, stop_reason)
    publish_status(last_resources, state, status)
    (output / "run.status").write_text(str(status) + "\n")
    (output / "supervisor-result.json").write_text(json.dumps({
        **epoch_policy,
        "exit_status": status, "elapsed_seconds": time.monotonic()-started,
        "reuse_initial_d_bands": args.reuse_initial_d_bands,
        "publication_policy": (args.publication_policy or "ordered") if symbolic else None,
        "g2_residual_anchors": (args.g2_residual_anchors or "off") if symbolic else None,
        "requested_inspection_workers": args.inspection_workers,
        "requested_max_queries": args.max_queries,
        "requested_max_query_bytes": args.max_query_bytes,
        "peak_observed_aggregate_rss_bytes": peak, "operator_or_resource_stop": stop_reason,
        "native_stop_reason": terminal_progress.get("native_stop_reason"),
        "frontier_policy": (args.frontier_policy or "record") if symbolic else None,
        "ram_guard": guard.policy(),
        "initial_host_memory": initial_host,
        "last_host_available_bytes": last_resources.get("host_available_bytes"),
        "last_zfs_arc_bytes": last_resources.get("zfs_arc_bytes"),
        "last_own_swap_growth_bytes_per_second": last_resources.get("own_swap_growth_bytes_per_second"),
        "last_host_swap_in_bytes_per_second": last_resources.get("host_swap_in_bytes_per_second"),
        "ram_guard_stop": ram_guard_stop,
        "min_observed_host_available_bytes": min_available,
        "max_observed_zfs_arc_bytes": max_arc,
        "hard_stopped": hard_stopped,
        "work_checkpoint": bool(resumable_checkpoint(last_checkpoint)
                                and not last_checkpoint.get("bootstrap", False)),
        "resume_checkpoint_available": resumable_checkpoint(last_checkpoint),
        "checkpoint": last_checkpoint, "state": state, "resume_requested": args.resume is not None,
        "resume_command": resume_command,
        "unbounded_work": args.unbounded_work, "family_closure_claim": False,
        "child_rlimit_as_bytes": child_as, "memory_failure_is_incomplete": True,
        "requested_hard_memory_bytes": args.max_memory_bytes,
        "effective_hard_memory_bytes": effective_hard, "effective_soft_memory_bytes": effective_soft,
        "host_memory_reserve_bytes": host_reserve, "memory_admission": admission,
        "ram_guard_margin_percent": args.ram_guard_margin_percent,
        "rust_result_present": (output / "result.json").is_file(),
    }, indent=2) + "\n")
    # Frontier rescue: a frontier stop with a known rescue is a pause, never
    # the end of the campaign (owner requirement 2026-09-28).
    trigger = (reconciled_rescue_trigger(output, status, args.publication_policy, last_checkpoint)
               if args.auto_rescue else None)
    if trigger and stop_reason is None and not hard_stopped and checkpoint_directory:
        planner_resources = {"phase": "rescue_planning"}

        def planner_status(sample):
            nonlocal planner_resources
            planner_resources = sample
            # The resource heartbeat is live; native progress remains the
            # correctly aged, already drained walk snapshot.
            publish_status(sample, "rescue_planning")

        def rescue_stop_requested():
            if stop_file.exists() and stop_reason is None:
                request_stop("existing_operator_stop_file")
            return stop_reason

        def run_planner(command, **kwargs):
            return guarded_rescue_planner(command, **kwargs, output=output, args=args, cpus=cpus,
                collector=collector, request_stop=request_stop, stop_requested=rescue_stop_requested,
                child_address_space=child_as, sample_observer=planner_status)

        print("Frontier rescue: planning from the durable checkpoint under CPU/RAM guards.", flush=True)
        planner_status(planner_resources)
        receipt = plan_rescue(args.executable.resolve(), output, amendments_directory, args.helper_id_prefix,
                              args.rescue_helpers, env, args.max_rescues, rescue_attempts(amendments_directory),
                              runner=run_planner, trigger=trigger, scope=args.rescue_scope,
                              stop_requested=rescue_stop_requested)
        rescue_cancelled(receipt, rescue_stop_requested)
        publish_status(planner_resources, terminal_state(status, args.publication_policy, tail.latest,
                       terminal_progress, last_checkpoint, stop_reason), status)
        if receipt["action"] == "complete":
            record_rescue(amendments_directory, output, receipt)
            print("Frontier rescue: " + receipt["reason"], flush=True)
        elif receipt["action"] == "resume":
            if receipt["amendment"] is not None:
                args.amend_queries = [*args.amend_queries, Path(receipt["amendment"]["path"])]
            restart = restart_command(args, output, checkpoint_directory, cpus)
            receipt["resume_command"] = restart
            record_rescue(amendments_directory, output, receipt)
            print(f"Frontier stop rescued automatically (attempt {receipt['attempt']}, "
                  f"{receipt['plan']['verdict']}): {receipt['plan']['reason']}", flush=True)
            if receipt["amendment"] is not None:
                print(f"Amendment {receipt['amendment']['sequence']}: {receipt['amendment']['path']}", flush=True)
            print("Resuming: " + shlex.join(restart), flush=True)
            sys.stdout.flush()
            sys.stderr.flush()
            if rescue_cancelled(receipt, rescue_stop_requested):
                # Preserve the earlier resume intent and the subsequent abort;
                # an interrupted handoff conservatively consumes one attempt.
                record_rescue(amendments_directory, output, receipt)
                publish_status(planner_resources, terminal_state(status, args.publication_policy, tail.latest,
                               terminal_progress, last_checkpoint, stop_reason), status)
                print("FRONTIER RESCUE: automatic resume cancelled; WAITS FOR THE OWNER.", file=sys.stderr, flush=True)
            else:
                os.execv(restart[0], restart)
        else:
            record_rescue(amendments_directory, output, receipt)
            print("FRONTIER RESCUE: the campaign is paused and WAITS FOR THE OWNER. " + receipt["reason"],
                  file=sys.stderr, flush=True)
            print(f"Rescue receipt: {output / 'rescue.json'}; plan: {receipt.get('plan_path')}",
                  file=sys.stderr, flush=True)
    if resume_command:
        print(f"Durable checkpoint: {checkpoint_directory}", flush=True)
        print("Resume with fresh receipts: " + shlex.join(resume_command), flush=True)
    return status if status >= 0 else 128-status


if __name__ == "__main__":
    raise SystemExit(main())
