#!/usr/bin/env python
"""Summarize W0.8 knob runs (knob_run.py output directories).

For every <runs>/<label>/<family>/metrics.json prints one row with the order
metrics (mistakes and transfers per native, peak pending, natives, wall,
inspector CPU per native, foreign load) and the root-certification trajectory
against cumulative inspector CPU (events.jsonl heartbeats x cpu.jsonl):
  cpu50/cpu90: inspector CPU-seconds when >=50%/90% of initial roots were
               certified (closure monitor snapshot, conservative);
  auc:         mean certified-root fraction over [0, C] of inspector CPU, with
               C the reference arm's total inspector CPU (--reference label).

Usage: summarize.py [--runs DIR] [--reference LABEL] [--json OUT] LABEL_GLOB...
"""
import argparse
import bisect
import fnmatch
import json
import math
import statistics
from pathlib import Path

RUNS = Path("/common/dev/rustred/TMP/w0/knobs/runs")


def load_series(run):
    cpu = []
    path = run / "cpu.jsonl"
    if path.exists():
        for line in path.read_text().splitlines():
            try:
                row = json.loads(line)
            except ValueError:
                continue
            cpu.append((row["t"], row["inspector_cpu_seconds"]))
    beats = []
    path = run / "events.jsonl"
    if path.exists():
        with open(path) as f:
            for line in f:
                if '"heartbeat"' not in line[:200] and '"event":"heartbeat"' not in line:
                    continue
                try:
                    e = json.loads(line)
                except ValueError:
                    continue
                if e.get("event") != "heartbeat":
                    continue
                p = e.get("progress", {})
                dc = p.get("descendant_closure") or {}
                if not dc.get("available"):
                    continue
                order = (p.get("parallel") or {}).get("dispatch_order") or {}
                beats.append({"t": e["elapsed_seconds"],
                              "closed": dc.get("initial_closed"),
                              "total": dc.get("initial_total"),
                              "domains_closed": dc.get("total_closed"),
                              "queued": p.get("queued_nodes"),
                              "natives": p.get("native_processed_nodes") or p.get("completed_nodes"),
                              "mistakes": order.get("mistakes")})
    return cpu, beats


def cpu_at(cpu, t):
    if not cpu:
        return None
    times = [c[0] for c in cpu]
    i = bisect.bisect_left(times, t)
    if i <= 0:
        return cpu[0][1] * (t / cpu[0][0]) if cpu[0][0] > 0 else cpu[0][1]
    if i >= len(cpu):
        return cpu[-1][1]
    (t0, c0), (t1, c1) = cpu[i - 1], cpu[i]
    return c0 + (c1 - c0) * (t - t0) / (t1 - t0) if t1 > t0 else c1


def trajectory(cpu, beats, final_closed, total, final_cpu):
    points = [(0.0, 0)]
    for b in beats:
        if b["closed"] is None:
            continue
        c = cpu_at(cpu, b["t"])
        if c is not None:
            points.append((c, b["closed"]))
    if final_closed is not None and final_cpu is not None:
        points.append((final_cpu, final_closed))
    points.sort()
    # closure snapshots are conservative lower bounds; keep the running max
    best, mono = 0, []
    for c, k in points:
        best = max(best, k)
        mono.append((c, best))
    return mono


def first_cpu(mono, target):
    for c, k in mono:
        if k >= target:
            return c
    return None


def auc(mono, horizon, total):
    if not mono or not total or not horizon:
        return None
    area, prev_c, prev_k = 0.0, 0.0, 0
    for c, k in mono:
        c = min(c, horizon)
        if c > prev_c:
            area += prev_k * (c - prev_c)
            prev_c = c
        prev_k = k
        if c >= horizon:
            break
    if prev_c < horizon:
        area += prev_k * (horizon - prev_c)
    return area / (horizon * total)


def row(run, horizon=None):
    m = json.loads((run / "metrics.json").read_text())
    order = m.get("dispatch_order") or {}
    natives = m.get("native_publications") or m.get("native_processed_nodes")
    cpu, beats = load_series(run)
    total = m.get("initial_total")
    mono = trajectory(cpu, beats, m.get("initial_closed"), total, m.get("inspector_cpu_seconds"))
    peak_queued = max((b["queued"] or 0 for b in beats), default=None)
    out = {
        "label": m["label"], "family": m["family"], "exit": m["exit_code"],
        "timed_out": m.get("timed_out"),
        "order": order.get("order") or (m.get("env") or {}).get("RUSTRED_WALK_DISPATCH_ORDER", "fifo"),
        "natives": natives, "transfers": m.get("transferred_obligations"),
        "scheduled": m.get("scheduled_nodes"), "frontiers": m.get("frontiers"),
        "retired": m.get("containment_retired_candidates"),
        "mistakes": order.get("mistakes"),
        "mistakes_per_native": order.get("mistakes_per_native"),
        "transfers_per_native": (m.get("transferred_obligations") / natives) if natives and m.get("transferred_obligations") is not None else None,
        "retired_protected": order.get("retired_protected_initial"),
        "peak_pending": order.get("peak_pending"), "peak_queued_heartbeat": peak_queued,
        "peak_unreserved": order.get("peak_unreserved"),
        "aged_reservations": order.get("aged_reservations"),
        "wall": m.get("whole_command_seconds"), "traversal": m.get("traversal_seconds"),
        "inspector_cpu": m.get("inspector_cpu_seconds"),
        "inspector_ms_per_native": m.get("inspector_cpu_ms_per_native"),
        "inspector_run_delay": m.get("inspector_run_delay_seconds"),
        "foreign_load": m.get("foreign_load_fraction"),
        "peak_rss_gb": (m.get("peak_rss_bytes") or 0) / 1e9,
        "roots_closed": m.get("initial_closed"), "roots_total": total,
        "cpu50": first_cpu(mono, math.ceil(0.5 * total)) if total else None,
        "cpu90": first_cpu(mono, math.ceil(0.9 * total)) if total else None,
        "roots_per_cpu_hour": (m.get("initial_closed") or 0) / (m["inspector_cpu_seconds"] / 3600) if m.get("inspector_cpu_seconds") else None,
        "all_discharged": m.get("all_ledger_obligations_discharged"),
        "boost": order.get("closure_boost"),
        "env": m.get("env"),
    }
    if horizon:
        out["auc"] = auc(mono, horizon, total)
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--runs", default=str(RUNS))
    p.add_argument("--reference", help="label glob of the reference (FIFO) arm for the AUC horizon")
    p.add_argument("--json")
    p.add_argument("labels", nargs="+")
    args = p.parse_args()
    runs_dir = Path(args.runs)
    runs = sorted(r for r in runs_dir.glob("*/*") if (r / "metrics.json").exists()
                  and any(fnmatch.fnmatch(r.parent.name, g) for g in args.labels))
    horizons = {}
    if args.reference:
        for r in runs:
            if fnmatch.fnmatch(r.parent.name, args.reference):
                m = json.loads((r / "metrics.json").read_text())
                horizons.setdefault(r.name, []).append(m.get("inspector_cpu_seconds"))
    rows = [row(r, statistics.mean(horizons[r.name]) if r.name in horizons else None) for r in runs]
    cols = ("family", "label", "order", "exit", "natives", "transfers_per_native", "mistakes_per_native",
            "peak_pending", "wall", "inspector_cpu", "inspector_ms_per_native", "cpu50", "cpu90",
            "auc", "foreign_load", "frontiers", "roots_closed", "roots_total")
    print("| " + " | ".join(cols) + " |")
    print("|" + "---|" * len(cols))
    for r in rows:
        def fmt(v):
            if isinstance(v, float):
                return f"{v:.4g}"
            return str(v)
        print("| " + " | ".join(fmt(r.get(c)) for c in cols) + " |")
    if args.json:
        json.dump(rows, open(args.json, "w"), indent=1)


if __name__ == "__main__":
    main()
