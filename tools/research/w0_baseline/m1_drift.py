#!/usr/bin/env python
"""Admission drift of a walk from its native events.jsonl (M1 run or live campaign run).

Reads only the `domain_progress` heartbeats (read-only; a live campaign's events.jsonl may be
passed) and reports, per window, the admission-work counters that decide the coordinator's cost:

  spec_checks_per_request   speculative containment checks per admission request (helpers)
  commit_checks_per_request persisted containment_checks per request (committed path)
  maint_checks_per_request  persisted containment_maintenance_checks per request (reverse/retire)
  callbacks_per_request     coordinator prefilter callbacks (forward + reverse) per request
  commit_ns_per_check       ordered-commit wall per persisted containment check
  commit_us_per_record      ordered-commit wall per prepared record
  successors_per_committed  admission requests per committed domain (work mix signature)
  natives/obligations/discovered per hour, apply share and dominant owner of the heartbeats

Windows: `full` [T, end], 5-min slices, 60-s bins, hourly (long runs) and the last 720 s.
T is the first traversal heartbeat; end is the last heartbeat at which committed_domains changed,
or T + --max-seconds.  Counters are the native's own; elapsed_seconds is the native clock.

Usage: m1_drift.py EVENTS --label L --out OUT.json [--max-seconds S] [--tail-seconds S]
"""
import argparse
import collections
import json
import statistics
from pathlib import Path

AP = "parallel.admission_preparation."
PF = "parallel.containment_prefilter."
FIELDS = [
    "committed_domains", "native_processed_nodes", "containment_checks", "containment_maintenance_checks",
    "containment_candidates", "containment_retired_candidates", "successors", "owner", "phase",
    AP + "speculative_admission_requests", AP + "speculative_containment_checks",
    AP + "speculative_reverse_checks", AP + "prepared_batch_records", AP + "parallel_batches",
    AP + "ordered_commit_wall_seconds", AP + "preparation_wall_seconds",
    PF + "forward_callbacks", PF + "reverse_callbacks",
    "delegation.pending_native_publications", "descendant_closure.initial_closed",
]


def get(doc, path):
    for key in path.split("."):
        if not isinstance(doc, dict):
            return None
        doc = doc.get(key)
    return doc


def load(path):
    rows = []
    for line in open(path):
        if '"domain_progress"' not in line:
            continue
        try:
            doc = json.loads(line)
        except ValueError:
            continue
        progress = doc.get("progress") or {}
        if progress.get("event") != "domain_progress":
            continue
        row = {"t": doc.get("elapsed_seconds"), "rss": doc.get("process_rss_bytes"),
               "discovered": doc.get("currently_discovered_nodes")}
        for field in FIELDS:
            row[field] = get(progress, field)
        rows.append(row)
    return rows


def delta(a, b, key):
    if a.get(key) is None or b.get(key) is None:
        return None
    return b[key] - a[key]


def ratio(num, den, scale=1.0):
    return scale * num / den if num is not None and den else None


def metrics(rows):
    a, b = rows[0], rows[-1]
    dt = b["t"] - a["t"]
    req = delta(a, b, AP + "speculative_admission_requests")
    committed = delta(a, b, "committed_domains")
    commit_wall = delta(a, b, AP + "ordered_commit_wall_seconds")
    checks = delta(a, b, "containment_checks")
    callbacks = None
    if delta(a, b, PF + "forward_callbacks") is not None:
        callbacks = delta(a, b, PF + "forward_callbacks") + delta(a, b, PF + "reverse_callbacks")
    owners = collections.Counter(r["owner"] for r in rows)
    top_owner, top_count = owners.most_common(1)[0]
    return {
        "from_seconds": a["t"], "seconds": dt, "heartbeats": len(rows),
        "discovered_start": a["discovered"], "discovered_end": b["discovered"],
        "natives_per_hour": ratio(delta(a, b, "native_processed_nodes"), dt, 3600),
        "obligations_per_hour": ratio(committed, dt, 3600),
        "discovered_per_hour": ratio(delta(a, b, "discovered"), dt, 3600),
        "spec_checks_per_request": ratio(delta(a, b, AP + "speculative_containment_checks"), req),
        "spec_reverse_checks_per_request": ratio(delta(a, b, AP + "speculative_reverse_checks"), req),
        "commit_checks_per_request": ratio(checks, req),
        "maint_checks_per_request": ratio(delta(a, b, "containment_maintenance_checks"), req),
        "callbacks_per_request": ratio(callbacks, req),
        "commit_ns_per_check": ratio(commit_wall, checks, 1e9),
        "commit_us_per_record": ratio(commit_wall, delta(a, b, AP + "prepared_batch_records"), 1e6),
        "prep_us_per_batch": ratio(delta(a, b, AP + "preparation_wall_seconds"),
                                   delta(a, b, AP + "parallel_batches"), 1e6),
        "successors_per_committed": ratio(req, committed),
        "apply_share": sum(1 for r in rows if r["phase"] == "Apply") / len(rows),
        "owners_seen": len(owners), "top_owner": top_owner, "top_owner_share": top_count / len(rows),
        "candidates_end": b["containment_candidates"],
        "pending_native_delta": delta(a, b, "delegation.pending_native_publications"),
    }


def windows(rows, t0, t1, width, min_fraction=0.9):
    out, t = [], t0
    while t < t1:
        part = [r for r in rows if t <= r["t"] <= min(t + width, t1)]
        if len(part) >= 2 and part[-1]["t"] - part[0]["t"] >= min_fraction * min(width, t1 - t):
            out.append(metrics(part))
        t += width
    return out


def spread(items, keys):
    out = {}
    for key in keys:
        vals = sorted(x[key] for x in items if x.get(key) is not None)
        if vals:
            out[key] = {"n": len(vals), "min": vals[0], "p10": vals[int(0.1 * (len(vals) - 1))],
                        "median": statistics.median(vals), "p90": vals[int(0.9 * (len(vals) - 1))],
                        "max": vals[-1]}
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("events", type=Path)
    p.add_argument("--label", required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--max-seconds", type=float, help="end the measured interval at T + this")
    p.add_argument("--tail-seconds", type=float, help="report slices only over the last S seconds before end")
    args = p.parse_args()
    rows = load(args.events)
    if len(rows) < 2:
        raise SystemExit("no domain_progress heartbeats")
    t0 = rows[0]["t"]
    end = max(r["t"] for i, r in enumerate(rows[1:], 1)
              if r["committed_domains"] != rows[i - 1]["committed_domains"])
    if args.max_seconds is not None:
        end = min(end, t0 + args.max_seconds)
    start = max(t0, end - args.tail_seconds) if args.tail_seconds else t0
    measured = [r for r in rows if start <= r["t"] <= end]
    keys = ("natives_per_hour", "obligations_per_hour", "spec_checks_per_request", "commit_checks_per_request",
            "commit_ns_per_check", "commit_us_per_record", "successors_per_committed")
    slices = windows(measured, start, end, 300)
    doc = {
        "label": args.label, "events": str(args.events), "heartbeats": len(rows),
        "first": {k: rows[0][k] for k in ("t", "discovered", "committed_domains", "containment_candidates",
                                          "containment_retired_candidates", "rss")},
        "last": {k: rows[-1][k] for k in ("t", "discovered", "committed_domains", "containment_candidates",
                                          "containment_retired_candidates", "rss")},
        "interval": {"start_seconds": start, "end_seconds": end},
        "full": metrics(measured),
        "last_720s": metrics([r for r in measured if r["t"] >= end - 720]),
        "slices_300s": slices,
        "slices_300s_spread": spread(slices, keys),
        "bins_60s": windows(measured, start, end, 60),
    }
    if end - start > 2 * 3600:
        doc["hourly"] = windows(measured, start, end, 3600)
        # How much do two consecutive 12-min windows of the same process differ?  This is the
        # yardstick for a 12-min comparison across different stretches of the ID sequence.
        twelve = windows(measured, start, end, 720)
        adjacent = {}
        for key in ("spec_checks_per_request", "natives_per_hour", "commit_us_per_record"):
            vals = [w[key] for w in twelve]
            ratios = [vals[i + 1] / vals[i] for i in range(len(vals) - 1) if vals[i] and vals[i + 1]]
            if ratios:
                ordered = sorted(ratios)
                adjacent[key] = {"pairs": len(ratios), "min": ordered[0],
                                 "p10": ordered[int(0.1 * (len(ordered) - 1))],
                                 "median": statistics.median(ordered),
                                 "p90": ordered[int(0.9 * (len(ordered) - 1))], "max": ordered[-1],
                                 "share_factor_ge_1.48": sum(1 for r in ratios if r >= 1.48 or r <= 1 / 1.48)
                                 / len(ratios),
                                 "share_factor_ge_2.44": sum(1 for r in ratios if r >= 2.44 or r <= 1 / 2.44)
                                 / len(ratios)}
        doc["adjacent_720s_ratios"] = adjacent
    args.out.parent.mkdir(parents=True, exist_ok=True)
    json.dump(doc, open(args.out, "w"), indent=1)
    brief = {k: doc[k] for k in ("label", "first", "last", "interval", "full", "slices_300s_spread")}
    print(json.dumps(brief, indent=1))


if __name__ == "__main__":
    main()
