#!/usr/bin/env python3
"""Bounded live status and terminal presentation for a saved-owner campaign.

This module observes receipts only. It cannot resume work or certify closure.
"""
from __future__ import annotations

import argparse
from collections import deque
from datetime import datetime, timezone
import json
import math
import os
from pathlib import Path
import shutil
import sys
import tempfile
import time

MAX_RECORD_BYTES = 1024 * 1024
MAX_POLL_BYTES = 4 * MAX_RECORD_BYTES


def atomic_json(path: Path, value: dict) -> None:
    """Publish one complete status generation, never a partial JSON document."""
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix="." + path.name + ".", delete=False) as stream:
            temporary = Path(stream.name)
            json.dump(value, stream, sort_keys=True, allow_nan=False)
            stream.write("\n")
        os.replace(temporary, path)
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


class EventTail:
    """Read complete JSONL records with bounded memory and per-poll I/O."""
    def __init__(self, path: Path):
        self.path = path
        self.identity = None
        self.offset = 0
        self.pending = b""
        self.discarding = False
        self.latest = {}
        self.observed_at = None
        self.invalid_records = 0
        self.oversized_records = 0
        self.rotations = 0
        self.milestones = deque(maxlen=64)
        self.milestone_keys = deque(maxlen=128)
        self.milestone_count = 0
        self.saved_checkpoint = None
        self.checkpoint_write = None

    def poll(self, now: float | None = None) -> dict:
        now = time.monotonic() if now is None else now
        try:
            with self.path.open("rb") as stream:
                info = os.fstat(stream.fileno())
                identity = (info.st_dev, info.st_ino)
                if self.identity is not None and (identity != self.identity or info.st_size < self.offset):
                    self.offset = 0
                    self.pending = b""
                    self.discarding = False
                    self.rotations += 1
                self.identity = identity
                stream.seek(self.offset)
                remaining = MAX_POLL_BYTES
                while remaining:
                    chunk = stream.read(min(65536, remaining))
                    if not chunk:
                        break
                    self.offset += len(chunk)
                    remaining -= len(chunk)
                    for index, piece in enumerate(chunk.split(b"\n")):
                        if index:
                            self._finish(now)
                        if self.discarding:
                            continue
                        if len(self.pending) + len(piece) > MAX_RECORD_BYTES:
                            self.pending = b""
                            self.discarding = True
                            self.oversized_records += 1
                        else:
                            self.pending += piece
        except FileNotFoundError:
            pass
        return self.latest

    def _finish(self, now):
        if self.discarding:
            self.discarding = False
        elif self.pending:
            try:
                value = json.loads(self.pending)
                if not isinstance(value, dict):
                    raise ValueError("event must be an object")
                self.latest = value
                self.observed_at = now
                self._checkpoint_milestone(value)
            except (ValueError, UnicodeError, RecursionError):
                self.invalid_records += 1
        self.pending = b""

    def _checkpoint_milestone(self, value):
        progress = value.get("progress", value)
        if not isinstance(progress, dict):
            return
        kind = progress.get("event")
        if kind not in ("checkpoint_started", "checkpoint_saved"):
            return
        checkpoint = progress.get("checkpoint_write" if kind == "checkpoint_started" else "checkpoint")
        if not isinstance(checkpoint, dict):
            return
        generation = checkpoint.get("generation")
        if not isinstance(generation, int) or isinstance(generation, bool) or generation < 0:
            return
        if kind == "checkpoint_saved":
            if self.saved_checkpoint is None or checkpoint.get("generation", 0) >= self.saved_checkpoint.get("generation", 0):
                self.saved_checkpoint = checkpoint
            if self.checkpoint_write and self.checkpoint_write.get("generation", 0) <= checkpoint.get("generation", 0):
                self.checkpoint_write = None
        elif self.saved_checkpoint is None or checkpoint.get("generation", 0) > self.saved_checkpoint.get("generation", 0):
            self.checkpoint_write = checkpoint
        key = (kind, checkpoint.get("generation"), checkpoint.get("state_path"))
        if key in self.milestone_keys:
            return
        self.milestone_keys.append(key)
        self.milestone_count += 1
        self.milestones.append({"event": kind, "sequence": self.milestone_count,
            **{name: checkpoint[name] for name in ("generation", "directory", "state_path",
               "started_unix_time", "saved_unix_time", "duration_seconds", "bootstrap")
               if name in checkpoint}})

    def diagnostics(self):
        return {"bytes_read": self.offset, "invalid_records": self.invalid_records,
                "oversized_records": self.oversized_records, "rotations": self.rotations,
                "partial_record_bytes": len(self.pending), "checkpoint_milestones_observed": self.milestone_count,
                "checkpoint_milestones_history_dropped": max(0, self.milestone_count - len(self.milestones))}


def number(value):
    if isinstance(value, bool):
        return None
    if isinstance(value, int) or isinstance(value, float) and math.isfinite(value):
        return value
    return None


def descendant_closure_summary(value) -> dict:
    """Keep native dependency closure distinct from local publication counters."""
    result = {
        "available": False,
        "initial_total": None,
        "initial_closed": None,
        "total_domains": None,
        "total_closed": None,
        "locally_inspected": None,
        "unresolved_domains": None,
        "dependency_edges": None,
        "graph_revision": None,
        "snapshot_revision": None,
        "snapshot_stale": None,
        "snapshot_age_seconds": None,
        "last_refresh_seconds": None,
        "refresh_count": None,
        "refresh_seconds": None,
        "retained_storage_estimate_bytes": None,
        "refresh_scratch_estimate_bytes": None,
        "storage_estimate_scope": None,
        "closed_counts_are_conservative_lower_bounds": True,
        "family_closure_claim": False,
        "scope": "discovered_dependency_coverage; not termination or family certificate",
        "method": None,
        "reason": "native descendant closure was not reported",
    }
    if not isinstance(value, dict):
        return result
    counts = {key: value.get(key) if type(value.get(key)) is int and value[key] >= 0 else None
              for key in ("initial_total", "initial_closed", "total_domains", "total_closed",
                          "locally_inspected", "unresolved_domains", "dependency_edges")}
    for key in ("initial_total", "total_domains", "locally_inspected", "dependency_edges"):
        result[key] = counts[key]
    for key in ("graph_revision", "snapshot_revision", "refresh_count",
                "retained_storage_estimate_bytes", "refresh_scratch_estimate_bytes"):
        if type(value.get(key)) is int and value[key] >= 0:
            result[key] = value[key]
    for key in ("snapshot_age_seconds", "last_refresh_seconds", "refresh_seconds"):
        if number(value.get(key)) is not None and value[key] >= 0:
            result[key] = value[key]
    if type(value.get("snapshot_stale")) is bool:
        result["snapshot_stale"] = value["snapshot_stale"]
    for key in ("scope", "method", "storage_estimate_scope"):
        if isinstance(value.get(key), str):
            result[key] = value[key]
    if value.get("available") is not True:
        result["reason"] = (value.get("reason") if isinstance(value.get("reason"), str)
                            and value["reason"] else "native descendant closure unavailable")
        return result
    required = ("initial_total", "initial_closed", "total_domains", "total_closed", "unresolved_domains")
    if (any(counts[key] is None for key in required)
            or not 0 <= counts["initial_closed"] <= counts["initial_total"] <= counts["total_domains"]
            or not counts["initial_closed"] <= counts["total_closed"] <= counts["total_domains"]
            or counts["unresolved_domains"] != counts["total_domains"] - counts["total_closed"]):
        result["reason"] = "invalid native descendant-closure counters"
        return result
    result.update(counts, available=True, reason=None)
    return result


def progress_summary(event: dict, observed_at: float | None, now: float) -> dict:
    """Only native counters establish progress; no inferred closure fraction."""
    outer = event.get("progress", event)
    outer = outer if isinstance(outer, dict) else {}
    counters = outer.get("snapshot", outer)
    counters = counters if isinstance(counters, dict) else {}
    parallel = counters.get("parallel", {})
    parallel = parallel if isinstance(parallel, dict) else {}
    allocation = parallel.get("admission_preparation", {})
    allocation = allocation if isinstance(allocation, dict) else {}
    age = number(event.get("progress_age_seconds"))
    if observed_at is not None:
        age = (age or 0.0) + max(0.0, now - observed_at)
    checkpoint = counters.get("checkpoint", outer.get("checkpoint"))
    checkpoint_write = counters.get("checkpoint_write", outer.get("checkpoint_write"))
    closure = descendant_closure_summary(counters.get("descendant_closure"))
    if closure["snapshot_age_seconds"] is not None:
        closure["snapshot_age_seconds"] += max(0.0, age or 0.0)
    return {
        "phase": counters.get("phase") or outer.get("event") or "starting",
        "native_status": counters.get("status", outer.get("status")),
        "owner": counters.get("owner"),
        "progress_age_seconds": age,
        "descendant_closure": closure,
        "initial_entry_progress": {
            "total": number(counters.get("initial_entry_domains_total")),
            "locally_inspected": number(counters.get("initial_entry_domains_inspected")),
            "published": number(counters.get("initial_entry_domains_published")),
            "scope": "initial domain obligations, not tuple coverage or descendant closure",
        },
        "work": {
            "scheduled": number(counters.get("scheduled_nodes")),
            "locally_completed": number(counters.get("completed_nodes")),
            "pending": number(counters.get("queued_nodes")),
            "pending_descendants": number(counters.get("pending_descendant_domains")),
            "frontiers": number(counters.get("frontiers")),
            "conditional_successors": number(counters.get("conditional_successors")),
            "physical_inspections_started": number(parallel.get("physical_inspections_started")),
            "physical_inspections_published": number(parallel.get("physical_inspections_published")),
            "subdivided_logical_inspections": number(parallel.get("subdivided_logical_inspections")),
            "recent_local_completions_per_second": number(event.get("recent_nodes_per_second")),
            "pending_growth_per_second": number(event.get("queue_growth_per_second")),
        },
        "active_native_slots": number(parallel.get("active_workers")),
        "backpressured_native_slots": number(parallel.get("backpressured_workers")),
        "finished_native_awaiting_publication": number(parallel.get("finished_uncommitted_domains")),
        "worker_reservations": {
            "inspectors": number(allocation.get("inspection_worker_limit")),
            "admission_helpers": number(allocation.get("lookup_worker_limit")),
            "coordinator": number(allocation.get("coordinator_worker_limit")),
        },
        "checkpoint": checkpoint if isinstance(checkpoint, dict) else None,
        "checkpoint_write": checkpoint_write if isinstance(checkpoint_write, dict) else None,
        "closure_eta_seconds": None,
        "family_closure_claim": False,
    }


def clean(value) -> str:
    return "".join(character for character in str(value) if character.isprintable())


def count(value) -> str:
    return "unknown" if number(value) is None else f"{value:,.0f}"


def duration(seconds) -> str:
    if number(seconds) is None:
        return "unknown"
    seconds = max(0, int(seconds))
    return f"{seconds // 3600:02d}:{seconds // 60 % 60:02d}:{seconds % 60:02d}"


def bar(value, total, elapsed=0, width=16) -> str:
    if number(value) is None or number(total) is None or total <= 0 or not 0 <= value <= total:
        marker = int(elapsed or 0) % width
        return "[" + " " * marker + "·" + " " * (width - marker - 1) + "]"
    filled = int(width * value / total)
    return "[" + "━" * filled + "─" * (width - filled) + "]"


def dashboard(status: dict) -> list[str]:
    progress = status.get("progress", {})
    work = progress.get("work", {})
    entry = progress.get("initial_entry_progress", {})
    closure = descendant_closure_summary(progress.get("descendant_closure"))
    resources = status.get("resources", {})
    checkpoint = status.get("checkpoint") or progress.get("checkpoint") or {}
    checkpoint_write = status.get("checkpoint_write", progress.get("checkpoint_write")) or {}
    allocation = progress.get("worker_reservations", {})
    cpu = number(resources.get("native_busy_cores"))
    cpu_text = "warming sample" if cpu is None else f"{cpu:.1f} observed cores"
    rss = number(resources.get("aggregate_rss_bytes"))
    memory_text = "unknown" if rss is None else f"{rss / 1e9:.2f} GB"
    available = number(resources.get("host_available_bytes"))
    host_text = "unknown" if available is None else f"{available / 1e9:.1f} GB"
    rate = number(work.get("recent_local_completions_per_second"))
    rate_text = "unknown" if rate is None else f"{rate:,.1f}/s"
    checkpoint_text = checkpoint.get("state", "not yet reported by Rust")
    if checkpoint.get("generation") is not None:
        checkpoint_text += f" generation {checkpoint['generation']}"
    if checkpoint.get("directory"):
        checkpoint_text += " · " + clean(checkpoint["directory"])
    if number(checkpoint.get("saved_unix_time")) is not None:
        try:
            checkpoint_text += " · completed " + datetime.fromtimestamp(checkpoint["saved_unix_time"], timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
        except (OSError, OverflowError, ValueError):
            checkpoint_text += " · completion timestamp invalid"
    if number(checkpoint.get("duration_seconds")) is not None:
        checkpoint_text += f" in {checkpoint['duration_seconds']:.2f}s"
    if checkpoint.get("bootstrap"):
        checkpoint_text += " · bootstrap; preparation restarts on resume"
    if checkpoint_write.get("state") == "writing":
        started_text = ""
        if number(checkpoint_write.get("started_unix_time")) is not None:
            try:
                started_text = " · started " + datetime.fromtimestamp(checkpoint_write["started_unix_time"], timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
            except (OSError, OverflowError, ValueError):
                started_text = " · start timestamp invalid"
        checkpoint_text = f"WRITING generation {checkpoint_write.get('generation', '?')}{started_text} · {clean(checkpoint_write.get('state_path', checkpoint_write.get('directory', '')))} · last {checkpoint_text}"
    hard = number(status.get("hard_memory_bytes"))
    soft = number(status.get("soft_memory_bytes"))
    hard_text = "unknown" if hard is None else f"{hard / 1e9:.2f} GB"
    soft_text = "unknown" if soft is None else f"{soft / 1e9:.2f} GB"
    closure_bar = bar(closure["initial_closed"], closure["initial_total"], status.get("elapsed_seconds"))
    closure_note = "" if closure["available"] else " · " + clean(closure["reason"])
    closed_prefix = unresolved_prefix = ""
    if closure["available"] and closure["snapshot_stale"]:
        closure_note = f" · conservative snapshot {duration(closure['snapshot_age_seconds'])} ago"
        closed_prefix, unresolved_prefix = "≥", "≤"
    discovered = closure["total_domains"]
    if discovered is None:
        discovered = work.get("scheduled")
    stale = " · STALE HEARTBEAT; current activity unverified" if status.get("heartbeat_stale") else ""
    state = clean(status.get('state', 'starting')).upper()
    if status.get("heartbeat_stale") and state in ("STARTING", "RUNNING", "STOPPING"):
        state = "LAST REPORTED " + state
    return [
        f"RustRed · {state} · {duration(status.get('elapsed_seconds'))}{stale}",
        f"CPU {bar(cpu, status.get('workers'))} {cpu_text} / {count(status.get('workers'))} total reserved",
        f"Workers {count(progress.get('active_native_slots'))} native active, {count(progress.get('backpressured_native_slots'))} blocked"
        + (f" · {count(progress['finished_native_awaiting_publication'])} finished waiting"
           if progress.get('finished_native_awaiting_publication') is not None else "")
        + f" · reserved {count(allocation.get('inspectors'))} inspect + {count(allocation.get('admission_helpers'))} admission + {count(allocation.get('coordinator'))} coordinator",
        f"Closure {closure_bar} {closed_prefix}{count(closure['initial_closed'])} / {count(closure['initial_total'])} initial roots recursively closed{closure_note}",
        f"Domains {count(discovered)} discovered · {closed_prefix}{count(closure['total_closed'])} recursively closed · {unresolved_prefix}{count(closure['unresolved_domains'])} unresolved",
        f"Initial {count(entry.get('published'))} / {count(entry.get('total'))} published · initial native inspected {count(entry.get('locally_inspected'))} · not closure",
        f"Queue {count(work.get('pending'))} pending · {count(work.get('locally_completed'))} local completions · {rate_text} local · frontiers {count(work.get('frontiers'))}",
        f"Descendants {count(work.get('pending_descendants'))} pending · discovered dependency coverage only; not termination/family proof · closure ETA unknown",
        f"Memory {memory_text} / {hard_text} ceiling · save+stop at {soft_text} · host available {host_text}",
        f"Checkpoint {clean(checkpoint_text)}",
        f"Phase {clean(progress.get('phase', 'starting'))} · update age {duration(progress.get('progress_age_seconds'))} · heartbeat age {duration(status.get('heartbeat_age_seconds'))} · closure ETA unknown",
        f"Receipts {clean(status.get('run_directory', ''))}",
    ]


class Presenter:
    """One reusable display; redirected output contains plain periodic lines."""
    def __init__(self, stream=None, enabled=True, plain_seconds=30.0):
        self.stream = sys.stderr if stream is None else stream
        self.enabled = enabled
        self.tty = self.stream.isatty() and os.environ.get("TERM") != "dumb"
        self.color = self.tty and "NO_COLOR" not in os.environ
        self.plain_seconds = plain_seconds
        self.last_at = None
        self.last_state = None
        self.drawn = 0
        self.last_milestone = 0

    def render(self, status: dict, now=None, force=False):
        if not self.enabled:
            return
        now = time.monotonic() if now is None else now
        milestones = [event for event in status.get("checkpoint_milestones", [])
                      if event.get("sequence", 0) > self.last_milestone]
        if milestones:
            if self.tty and self.drawn:
                self.stream.write(f"\x1b[{self.drawn}A\r\x1b[J")
                self.drawn = 0
            for event in milestones:
                label = "started" if event["event"] == "checkpoint_started" else "saved"
                detail = f"RustRed checkpoint {label} · generation {event.get('generation', '?')}"
                timestamp = number(event.get("started_unix_time" if label == "started" else "saved_unix_time"))
                if timestamp is not None:
                    try:
                        detail += " · " + datetime.fromtimestamp(timestamp, timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
                    except (OSError, OverflowError, ValueError):
                        detail += " · timestamp invalid"
                if number(event.get("duration_seconds")) is not None:
                    detail += f" · {event['duration_seconds']:.2f}s"
                detail += " · " + str(event.get("state_path", event.get("directory", "")))
                self.stream.write(clean(detail) + "\n")
            self.last_milestone = max(event["sequence"] for event in milestones)
            self.stream.flush()
        state = status.get("state")
        if not self.tty and not force and self.last_at is not None and state == self.last_state and now - self.last_at < self.plain_seconds:
            return
        lines = dashboard(status)
        if self.tty:
            width = max(20, shutil.get_terminal_size((100, 24)).columns - 1)
            if self.drawn:
                self.stream.write(f"\x1b[{self.drawn}A")
            for index, line in enumerate(lines):
                line = clean(line)
                line = line if len(line) <= width else line[:width - 1] + "…"
                if self.color:
                    color = "1;36" if index == 0 else "32" if line.startswith("CPU") else "33" if line.startswith("Memory") else "34" if line.startswith("Closure") else None
                    if color:
                        line = f"\x1b[{color}m" + line + "\x1b[0m"
                self.stream.write("\r\x1b[2K" + line + "\n")
            self.drawn = len(lines)
        else:
            self.stream.write(" | ".join(clean(line) for line in lines[:-1]) + "\n")
        self.stream.flush()
        self.last_at = now
        self.last_state = state


def read_status(directory: Path) -> dict:
    with (directory / "status.json").open("rb") as stream:
        raw = stream.read(MAX_RECORD_BYTES + 1)
    if len(raw) > MAX_RECORD_BYTES:
        raise ValueError("status document exceeds 1 MiB")
    result = json.loads(raw)
    if not isinstance(result, dict):
        raise ValueError("status document must be an object")
    written = number(result.get("heartbeat_unix_time"))
    result["heartbeat_age_seconds"] = None if written is None else max(0.0, time.time() - written)
    threshold = max(10.0, 3 * (number(result.get("sample_seconds")) or 2.0))
    result["heartbeat_stale"] = written is None or result["heartbeat_age_seconds"] > threshold
    identity = result.get("process_identity", {})
    try:
        current_boot = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except OSError:
        current_boot = None
    boot_matches = bool(current_boot and identity.get("boot_id") == current_boot)
    result["observed_boot_id_matches"] = boot_matches
    alive = {}
    for role in ("supervisor", "native"):
        process = identity.get(role, {})
        try:
            pid, expected_start = int(process["pid"]), int(process["start_ticks"])
            stat = Path(f"/proc/{pid}/stat").read_text()
            alive[role] = boot_matches and int(stat[stat.rfind(")") + 2:].split()[19]) == expected_start
        except (OSError, ValueError, KeyError, TypeError, IndexError):
            alive[role] = False
    result["observed_processes_alive"] = alive
    if result.get("state") in ("starting", "running", "stopping") and not alive.get("supervisor"):
        result["heartbeat_stale"] = True
    progress = result.get("progress", {})
    if isinstance(progress, dict) and number(progress.get("progress_age_seconds")) is not None:
        progress["progress_age_seconds"] += result["heartbeat_age_seconds"] or 0
    closure = progress.get("descendant_closure") if isinstance(progress, dict) else None
    if isinstance(closure, dict) and number(closure.get("snapshot_age_seconds")) is not None:
        closure["snapshot_age_seconds"] += result["heartbeat_age_seconds"] or 0
    return result


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("run_directory", type=Path)
    parser.add_argument("--once", action="store_true")
    parser.add_argument("--json", action="store_true", help="one machine-readable current status")
    parser.add_argument("--interval", type=float, default=2.0)
    args = parser.parse_args(argv)
    if not math.isfinite(args.interval) or args.interval < 0.1:
        parser.error("interval must be finite and at least 0.1 seconds")
    presenter = Presenter()
    try:
        while True:
            status = read_status(args.run_directory)
            if args.json:
                print(json.dumps(status, sort_keys=True))
            else:
                presenter.render(status, force=True)
            if args.once or args.json or status.get("state") in ("completed", "paused", "failed", "stopped"):
                return 0
            time.sleep(args.interval)
    except KeyboardInterrupt:
        return 0
    except (OSError, ValueError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    raise SystemExit(main())
