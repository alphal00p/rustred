"""Presentation-independent, bounded campaign telemetry (no algebra or control).

One normalized JSON frame can feed a terminal, a saved time series, or a future
web consumer. Missing measurements remain null. Rates come from the existing
heartbeat window, not from display polling or inferred progress.
"""
from __future__ import annotations

import json
import math
from pathlib import Path

SCHEMA = "rustred.campaign-telemetry.v1"
MAX_FRAME_BYTES = 64 * 1024


def mapping(value):
    return value if isinstance(value, dict) else {}


def number(value):
    if type(value) not in (int, float):
        return None
    try:
        return value if math.isfinite(value) else None
    except OverflowError:
        # Keep ordinary integer counts exact, but reject malformed integers
        # that cannot participate safely in display/plot floating arithmetic.
        return None


def clean(value, limit=4096):
    return "".join(character for character in str(value) if character.isprintable())[:limit]


def _numbers(source, keys):
    return {key: number(source.get(key)) for key in keys}


def normalize_status(status, sequence=None):
    """Convert one already observed status into a small, explicit public frame.

    Local completions are *not* recursively closed domains. The latter are
    conservative counts observed at periodic graph scans; their rate is an
    observed-window rate, never an instantaneous solver throughput claim.
    """
    status = mapping(status)
    progress = mapping(status.get("progress"))
    derived = mapping(status.get("derived"))
    work = mapping(progress.get("work"))
    closure = descendant_closure_summary(progress.get("descendant_closure"))
    net = mapping(derived.get("discovery_closure_net_1h"))
    entry = mapping(progress.get("initial_entry_progress"))
    resources = mapping(status.get("resources"))
    window = number(derived.get("window_seconds")) or 3600.0
    span = number(derived.get("window_wall_seconds"))
    completed_rate = number(derived.get("completions_per_hour_1h"))
    completed_delta = number(derived.get("completions_delta_1h"))
    completion_state = "measured" if completed_rate is not None and span and span > 0 else "missing"
    if derived.get("completions_window_valid_1h") is False:
        completed_rate, completion_state = None, "invalid_window"
    if completed_rate is not None and completed_rate < 0 or completed_delta is not None and completed_delta < 0:
        completed_rate, completion_state = None, "counter_reset"
    elapsed = number(derived.get("now_seconds"))
    completion = {
        "per_second": completed_rate / 3600.0 if completion_state == "measured" else None,
        "delta": completed_delta, "covered_seconds": span, "window_seconds": window,
        "first_elapsed_seconds": number(derived.get("first_elapsed_seconds")),
        "last_elapsed_seconds": number(derived.get("last_elapsed_seconds")),
        "warmup": derived.get("completions_warmup_1h", elapsed is None or elapsed < window) is True,
        "state": completion_state,
    }
    closure_rate = number(net.get("closed_per_second"))
    closure_span = number(net.get("covered_seconds"))
    if net.get("state") not in ("valid", "warmup") or not closure_span or closure_span <= 0:
        closure_rate = None
    recursive = {
        "per_second": closure_rate,
        "delta": number(net.get("closed_delta")),
        "covered_seconds": closure_span,
        "window_seconds": number(net.get("window_seconds")),
        "first_elapsed_seconds": number(net.get("first_elapsed_seconds")),
        "last_elapsed_seconds": number(net.get("last_elapsed_seconds")),
        "warmup": net.get("warmup") is not False,
        "state": clean(net.get("state", "missing"), 100),
        "reset_reason": clean(net["reset_reason"], 100) if net.get("reset_reason") else None,
        "observation": "conservative, scan-batched; not instantaneous throughput",
    }
    closure_counts = _numbers(closure, ("initial_total", "initial_closed", "total_domains", "total_closed",
                                         "unresolved_domains", "dependency_edges"))
    if closure_counts["total_domains"] is None:
        closure_counts["total_domains"] = number(work.get("scheduled"))
    available = closure.get("available") is True
    if not available:
        for key in ("initial_closed", "total_closed", "unresolved_domains"):
            closure_counts[key] = None
    snapshot = _numbers(closure if available else net,
                        ("snapshot_age_seconds", "snapshot_revision", "graph_revision", "refresh_count",
                         "last_refresh_seconds", "refresh_seconds"))
    if not available and snapshot["snapshot_age_seconds"] is not None:
        snapshot["snapshot_age_seconds"] += max(0, number(status.get("heartbeat_age_seconds")) or 0)
    snapshot.update(available=available,
                    stale=(closure if available else net).get("snapshot_stale"),
                    advanced=net.get("snapshot_advanced"),
                    closed_counts_are_conservative_lower_bounds=available,
                    unresolved_counts_are_conservative_upper_bounds=available,
                    reason=clean(closure.get("reason", "closure telemetry unavailable"), 300) if not available else None)
    checkpoint = mapping(status.get("checkpoint") or progress.get("checkpoint"))
    writing = mapping(status.get("checkpoint_write", progress.get("checkpoint_write")))

    def checkpoint_fields(source, text_limit=4096):
        return {**_numbers(source, ("generation", "saved_unix_time", "started_unix_time", "duration_seconds", "bytes")),
                "state": clean(source.get("state", "unknown"), 80),
                "directory": clean(source.get("directory", ""), text_limit),
                "state_path": clean(source.get("state_path", ""), text_limit),
                "bootstrap": source.get("bootstrap") is True}

    milestones = []
    raw_milestones = status.get("checkpoint_milestones")
    for event in raw_milestones[-64:] if isinstance(raw_milestones, list) else []:
        event = mapping(event)
        if event.get("event") not in ("checkpoint_started", "checkpoint_saved"):
            continue
        sequence_number = event.get("sequence")
        if type(sequence_number) is not int or sequence_number < 0:
            continue
        milestones.append({**checkpoint_fields(event, text_limit=128), "event": event["event"], "sequence": sequence_number})

    return {
        "schema": SCHEMA, "sequence": sequence,
        "unix_time": number(status.get("heartbeat_unix_time")),
        "elapsed_seconds": number(status.get("elapsed_seconds")),
        "state": clean(status.get("state", "starting"), 100),
        "phase": clean(progress.get("phase", "starting"), 100),
        "stop_reason": clean(status.get("stop_reason") or status.get("native_stop_reason") or
                             progress.get("native_stop_reason") or "", 300),
        "run_directory": clean(status.get("run_directory", "")),
        "heartbeat_stale": status.get("heartbeat_stale") is True,
        "heartbeat_age_seconds": number(status.get("heartbeat_age_seconds")),
        "progress_age_seconds": number(progress.get("progress_age_seconds")),
        "counts": {**_numbers(work, ("scheduled", "locally_completed", "pending", "pending_descendants", "frontiers")),
                   **closure_counts,
                   "initial_published": number(entry.get("published")),
                   "initial_inspected": number(entry.get("locally_inspected")),
                   "initial_entries": number(entry.get("total"))},
        "resources": {**_numbers(resources, ("native_busy_cores", "aggregate_rss_bytes", "host_available_bytes",
                                              "own_swap_growth_bytes_per_second", "host_swap_in_bytes_per_second")),
                      **_numbers(status, ("workers", "hard_memory_bytes", "soft_memory_bytes")),
                      **_numbers(progress, ("active_native_slots", "backpressured_native_slots", "finished_native_awaiting_publication")),
                      "reservations": _numbers(mapping(progress.get("worker_reservations")), ("inspectors", "admission_helpers", "coordinator")),
                      "computing_inspectors_mean_1h": number(derived.get("computing_inspectors_mean_1h")),
                      "coordinator_duty_1h": number(derived.get("coordinator_duty_1h")),
                      "stall_share_5s": number(derived.get("stall_share_5s"))},
        "rates": {"local_completion": completion, "recursive_closure": recursive,
                  "completions_per_hour_1h": number(derived.get("completions_per_hour_1h")),
                  "discovery_minus_closure": {**_numbers(net, (
                      "per_second", "covered_seconds", "window_seconds",
                      "discovered_delta", "closed_delta", "discovered_per_second", "closed_per_second",
                      "first_elapsed_seconds", "last_elapsed_seconds")),
                      "warmup": net.get("warmup") is True,
                      "state": clean(net["state"], 100) if net.get("state") is not None else None,
                      "reset_reason": clean(net["reset_reason"], 100) if net.get("reset_reason") is not None else None},
                  "pending_growth_per_completion_1h": number(derived.get("pending_growth_per_completion_1h")),
                  "recent_local_completions_per_second": number(work.get("recent_local_completions_per_second")),
                  "max_scheduled_finite_rank": number(derived.get("max_scheduled_finite_rank")),
                  "rss_bytes_per_discovered_domain": number(derived.get("rss_bytes_per_discovered_domain")),
                  "checkpoint_duty": number(derived.get("checkpoint_duty"))},
        "closure_snapshot": snapshot,
        "checkpoint": checkpoint_fields(checkpoint), "checkpoint_write": checkpoint_fields(writing),
        "checkpoint_milestones": milestones,
        "family_closure_claim": False,
        "scope": "observed local work and conservative dependency closure; no closure ETA or family certificate",
    }


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



class TelemetryStream:
    """Append compact bounded JSONL frames. Observation failures never stop work.

    One writer per fresh supervisor run directory. No rate history is retained
    here; the bounded heartbeat window owns the arithmetic. A failed writer is
    disabled and its error exposed in status, rather than retry-spamming disk.
    """
    def __init__(self, path: Path):
        self.path = Path(path)
        self.sequence = 0
        self.error = None
        self.bytes_written = 0

    def emit(self, status):
        try:
            frame = normalize_status(status, self.sequence)
        except (ValueError, TypeError, OverflowError, RecursionError) as error:
            self.error = clean(error, 300)
            frame = normalize_status({}, self.sequence)
            frame["observation_error"] = self.error
        self.sequence += 1
        if self.error is None:
            try:
                encoded = json.dumps(frame, ensure_ascii=False, allow_nan=False, separators=(",", ":")).encode() + b"\n"
                if len(encoded) > MAX_FRAME_BYTES:
                    raise ValueError("normalized telemetry frame exceeds 64 KiB")
                with self.path.open("ab") as stream:
                    stream.write(encoded)
                self.bytes_written += len(encoded)
            except (OSError, ValueError, TypeError, OverflowError, RecursionError) as error:
                self.error = clean(error, 300)
        return frame

    def diagnostics(self):
        return {"schema": SCHEMA, "path": str(self.path), "frames_observed": self.sequence,
                "bytes_written": self.bytes_written, "error": self.error}
