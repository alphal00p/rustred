#!/usr/bin/env python3
"""Derived heartbeat metrics from a native events.jsonl stream.

The live supervisor (`shared_owner_campaign.py`) feeds every parsed event into
`HeartbeatWindow` and publishes `derived()` in `status.json`; this module runs
the same arithmetic offline over any `events.jsonl` and an elapsed window, so a
pilot can be compared with a live run at matched elapsed time. All numbers are
measured deltas of native counters; nothing here estimates completion time or
family closure.
"""
from __future__ import annotations

import argparse
from collections import deque
import json
import math
from pathlib import Path
import sys

SCHEMA = "rustred.heartbeat-derived-metrics.v1"
DEFAULT_WINDOW_SECONDS = 3600.0
DEFAULT_RETAINED_SECONDS = 7200.0
STALL_THRESHOLDS = (5.0, 20.0)
MAX_LINE_BYTES = 64 * 1024 * 1024


def number(value):
    """Finite int/float or None; booleans are never numbers here."""
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, float) and math.isfinite(value):
        return value
    return None


def nonnegative_int(value):
    return value if type(value) is int and value >= 0 else None


def _mapping(value):
    return value if isinstance(value, dict) else {}


def sample_from_record(record, fallback_elapsed=None):
    """One heartbeat sample, or None when the record carries no walk counters."""
    if not isinstance(record, dict) or record.get("event") != "heartbeat":
        return None
    progress = _mapping(record.get("progress"))
    counters = _mapping(progress.get("snapshot", progress))
    completed = nonnegative_int(counters.get("completed_nodes"))
    if completed is None:
        return None
    elapsed = number(record.get("elapsed_seconds"))
    if elapsed is None:
        elapsed = fallback_elapsed
    if elapsed is None or elapsed < 0:
        return None
    parallel = _mapping(counters.get("parallel"))
    admission = _mapping(parallel.get("admission_preparation"))
    preparation = number(admission.get("preparation_wall_seconds"))
    commit = number(admission.get("ordered_commit_wall_seconds"))
    coordinator = None if preparation is None or commit is None else preparation + commit
    duty = _mapping(parallel.get("coordinator_duty"))
    duty_numbers = {key: value for key, value in duty.items()
                    if number(value) is not None and number(value) >= 0}
    closure = _mapping(counters.get("descendant_closure"))
    roots_closed = roots_total = None
    if closure.get("available") is True:
        roots_closed = nonnegative_int(closure.get("initial_closed"))
        roots_total = nonnegative_int(closure.get("initial_total"))
    discovered = nonnegative_int(record.get("currently_discovered_nodes"))
    if discovered is None:
        discovered = nonnegative_int(counters.get("scheduled_nodes"))
    phase_wall = _mapping(_mapping(counters.get("telemetry")).get("phase_wall_seconds"))
    checkpoint_seconds = number(phase_wall.get("checkpoint"))
    phase_seconds = (sum(phase_wall.values()) if checkpoint_seconds is not None and
                     all(number(value) is not None and value >= 0 for value in phase_wall.values()) else None)
    return {
        "elapsed": float(elapsed),
        "completed": completed,
        "scheduled": nonnegative_int(counters.get("scheduled_nodes")),
        "pending": nonnegative_int(counters.get("queued_nodes")),
        "discovered": discovered,
        "rss": nonnegative_int(record.get("process_rss_bytes")),
        "coordinator_seconds": coordinator,
        "coordinator_duty": duty_numbers or None,
        "computing": number(parallel.get("computing_workers")),
        # Epoch explicitly distinguishes unobservable activity from a lean
        # legacy event that simply omits the detailed worker fields.
        "computing_explicit": isinstance(parallel.get("activity_observation"), str),
        "active": number(parallel.get("active_workers")),
        "max_rank": nonnegative_int(counters.get("max_scheduled_finite_rank")),
        "checkpoint_phase_seconds": checkpoint_seconds,
        "coordinator_phase_seconds": phase_seconds,
        "roots_closed": roots_closed,
        "roots_total": roots_total,
    }


def checkpoint_save_from_record(record):
    """A completed checkpoint save described by this record, or None."""
    if not isinstance(record, dict):
        return None
    checkpoint = None
    if record.get("event") == "checkpoint_saved":
        checkpoint = record.get("checkpoint")
    elif record.get("event") == "heartbeat":
        progress = _mapping(record.get("progress"))
        counters = _mapping(progress.get("snapshot", progress))
        checkpoint = counters.get("checkpoint", progress.get("checkpoint"))
    if not isinstance(checkpoint, dict) or checkpoint.get("state") != "saved":
        return None
    generation = nonnegative_int(checkpoint.get("generation"))
    if generation is None:
        return None
    duration = number(checkpoint.get("duration_seconds"))
    return {"generation": generation,
            "bytes": nonnegative_int(checkpoint.get("bytes")),
            "duration_seconds": None if duration is None or duration < 0 else float(duration),
            "bootstrap": checkpoint.get("bootstrap") is True}


def _ratio(numerator, denominator):
    if numerator is None or denominator is None or denominator <= 0:
        return None
    return numerator / denominator


class HeartbeatWindow:
    """Bounded deque of recent heartbeat samples plus the checkpoint-save ledger."""

    def __init__(self, window_seconds=DEFAULT_WINDOW_SECONDS,
                 retained_seconds=DEFAULT_RETAINED_SECONDS, stall_thresholds=STALL_THRESHOLDS):
        if not (0 < window_seconds <= retained_seconds):
            raise ValueError("window must be positive and no longer than the retained span")
        self.window_seconds = float(window_seconds)
        self.retained_seconds = float(retained_seconds)
        self.stall_thresholds = tuple(float(threshold) for threshold in stall_thresholds)
        self.samples = deque()
        self.saves = {}
        self.last_elapsed = None
        self.records = 0
        self.samples_seen = 0
        self.completion_segment_start = None
        self.completion_last_sample = None
        self.closure_samples = deque()
        self.closure_state = "missing_closure"
        self.closure_reset_reason = None
        self.closure_segment_start = None
        self.closure_last_elapsed = None

    def _observe_closure(self, record):
        """An independent segment: never change the historical pending metric."""
        if not isinstance(record, dict) or record.get("event") != "heartbeat":
            return
        progress = _mapping(record.get("progress"))
        counters = _mapping(progress.get("snapshot", progress))
        closure = _mapping(counters.get("descendant_closure"))
        elapsed = number(record.get("elapsed_seconds"))
        total = nonnegative_int(closure.get("total_domains"))
        closed = nonnegative_int(closure.get("total_closed"))
        reason = None
        if elapsed is None or elapsed < 0:
            reason = "invalid_time"
        elif self.closure_last_elapsed is not None and elapsed <= self.closure_last_elapsed:
            reason = "nonmonotonic_time"
        self.closure_last_elapsed = elapsed if elapsed is not None and elapsed >= 0 else None
        if reason is not None:
            self.closure_samples.clear()
            self.closure_segment_start = None
            self.closure_state = self.closure_reset_reason = reason
        if reason == "invalid_time":
            return
        # Optional/lean telemetry is not a reset. Keep prior trustworthy pairs,
        # but do not present a carried-forward rate as a current observation.
        if closure.get("available") is not True:
            self.closure_state = "missing_current" if not closure else "unavailable_current"
            return
        if total is None or closed is None or closed > total:
            self.closure_samples.clear()
            self.closure_segment_start = None
            self.closure_state = self.closure_reset_reason = "invalid_counts"
            return
        age = number(closure.get("snapshot_age_seconds"))
        progress_age = number(record.get("progress_age_seconds"))
        sample = {"elapsed": float(elapsed), "total": total, "closed": closed,
                  "snapshot_revision": nonnegative_int(closure.get("snapshot_revision")),
                  "refresh_count": nonnegative_int(closure.get("refresh_count")),
                  "snapshot_stale": closure.get("snapshot_stale") if type(closure.get("snapshot_stale")) is bool else None,
                  "snapshot_age_seconds": None if age is None or age < 0 else age + max(0, progress_age or 0)}
        if self.closure_samples:
            previous = self.closure_samples[-1]
            if elapsed <= previous["elapsed"]:
                reason = "nonmonotonic_time"
            elif total < previous["total"] or closed < previous["closed"]:
                reason = "counter_reset"
            elif any(sample[key] is not None and previous[key] is not None
                     and sample[key] < previous[key] for key in ("snapshot_revision", "refresh_count")):
                reason = "snapshot_reset"
            if reason is not None:
                self.closure_samples.clear()
                self.closure_segment_start = None
                self.closure_reset_reason = reason
        if self.closure_segment_start is None:
            self.closure_segment_start = float(elapsed)
        self.closure_samples.append(sample)
        while elapsed - self.closure_samples[0]["elapsed"] > self.retained_seconds:
            self.closure_samples.popleft()
        self.closure_state = "valid"

    def _closure_derived(self, now, window):
        """Observed conservative-gap trend, not an instantaneous closure rate.

        Both counts come from one telemetry object. Its total is current, but
        closed is a conservative last-scan count; repeated scans are disclosed.
        No interpolation invents a closure update at the rolling boundary.
        """
        selected = [] if now is None else [sample for sample in self.closure_samples
                                          if now - window <= sample["elapsed"] <= now]
        result = {"per_second": None, "covered_seconds": None, "window_seconds": window,
                  "discovered_delta": None, "closed_delta": None,
                  "discovered_per_second": None, "closed_per_second": None,
                  "first_elapsed_seconds": None, "last_elapsed_seconds": None,
                  "samples": len(selected), "warmup": True, "state": self.closure_state,
                  "reset_reason": self.closure_reset_reason, "snapshot_advanced": None,
                  "snapshot_stale": None, "snapshot_age_seconds": None,
                  "scope": "observed conservative-gap trend: delta(total_domains-total_closed)/seconds; as of last heartbeat, not instantaneous closure throughput"}
        if not selected:
            if self.closure_state == "valid":
                result["state"] = "no_recent_samples"
            return result
        first, last = selected[0], selected[-1]
        span = last["elapsed"] - first["elapsed"]
        result.update(covered_seconds=span,
                      first_elapsed_seconds=first["elapsed"], last_elapsed_seconds=last["elapsed"],
                      state=self.closure_state if self.closure_state != "valid" else "valid" if span > 0 else "warmup",
                      snapshot_stale=last["snapshot_stale"],
                      snapshot_age_seconds=None if last["snapshot_age_seconds"] is None else
                      last["snapshot_age_seconds"] + max(0, now - last["elapsed"]))
        result["warmup"] = self.closure_segment_start is None or now - self.closure_segment_start < window
        if span > 0:
            result.update(discovered_delta=last["total"] - first["total"],
                          closed_delta=last["closed"] - first["closed"],
                          discovered_per_second=(last["total"] - first["total"]) / span,
                          closed_per_second=(last["closed"] - first["closed"]) / span)
            result["per_second"] = ((last["total"] - first["total"]) -
                                    (last["closed"] - first["closed"])) / span
            changes = [last[key] > first[key] for key in ("snapshot_revision", "refresh_count")
                       if first[key] is not None and last[key] is not None]
            result["snapshot_advanced"] = any(changes) if changes else None
        return result

    def observe(self, record):
        self.records += 1
        self._observe_closure(record)
        elapsed = number(record.get("elapsed_seconds")) if isinstance(record, dict) else None
        if elapsed is not None and elapsed >= 0:
            self.last_elapsed = float(elapsed)
        sample = sample_from_record(record, self.last_elapsed)
        if sample is not None:
            previous = self.completion_last_sample
            if (self.completion_segment_start is None or previous is not None and
                    (sample["elapsed"] <= previous["elapsed"] or sample["completed"] < previous["completed"])):
                self.completion_segment_start = sample["elapsed"]
            self.completion_last_sample = sample
            self.samples_seen += 1
            self.samples.append(sample)
            while self.samples and sample["elapsed"] - self.samples[0]["elapsed"] > self.retained_seconds:
                self.samples.popleft()
        save = checkpoint_save_from_record(record)
        if save is not None:
            known = self.saves.get(save["generation"])
            if known is None:
                save["elapsed"] = self.last_elapsed
                self.saves[save["generation"]] = save
            elif known["duration_seconds"] is None and save["duration_seconds"] is not None:
                known.update(duration_seconds=save["duration_seconds"], bytes=save["bytes"])
            elif known["bytes"] is None and save["bytes"] is not None:
                known["bytes"] = save["bytes"]

    def derived(self, now=None, session_start=0.0, window_seconds=None):
        """Measured rates over [now - window, now]; checkpoint duty over [session_start, now]."""
        window = self.window_seconds if window_seconds is None else float(window_seconds)
        if now is None:
            now = self.samples[-1]["elapsed"] if self.samples else self.last_elapsed
        result = {
            "schema": SCHEMA,
            "window_seconds": window,
            "retained_seconds": self.retained_seconds,
            "session_start_seconds": session_start,
            "now_seconds": now,
            "samples_in_window": 0,
            "window_wall_seconds": None,
            "first_elapsed_seconds": None,
            "last_elapsed_seconds": None,
            "completions_per_hour_1h": None,
            "completions_delta_1h": None,
            "completions_warmup_1h": now is None or self.completion_segment_start is None or now - self.completion_segment_start < window,
            "completions_window_valid_1h": False,
            "stall_share_5s": None,
            "stall_share_20s": None,
            "pending_growth_per_completion_1h": None,
            "rss_bytes_per_discovered_domain": None,
            "coordinator_duty_1h": None,
            "coordinator_duty_breakdown_1h": None,
            "checkpoint_duty": None,
            "checkpoint_duty_scope": "completed saves / observed invocation time",
            "checkpoint_saves_counted": 0,
            "checkpoint_save_seconds": 0.0,
            "computing_inspectors_mean_1h": None,
            "computing_workers": None,
            "active_workers": None,
            "max_scheduled_finite_rank": None,
            "roots_closed": None,
            "roots_total": None,
            "completed_nodes": None,
            "pending_nodes": None,
            "discovered_nodes": None,
            "process_rss_bytes": None,
            "last_checkpoint": None,
            "scope": "measured heartbeat deltas; not an ETA, closure fraction or family certificate",
            "discovery_closure_net_1h": self._closure_derived(now, window),
        }
        if now is None:
            return result
        selected = [sample for sample in self.samples
                    if now - window <= sample["elapsed"] <= now]
        if selected:
            first, last = selected[0], selected[-1]
            wall = last["elapsed"] - first["elapsed"]
            # A heartbeat journals the latest native event. Per-domain events
            # (domain_started/domain_delegated) keep the lean historical key
            # set without `computing_workers` or `coordinator_duty`; only the
            # periodic domain_progress events carry them. Use the latest (and
            # for the breakdown also the earliest) sample that has them.
            # An explicit Epoch observation is authoritative about availability:
            # stop here even when its current computing count is unknown.
            computing_now = next((sample["computing"] for sample in reversed(selected)
                                  if sample["computing"] is not None or sample["computing_explicit"]), None)
            result.update(samples_in_window=len(selected), window_wall_seconds=wall,
                          first_elapsed_seconds=first["elapsed"], last_elapsed_seconds=last["elapsed"],
                          completed_nodes=last["completed"], pending_nodes=last["pending"],
                          discovered_nodes=last["discovered"], process_rss_bytes=last["rss"],
                          max_scheduled_finite_rank=last["max_rank"],
                          roots_closed=last["roots_closed"], roots_total=last["roots_total"],
                          computing_workers=computing_now, active_workers=last["active"])
            completions = last["completed"] - first["completed"]
            result["completions_delta_1h"] = completions
            result["completions_window_valid_1h"] = (wall > 0 and completions >= 0 and
                self.completion_segment_start is not None and first["elapsed"] >= self.completion_segment_start)
            if wall > 0:
                result["completions_per_hour_1h"] = completions / wall * 3600.0
                stalled = {threshold: 0.0 for threshold in self.stall_thresholds}
                for before, after in zip(selected, selected[1:]):
                    interval = after["elapsed"] - before["elapsed"]
                    if after["completed"] - before["completed"] <= 0:
                        for threshold in self.stall_thresholds:
                            if interval >= threshold:
                                stalled[threshold] += interval
                for threshold in self.stall_thresholds:
                    result[f"stall_share_{threshold:.0f}s"] = stalled[threshold] / wall
                if first["coordinator_seconds"] is not None and last["coordinator_seconds"] is not None:
                    delta = last["coordinator_seconds"] - first["coordinator_seconds"]
                    result["coordinator_duty_1h"] = delta / wall if delta >= 0 else None
                with_duty = [sample for sample in selected if sample["coordinator_duty"]]
                if len(with_duty) >= 2 and with_duty[-1]["elapsed"] > with_duty[0]["elapsed"]:
                    start_sample, end_sample = with_duty[0], with_duty[-1]
                    span = end_sample["elapsed"] - start_sample["elapsed"]
                    breakdown = {}
                    for key, value in end_sample["coordinator_duty"].items():
                        start = start_sample["coordinator_duty"].get(key)
                        if start is not None and value >= start:
                            breakdown[key] = (value - start) / span
                    result["coordinator_duty_breakdown_1h"] = breakdown or None
            if completions > 0 and first["pending"] is not None and last["pending"] is not None:
                result["pending_growth_per_completion_1h"] = (last["pending"] - first["pending"]) / completions
            result["rss_bytes_per_discovered_domain"] = _ratio(last["rss"], last["discovered"])
            computing = [sample["computing"] for sample in selected if sample["computing"] is not None]
            if computing:
                result["computing_inspectors_mean_1h"] = sum(computing) / len(computing)
        counted = [save for save in self.saves.values()
                   if save["duration_seconds"] is not None
                   and (save["elapsed"] is None or session_start <= save["elapsed"] <= now)]
        total = sum(save["duration_seconds"] for save in counted)
        result.update(checkpoint_saves_counted=len(counted), checkpoint_save_seconds=total,
                      checkpoint_duty=_ratio(total, now - session_start) if counted else None)
        if not counted:
            phase = next((sample for sample in reversed(self.samples)
                          if sample["coordinator_phase_seconds"] is not None), None)
            if phase is not None:
                result["checkpoint_duty"] = _ratio(phase["checkpoint_phase_seconds"], phase["coordinator_phase_seconds"])
                result["checkpoint_duty_scope"] = "native current-invocation coordinator phase wall"
        if self.saves:
            latest = self.saves[max(self.saves)]
            result["last_checkpoint"] = {name: latest[name] for name in
                                         ("generation", "bytes", "duration_seconds")}
        return result


def stream_events(path: Path):
    """Yield (record, partial_last_line, invalid) per line; bounded, tolerant of a torn tail."""
    with Path(path).open("rb") as stream:
        for line in stream:
            complete = line.endswith(b"\n")
            piece = line.strip()
            if not piece:
                continue
            if len(piece) > MAX_LINE_BYTES:
                yield None, False, True
                continue
            try:
                record = json.loads(piece)
            except ValueError:
                yield None, not complete, complete
                continue
            if not isinstance(record, dict):
                yield None, False, True
                continue
            yield record, False, False


def compute(path: Path, start=None, end=None, window=None, retained=None) -> dict:
    """Derived metrics for the elapsed window [start, end] of one events.jsonl."""
    start = 0.0 if start is None else float(start)
    if end is not None and end < start:
        raise ValueError("end must not precede start")
    if window is None:
        window = (end - start) if end is not None and end > start else DEFAULT_WINDOW_SECONDS
    retained = max(window, DEFAULT_RETAINED_SECONDS if retained is None else float(retained))
    metrics = HeartbeatWindow(window_seconds=window, retained_seconds=retained)
    counts = {"records": 0, "invalid_records": 0, "partial_last_line": False,
              "records_before_start": 0, "records_after_end": 0}
    last_elapsed = None
    for record, partial, invalid in stream_events(path):
        if partial:
            counts["partial_last_line"] = True
            continue
        if invalid:
            counts["invalid_records"] += 1
            continue
        counts["records"] += 1
        elapsed = number(record.get("elapsed_seconds"))
        position = last_elapsed if elapsed is None else elapsed
        if elapsed is not None:
            last_elapsed = elapsed
        if position is not None and position < start:
            counts["records_before_start"] += 1
            continue
        if end is not None and position is not None and position > end:
            counts["records_after_end"] += 1
            continue
        metrics.observe(record)
    now = end if end is not None else metrics.last_elapsed
    derived = metrics.derived(now=now, session_start=start, window_seconds=window)
    derived.update(events_file=str(path), start_seconds=start, end_seconds=end, **counts)
    return derived


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("events", type=Path, help="native events.jsonl (may still be growing)")
    parser.add_argument("--start", type=float, default=None, help="window start in native elapsed seconds (default 0)")
    parser.add_argument("--end", type=float, default=None, help="window end in native elapsed seconds (default: last record)")
    parser.add_argument("--window", type=float, default=None,
                        help="rate window in seconds ending at --end (default: end-start when both given, else 3600)")
    parser.add_argument("--indent", type=int, default=2)
    args = parser.parse_args(argv)
    for name in ("start", "end", "window"):
        value = getattr(args, name)
        if value is not None and (not math.isfinite(value) or value < 0 or (name == "window" and value == 0)):
            parser.error(f"--{name} must be a finite nonnegative number" + (" and positive" if name == "window" else ""))
    try:
        result = compute(args.events, args.start, args.end, args.window)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    json.dump(result, sys.stdout, indent=args.indent, sort_keys=True, allow_nan=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
