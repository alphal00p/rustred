#!/usr/bin/env python3
"""Bounded live status and terminal presentation for a saved-owner campaign.

This module observes receipts only. It cannot resume work or certify closure.
"""
from __future__ import annotations

import argparse
import importlib.util
from collections import deque
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
MAX_CHECKPOINT_METADATA_BYTES = 16 * 1024 * 1024


def _sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


TELEMETRY = _sibling("campaign_telemetry")
DASHBOARD = _sibling("campaign_dashboard")


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


class CheckpointSizeCache:
    """Read bounded published metadata once per generation, never walk a tree.

    Counts the files referenced by that generation, including CP6 sealed record
    sidecars. It excludes older/orphan generations, directory metadata and the
    small manifest itself, matching native CP5's payload-byte convention.
    This is observation, not checkpoint authentication or resume authority.
    """
    def __init__(self):
        self.key = None
        self.details = {}
        self.checked_at = None

    @staticmethod
    def _json(path):
        with Path(path).open("rb") as stream:
            raw = stream.read(MAX_CHECKPOINT_METADATA_BYTES + 1)
        if len(raw) > MAX_CHECKPOINT_METADATA_BYTES:
            raise ValueError("checkpoint metadata exceeds observation limit")
        return json.loads(raw)

    @staticmethod
    def _file(reference):
        if not isinstance(reference, dict):
            raise ValueError("invalid checkpoint file reference")
        name, size = reference.get("file"), reference.get("bytes")
        if not isinstance(name, str) or not name or Path(name).name != name or name in (".", ".."):
            raise ValueError("invalid checkpoint file name")
        if type(size) is not int or size < 0:
            raise ValueError("invalid checkpoint byte count")
        return name, size

    def enrich(self, checkpoint):
        if not isinstance(checkpoint, dict) or checkpoint.get("state") != "saved":
            return checkpoint
        if type(checkpoint.get("bytes")) is int and checkpoint["bytes"] >= 0:
            return checkpoint
        directory, generation = checkpoint.get("directory"), checkpoint.get("generation")
        if not isinstance(directory, str) or type(generation) is not int or generation < 0:
            return checkpoint
        key = (directory, generation)
        now = time.monotonic()
        if key != self.key or (self.details.get("bytes") is None and
                               (self.checked_at is None or now - self.checked_at >= 30)):
            self.key, self.details = key, {}
            self.checked_at = now
            try:
                self.details = self._measure(Path(directory), generation)
            except (OSError, ValueError, TypeError, KeyError, RecursionError) as error:
                self.details = {"bytes": None, "bytes_observation_error": str(error)[:300]}
        return {**checkpoint, **self.details}

    def _measure(self, directory, generation):
        manifest = None
        for name in ("latest.json", "previous.json"):
            try:
                document = self._json(directory / name)
            except FileNotFoundError:
                continue
            candidate = document.get("manifest", document) if isinstance(document, dict) else {}
            if not isinstance(candidate, dict):
                raise ValueError("checkpoint manifest must be an object")
            if candidate.get("generation") == generation:
                manifest = candidate
                break
        if manifest is None:
            raise ValueError("saved generation is not named by latest/previous manifest")
        references = []
        if manifest.get("format") == "RUSTRED-WALK-CP5":
            sections = manifest.get("sections")
            if not isinstance(sections, dict):
                raise ValueError("missing checkpoint sections")
            for section in sections.values():
                if section is None:
                    continue
                references.extend(section["segments"] if "segments" in section else [section])
        elif manifest.get("format") == "RUSTRED-WALK-CP6":
            if not isinstance(manifest.get("files"), list):
                raise ValueError("checkpoint files must be an array")
            references = list(manifest["files"])
            for reference in tuple(references):
                if not isinstance(reference, dict):
                    raise ValueError("invalid checkpoint file reference")
                if reference.get("key") == "record-segments":
                    name, _ = self._file(reference)
                    segments = self._json(directory / name)
                    if not isinstance(segments, list):
                        raise ValueError("invalid checkpoint record-segment index")
                    references.extend(segments)
        else:
            raise ValueError("unsupported checkpoint metadata format")
        files = {}
        for reference in references:
            name, size = self._file(reference)
            if name in files and files[name] != size:
                raise ValueError("inconsistent duplicate checkpoint file size")
            files[name] = size
        return {"bytes": sum(files.values()), "bytes_source": "published_manifest",
                "bytes_scope": "this generation's referenced payloads, including sealed record segments"}


_READ_CHECKPOINT_SIZE = CheckpointSizeCache()


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
        self.query_admission = None
        # Every complete parsed record is also handed to these callbacks (for
        # example the derived heartbeat metrics); they must not raise.
        self.observers = []

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
                    self.query_admission = None
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
                self._query_scope(value)
                for observer in self.observers:
                    observer(value)
            except (ValueError, UnicodeError, RecursionError):
                self.invalid_records += 1
        self.pending = b""

    def _query_scope(self, value):
        outer = value.get("progress", value)
        if not isinstance(outer, dict):
            return
        snapshot = outer.get("snapshot", outer)
        snapshot = snapshot if isinstance(snapshot, dict) else {}
        scope = snapshot.get("query_admission", outer.get("query_admission"))
        if isinstance(scope, dict):
            # Immutable declared scope survives lean heartbeats. Retain only
            # bounded scalar counts, never a cached claim that queries closed.
            self.query_admission = {name: number(scope.get(name)) for name in (
                "requested", "required", "auxiliary", "original_requested", "original_required",
                "appended_requested", "appended_required", "admitted_required", "unadmitted")}

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
               "started_unix_time", "saved_unix_time", "duration_seconds", "bytes", "bootstrap")
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


descendant_closure_summary = TELEMETRY.descendant_closure_summary
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
    if (closure.get("refresh_policy", {}).get("status") == "unknown"
            and isinstance(parallel.get("closure_refresh_policy"), dict)):
        closure["refresh_policy"] = dict(parallel["closure_refresh_policy"])
    if closure["snapshot_age_seconds"] is not None:
        closure["snapshot_age_seconds"] += max(0.0, age or 0.0)
    return {
        "phase": counters.get("phase") or outer.get("event") or "starting",
        "native_status": counters.get("status", outer.get("status")),
        "native_stop_reason": counters.get("stop_reason", outer.get("stop_reason")),
        "owner": counters.get("owner"),
        "progress_age_seconds": age,
        "descendant_closure": closure,
        "query_admission": counters.get("query_admission", outer.get("query_admission")),
        "encountered_numerator_rank": counters.get("encountered_numerator_rank"),
        "max_scheduled_finite_rank": number(counters.get("max_scheduled_finite_rank")),
        "unbounded_rank_domains": number(counters.get("unbounded_rank_domains")),
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


# Public convenience names; presentation is implemented in its own consumer.
Presenter = DASHBOARD.Presenter
dashboard = DASHBOARD.dashboard
derived_lines = DASHBOARD.derived_lines
clean = DASHBOARD.clean
duration = DASHBOARD.duration
def read_status(directory: Path) -> dict:
    directory = active_status_directory(directory)
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
    checkpoint = result.get("checkpoint") or (progress.get("checkpoint") if isinstance(progress, dict) else None)
    result["checkpoint"] = _READ_CHECKPOINT_SIZE.enrich(checkpoint)
    return result


def active_status_directory(directory: Path) -> Path:
    """Observe a run directly, or follow a campaign's explicit phase pointer."""
    if (directory / "status.json").is_file():
        return directory
    for pointer in (directory / "master-reduction/active-phase.json", directory / "active-run.json"):
        if not pointer.is_file():
            continue
        with pointer.open("rb") as stream:
            raw = stream.read(MAX_RECORD_BYTES + 1)
        if len(raw) > MAX_RECORD_BYTES:
            raise ValueError("active-phase pointer exceeds 1 MiB")
        value = json.loads(raw)
        path = value.get("run_directory") if isinstance(value, dict) else None
        if not isinstance(path, str) or not path:
            raise ValueError("active-phase pointer has no run directory")
        return Path(path)
    return directory


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("run_directory", type=Path, help="run directory, or campaign directory to follow its current phase")
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
            if args.once or args.json or status.get("state") in ("completed", "published_unrefined", "completed_nonminimal", "paused", "failed", "stopped"):
                return 0
            time.sleep(args.interval)
    except KeyboardInterrupt:
        return 0
    except (OSError, ValueError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    raise SystemExit(main())
