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


def completed_scan_ratio(history, now=None):
    """A retained observation between two completed scans, never a live estimate."""
    fields = ("completed_unix_seconds", "total_domains", "total_closed", "initial_closed",
              "refresh_count", "scan_seconds")
    history = mapping(history)
    result = {"previous": None, "latest": None, "value": None, "infinite": False,
              "state": "awaiting_two_scans", "covered_seconds": None,
              "discovered_delta": None, "closed_delta": None, "age_seconds": None,
              "discovered_per_second": None, "closed_per_second": None,
              "scope": "last two completed scans; retained when current snapshot is stale"}
    for endpoint in ("previous", "latest"):
        raw = mapping(history.get(endpoint))
        if not raw:
            continue
        sample = _numbers(raw, fields)
        required = ("completed_unix_seconds", "total_domains", "total_closed", "refresh_count")
        if (any(sample[key] is None or sample[key] < 0 for key in required)
                or any(type(sample[key]) is not int for key in
                       ("total_domains", "total_closed", "refresh_count"))
                or sample["total_closed"] > sample["total_domains"]):
            result["state"] = "invalid_scan"
            return result
        result[endpoint] = sample
    before, after = result["previous"], result["latest"]
    if after is not None and number(now) is not None:
        result["age_seconds"] = max(0, now - after["completed_unix_seconds"])
    if before is None or after is None:
        return result
    span = after["completed_unix_seconds"] - before["completed_unix_seconds"]
    discovered = after["total_domains"] - before["total_domains"]
    closed = after["total_closed"] - before["total_closed"]
    if span <= 0 or after["refresh_count"] <= before["refresh_count"] or min(discovered, closed) < 0:
        result["state"] = "scan_counter_reset"
        return result
    result.update(state="valid", covered_seconds=span, discovered_delta=discovered, closed_delta=closed,
                  discovered_per_second=discovered / span, closed_per_second=closed / span)
    if closed:
        result["value"] = discovered / closed
    elif discovered:
        result["infinite"] = True
        result["state"] = "no_observed_closures"
    else:
        result["state"] = "empty_scan_interval"
    return result


def encountered_rank_summary(progress, derived):
    """Native domain geometry bound, not an assertion that its maximum was attained."""
    raw = mapping(progress.get("encountered_numerator_rank"))
    if raw:
        maximum = raw.get("maximum")
        if isinstance(maximum, str) and len(maximum) <= 128 and maximum.isascii() and maximum.isdecimal():
            maximum = int(maximum)
        maximum = maximum if type(maximum) is int and maximum >= 0 else None
        result = {**_numbers(raw, ("finite_domains", "unbounded_domains", "unknown_domains", "empty_domains")),
                  "maximum": maximum,
                  "status": raw.get("status", "unknown"), "scope": clean(raw.get("scope", ""), 200)}
        if result["status"] not in ("finite", "unbounded", "unknown") or (
                result["status"] == "finite" and (result["maximum"] is None or result["maximum"] < 0)):
            result["status"] = "unknown"
        return result
    finite = number(progress.get("max_scheduled_finite_rank", derived.get("max_scheduled_finite_rank")))
    unbounded = number(progress.get("unbounded_rank_domains", derived.get("unbounded_rank_domains")))
    # The historical "unbounded" counter only means an absent explicit cap.
    # Finite rectangular geometry can still bound those domains (rank zero
    # for an all-positive sector), so it cannot establish infinite rank.
    return {"status": "finite" if unbounded == 0 and finite is not None else "unknown",
            "maximum": finite, "finite_domains": None, "unbounded_domains": unbounded,
            "unknown_domains": None, "scope": "legacy explicit rank caps; effective geometry unavailable"}


def discovery_per_recursive_closure_1h(gap, snapshot=None):
    """New discoveries / recursive closures in the SAME trailing-hour window.

    This is an observed scan-batched ratio, not convergence or a bound.
    Never substitute the independently sampled local-completion window.
    An explicit infinity flag keeps the serialized frame valid JSON; null
    without that flag means unavailable, including an empty (0/0) window.
    """
    gap = mapping(gap)
    snapshot = gap if snapshot is None else mapping(snapshot)
    result = {**_numbers(gap, ("covered_seconds", "window_seconds", "first_elapsed_seconds",
                               "last_elapsed_seconds")), "value": None, "infinite": False,
              "warmup": gap.get("warmup") is True,
              "state": clean(gap.get("state") or "missing", 100)}
    if result["state"] not in ("valid", "warmup"):
        return result
    span, window, first, last = (result[key] for key in
        ("covered_seconds", "window_seconds", "first_elapsed_seconds", "last_elapsed_seconds"))
    if any(value is None for value in (span, window, first, last)):
        result["state"] = "missing_window"
        return result
    if (span <= 0 or window <= 0 or first < 0 or last <= first or span > window
            or not math.isclose(last - first, span, rel_tol=1e-9, abs_tol=1e-9)):
        result["state"] = "window_mismatch"
        return result
    discovered, closed = number(gap.get("discovered_delta")), number(gap.get("closed_delta"))
    if discovered is None or closed is None or discovered < 0 or closed < 0:
        result["state"] = "invalid_deltas"
        return result
    # Reject inconsistent/malformed supplied rate fields, but the ratio itself
    # uses the paired deltas, not rounded rates or an unrelated sample window.
    for key, expected in (("discovered_per_second", discovered / span),
                          ("closed_per_second", closed / span),
                          ("per_second", (discovered - closed) / span)):
        if key in gap and (number(gap[key]) is None or not math.isclose(gap[key], expected, rel_tol=1e-9, abs_tol=1e-9)):
            result["state"] = "window_mismatch"
            return result
    if snapshot.get("available") is False:
        result["state"] = "closure_unavailable"
        return result
    age = number(snapshot.get("snapshot_age_seconds"))
    stale = snapshot.get("stale", snapshot.get("snapshot_stale"))
    # A dirty graph with no closure scan inside this entire sampled window
    # provides no closure-rate observation. Zero cached counter movement is
    # not a measured zero rate. An unchanged, explicitly fresh graph is not
    # affected; its old but still-current counter can legitimately be flat.
    if stale is not False and age is not None and age >= span:
        result["state"] = "awaiting_closure_scan"
        return result
    if closed == 0:
        if discovered > 0:
            result["infinite"] = True
        else:
            result["state"] = "empty_window"
        return result
    result["value"] = number(discovered / closed)
    if result["value"] is None:
        result["state"] = "invalid_deltas"
    return result


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
    scan_ratio = completed_scan_ratio(closure.get("scan_history"),
        (number(status.get("heartbeat_unix_time")) or 0) +
        max(0, number(status.get("heartbeat_age_seconds")) or 0))
    if snapshot["snapshot_age_seconds"] is None and scan_ratio.get("age_seconds") is not None:
        # A restored native monotonic clock has no old scan instant. The
        # persisted completed-scan timestamp still supplies its observed age.
        snapshot["snapshot_age_seconds"] = scan_ratio["age_seconds"]
    ratio = discovery_per_recursive_closure_1h(net, snapshot)
    if not available:
        ratio.update(value=None, infinite=False, state="closure_unavailable")
    if ratio["state"] in ("awaiting_closure_scan", "closure_unavailable"):
        recursive.update(per_second=None, state=ratio["state"])
    snapshot["refresh_policy"] = mapping(closure.get("refresh_policy"))
    policy = snapshot["refresh_policy"]
    earliest = number(policy.get("earliest_refresh_unix_seconds"))
    observed = number(status.get("heartbeat_unix_time"))
    if policy.get("status") == "throttled" and earliest is not None and observed is not None:
        remaining = max(0, earliest - observed - max(0, number(status.get("heartbeat_age_seconds")) or 0))
        policy["next_refresh_seconds"] = remaining
        if remaining == 0:
            policy["status"] = "eligible"
    checkpoint = mapping(status.get("checkpoint") or progress.get("checkpoint"))
    writing = mapping(status.get("checkpoint_write", progress.get("checkpoint_write")))

    def checkpoint_fields(source, text_limit=4096):
        return {**_numbers(source, ("generation", "saved_unix_time", "started_unix_time", "duration_seconds", "bytes")),
                "state": clean(source.get("state", "unknown"), 80),
                "directory": clean(source.get("directory", ""), text_limit),
                "state_path": clean(source.get("state_path", ""), text_limit),
                "bytes_source": clean(source.get("bytes_source", "native" if number(source.get("bytes")) is not None else "unknown"), 100),
                "bytes_scope": clean(source.get("bytes_scope", ""), 300),
                "bytes_observation_error": clean(source.get("bytes_observation_error", ""), 300),
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

    frame = {
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
        "query_scope": _numbers(mapping(progress.get("query_admission")),
            ("required", "original_required", "appended_required", "admitted_required", "unadmitted")),
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
                  "discovery_per_recursive_closure_1h": ratio,
                  "discovery_per_recursive_closure_scans": scan_ratio,
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
                  "encountered_numerator_rank": encountered_rank_summary(progress, derived),
                  "rss_bytes_per_discovered_domain": number(derived.get("rss_bytes_per_discovered_domain")),
                  "checkpoint_duty": number(derived.get("checkpoint_duty")),
                  "checkpoint_duty_scope": clean(derived.get("checkpoint_duty_scope", "unknown"), 300)},
        "closure_snapshot": snapshot,
        "checkpoint": checkpoint_fields(checkpoint), "checkpoint_write": checkpoint_fields(writing),
        "checkpoint_milestones": milestones,
        "family_closure_claim": False,
        "scope": "observed local work and conservative dependency closure; no closure ETA or family certificate",
    }
    master = mapping(status.get("master_reduction"))
    if master:
        operation = master.get("operation", "refine")
        frame["phase"] = "Artifact publication" if operation == "publish" else "Master refinement"
        frame["master_reduction"] = {
            "operation": operation,
            **_numbers(master, ("raw_terminals", "normalized_terminals", "remaining_terminals",
                                "relation_rows", "eliminated_terminals", "completed_work", "total_work",
                                "seed_depth", "independent_rows", "auxiliary_columns", "nonzeros")),
            **{key: clean(master.get(key, ""), 4096 if key == "artifact" else 300)
               for key in ("stage", "status", "artifact", "scope_binding")},
            "master_minimality_claim": False,
            "numerical_evaluation": False,
        }
        collection = mapping(master.get("collection"))
        if collection:
            frame["master_reduction"]["collection"] = {
                "finite_feedback_stage": clean(collection.get("finite_feedback_stage", ""), 300),
                **_numbers(collection, ("finite_feedback_rows", "finite_feedback_columns",
                                        "finite_feedback_auxiliary_columns", "finite_feedback_aliases",
                                        "finite_feedback_equations", "finite_feedback_nonzeros",
                                        "finite_feedback_replay_operations")),
            }
        frame["scope"] = ("scoped dependency coverage and portable rule package; no master refinement" if operation == "publish" else
                          "bounded exact terminal relations; no minimality or numerical master evaluation claim")
    return frame


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
        "scan_history": {},
        "refresh_policy": {},
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
    # Native producer owns the bounded two-scan history. Keep it independently
    # of snapshot freshness; renderer validation never invents a completed scan.
    history = mapping(value.get("scan_history"))
    result["scan_history"] = {key: _numbers(mapping(history.get(key)),
        ("completed_unix_seconds", "total_domains", "total_closed", "initial_closed", "refresh_count", "scan_seconds"))
        for key in ("previous", "latest") if mapping(history.get(key))}
    policy = mapping(value.get("refresh_policy"))
    result["refresh_policy"] = {**_numbers(policy, ("next_refresh_seconds", "earliest_refresh_unix_seconds")),
                                "status": clean(policy.get("status", "unknown"), 80)}
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
