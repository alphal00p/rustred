#!/usr/bin/env python
"""Markdown A/B tables for the W0.9 G1 falsifier from run_arm.py metrics.json
and walkstats.json (read-only). usage: summarize.py [RUNS_DIR]"""
import json
import sys
from pathlib import Path

RUNS = Path(sys.argv[1] if len(sys.argv) > 1 else "/common/dev/rustred/TMP/w0/falsify/runs")

PAIRS = [
    # (control, family, exact label, widened label)
    ("C-4L FG r1", "fg", "exact-bce6772f", "widen-all-bce6772f"),
    ("C-4L BMW r1", "bmw", "exact-bce6772f", "widen-all-bce6772f"),
    ("C-4L H r1", "h", "exact-bce6772f", "widen-all-bce6772f"),
    ("C-4L X r1", "x", "exact-bce6772f", "widen-all-bce6772f"),
    ("C-4L FG r2", "fg", "exact-bce6772f-rep2", "widen-all-bce6772f-rep2"),
    ("C-4L BMW r2", "bmw", "exact-bce6772f-rep2", "widen-all-bce6772f-rep2"),
    ("C-4L H r2", "h", "exact-bce6772f-rep2", "widen-all-bce6772f-rep2"),
    ("C-4L X r2", "x", "exact-bce6772f-rep2", "widen-all-bce6772f-rep2"),
    ("C-5F", "five-finite", "exact-bce6772f", "widen-all-bce6772f"),
    ("C-HOT-sub (r1a12)", "hot", "exact-bce6772f-hotsub-r1a12-w12", "widen-all-bce6772f-hotsub-r1a12-w12"),
    ("C-HOT-sub2 (r2a11)", "hot", "exact-bce6772f-hotsub-r2a11-w12", "widen-all-bce6772f-hotsub-r2a11-w12"),
    ("C-HOT-sub (r1a12), hot owner not widened", "hot", "exact-bce6772f-hotsub-r1a12-w12", "widen-exclhot-bce6772f-hotsub-r1a12-w12"),
]
HOT = "000011001001011"


def max_rank(path):
    import re
    size = path.stat().st_size
    with open(path, "rb") as f:
        f.seek(max(0, size - 4_000_000))
        tail = f.read().decode("utf-8", "replace")
    mm = re.search(r'\n  "max_scheduled_finite_rank": ([0-9]+|null)', tail)
    return mm.group(1) if mm else None


def load(label, fam):
    d = RUNS / label / fam
    m = json.load(open(d / "metrics.json"))
    m["max_scheduled_finite_rank"] = max_rank(d / "result.json")
    w = json.load(open(d / "walkstats.json")) if (d / "walkstats.json").exists() else None
    return m, w


def rec_seconds(w):
    return sum(v["record_seconds"] for v in w["native_by_phase"].values())


def owner_seconds(w, owner):
    for r in w["apply_owner_seconds_top"]:
        if r["owner"] == owner:
            return r["inspections"], r["record_seconds"]
    return None, None


def main():
    print("| Control | Arm | Exit / status | Frontiers | Roots closed | Natives (Apply / Route) | Record-s (all natives) | Slot-busy s | Traversal s | Max rank | Hot-owner insp / s | Start (UTC) |")
    print("|---|---|---|---:|---|---|---:|---:|---:|---:|---|---|")
    ratios = []
    for name, fam, ex, wi in PAIRS:
        try:
            rows = [("exact", *load(ex, fam)), ("G1", *load(wi, fam))]
        except FileNotFoundError:
            continue
        vals = {}
        for arm, m, w in rows:
            nb = w["native_by_phase"]
            ap = nb.get("Apply", {}).get("inspections", 0)
            rt = nb.get("Route", {}).get("inspections", 0)
            rs = rec_seconds(w)
            hi, hs = owner_seconds(w, HOT)
            vals[arm] = (rs, float(m["slot_busy_seconds_sum"]), float(m["traversal_seconds"]))
            print(f"| {name} | {arm} | {m['exit_code']} / {m.get('status')} | {m['frontiers']} | "
                  f"{m.get('initial_closed')}/{m.get('initial_total')} | {ap:,} / {rt:,} | {rs:,.1f} | "
                  f"{float(m['slot_busy_seconds_sum']):,.1f} | {float(m['traversal_seconds']):,.1f} | "
                  f"{m.get('max_scheduled_finite_rank')} | "
                  f"{'-' if hi is None else f'{hi:,} / {hs:,.1f}'} | {m['started_utc']} |")
        r = tuple(vals["G1"][i] / vals["exact"][i] for i in range(3))
        ratios.append((name, r))
    print()
    print("| Control | G1/exact record-s | G1/exact slot-busy s | G1/exact traversal |")
    print("|---|---:|---:|---:|")
    for name, r in ratios:
        print(f"| {name} | {r[0]:.3f} | {r[1]:.3f} | {r[2]:.3f} |")


if __name__ == "__main__":
    main()
