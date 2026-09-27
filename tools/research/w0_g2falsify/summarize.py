#!/usr/bin/env python
"""Markdown A/B tables for the W0 G2' falsifier (read-only).

usage: summarize.py SHA [RUNS_DIR]
Reads RUNS_DIR/<label>/<family>/{metrics,g2stats,pending,audit}.json of the
session labels written by session.sh for binary SHA and prints: the per-run
table, the flag-on/flag-off ratio table (per repeat), and the per owner class
G2' table. Missing runs are skipped.
"""
import json
import sys
from pathlib import Path

SHA = sys.argv[1]
RUNS = Path(sys.argv[2] if len(sys.argv) > 2 else "/common/dev/rustred/TMP/w0/g2falsify/runs")
CLASSES = ("000011001001011", "011101110111000", "other")

PAIRS = []
for fam in ("fg", "bmw", "h", "x"):
    for r in ("r1", "r2"):
        PAIRS.append((f"C-4L {fam.upper()} Ordered W6 {r}", fam, f"c4l-off-{r}-{SHA}", f"c4l-on-{r}-{SHA}"))
for pol in ("ord", "rdy"):
    for r in ("r1", "r2"):
        name = {"ord": "Ordered", "rdy": "Ready"}[pol]
        PAIRS.append((f"C-5F {name} W24 {r}", "five-finite", f"c5f-{pol}-off-{r}-{SHA}", f"c5f-{pol}-on-{r}-{SHA}"))
for r in ("r1", "r2"):
    PAIRS.append((f"C-HOT-sub r1a12 Ready W12 {r}", "hot", f"hotsub-r1a12-off-{r}-{SHA}", f"hotsub-r1a12-on-{r}-{SHA}"))


def load(label, fam):
    d = RUNS / label / fam
    out = {}
    for name in ("metrics", "g2stats", "pending", "audit"):
        p = d / f"{name}.json"
        if p.exists() and p.stat().st_size:
            try:
                out[name] = json.load(open(p))
            except json.JSONDecodeError:
                out[name] = {}
        else:
            out[name] = {}
    return out if out["metrics"] else None


def fnum(x, fmt="{:,.0f}"):
    return "-" if x is None else fmt.format(x)


def rec_seconds(g):
    return sum(v["record_seconds"] for v in g.get("native_by_phase", {}).values()) if g else None


def native_calls(g):
    if not g:
        return None, None
    c = g["apply_by_owner_class"]
    apply_calls = sum(c[k]["native_calls"] for k in CLASSES)
    route = g["native_by_phase"].get("Route", {}).get("records", 0)
    return apply_calls, route


def row(name, arm, r):
    m, g, p, a = r["metrics"], r["g2stats"], r["pending"], r["audit"]
    ap, rt = native_calls(g)
    rec = m.get("recorder", {})
    closure = m.get("closure") or {}
    g2 = (m.get("w0_g2_donly") or {}).get("by_owner_class", {})
    plan_s = sum(v.get("plan_seconds", 0) for v in g2.values()) if g2 else None
    return (f"| {name} | {arm} | {m.get('exit_code')} / {m.get('status')} | {m.get('frontiers')} | "
            f"{closure.get('initial_closed')}/{closure.get('initial_total')} | {fnum(ap)} / {fnum(rt)} | "
            f"{fnum(rec_seconds(g), '{:,.1f}')} | {fnum(plan_s, '{:,.1f}')} | {fnum(m.get('slot_busy_seconds_sum'), '{:,.1f}')} | "
            f"{fnum(float(m['traversal_seconds']) if m.get('traversal_seconds') else None, '{:,.1f}')} | "
            f"{fnum(int(m['scheduled_nodes']) if m.get('scheduled_nodes') else None)} | {fnum(p.get('peak_pending_domains'))} | "
            f"{fnum(p.get('pending_growth_per_completion_slope_to_peak'), '{:.3f}')} | "
            f"{fnum(p.get('discovered_domains_per_native'), '{:.3f}')} | {a.get('audit', '-')} | "
            f"{fnum(rec.get('foreign_busy_cpus_mean'), '{:.1f}')} | {fnum(rec.get('schedstat_run_delay_seconds'), '{:,.0f}')} | "
            f"{m.get('started_utc')} |")


def main():
    print("| Control | Arm | Exit / status | Frontiers | Roots closed | Native calls Apply / Route | "
          "Inspector record-s | of which G2' plan-s | Slot-busy s | Traversal s | Scheduled domains | Peak pending | "
          "Pending growth / completion (slope to peak) | Discovered / native | Audit | Foreign busy CPUs (mean) | Run delay s | Start (UTC) |")
    print("|---|---|---|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---|")
    loaded = []
    for name, fam, off, on in PAIRS:
        a, b = load(off, fam), load(on, fam)
        if not a or not b:
            continue
        loaded.append((name, a, b))
        print(row(name, "off", a))
        print(row(name, "G2'", b))
    print()
    print("| Control | record-s on/off | slot-busy on/off | traversal on/off | Apply native calls on/off | "
          "Route natives on/off | scheduled domains on/off | peak pending on/off | discovered/native on/off |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for name, a, b in loaded:
        def rat(f):
            try:
                x, y = f(b), f(a)
                return f"{x / y:.3f}" if x is not None and y else "-"
            except (KeyError, TypeError, ValueError):
                return "-"
        print(f"| {name} | {rat(lambda r: rec_seconds(r['g2stats']))} | "
              f"{rat(lambda r: float(r['metrics']['slot_busy_seconds_sum']))} | "
              f"{rat(lambda r: float(r['metrics']['traversal_seconds']))} | "
              f"{rat(lambda r: native_calls(r['g2stats'])[0])} | {rat(lambda r: native_calls(r['g2stats'])[1])} | "
              f"{rat(lambda r: int(r['metrics']['scheduled_nodes']))} | "
              f"{rat(lambda r: r['pending']['peak_pending_domains'])} | "
              f"{rat(lambda r: r['pending']['discovered_domains_per_native'])} |")
    print()
    print("| Control | Owner class | Apply records off / on | G2' residual share | G2' full-cover share | "
          "residual point fraction (planned) | Apply record-s off / on (ratio) | distinct pts off / on | new pts per native call off / on |")
    print("|---|---|---|---:|---:|---:|---|---|---|")
    for name, a, b in loaded:
        for k in CLASSES:
            ca = a["g2stats"].get("apply_by_owner_class", {}).get(k)
            cb = b["g2stats"].get("apply_by_owner_class", {}).get(k)
            if not ca or not cb or not ca["apply_records_inspected_or_planned"]:
                continue
            sa, sb = ca["apply_record_seconds"], cb["apply_record_seconds"]
            print(f"| {name} | {k} | {ca['apply_records_inspected_or_planned']:,} / {cb['apply_records_inspected_or_planned']:,} | "
                  f"{cb['share_g2_residual']:.3f} | {cb['share_g2_full_cover']:.3f} | {cb['planned_residual_point_fraction']:.3f} | "
                  f"{sa:,.1f} / {sb:,.1f} ({(sb / sa if sa else float('nan')):.3f}) | "
                  f"{fnum(ca.get('distinct_inspected_points'))} / {fnum(cb.get('distinct_inspected_points'))} | "
                  f"{fnum(ca.get('distinct_points_per_native_call'), '{:.1f}')} / {fnum(cb.get('distinct_points_per_native_call'), '{:.1f}')} |")


if __name__ == "__main__":
    main()
