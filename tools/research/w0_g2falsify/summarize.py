#!/usr/bin/env python
"""Markdown A/B tables for the W0 G2' falsifier (read-only).

usage: summarize.py SHA [RUNS_DIR]
Reads RUNS_DIR/<label>/<family>/{metrics,g2stats,pending,audit}.json of the
session labels written by session.sh for binary SHA and prints: the per-run
table, the flag-on/flag-off ratio table (per repeat, each G2' arm against the
flag-off arm of the same repeat), and the per owner class G2' table. Arms:
off (flag unset), on (B1 binary, one anchor), m1 / m2 (B2 binary, one / up to
two anchors). Missing runs are skipped.
"""
import json
import sys
from pathlib import Path

SHA = sys.argv[1]
RUNS = Path(sys.argv[2] if len(sys.argv) > 2 else "/common/dev/rustred/TMP/w0/g2falsify/runs")
CLASSES = ("000011001001011", "011101110111000", "other")
ARMS = ("on", "m1", "m2", "u", "n")
REPS = ("r1", "r2", "r3", "r4")
ARM_NAME = {"off": "off", "on": "G2' 1 anchor", "m1": "G2' 1 anchor", "m2": "G2' <=2 anchors",
            "u": "G2' union per D level", "n": "G2' union, full-native anchors"}

# (control name, family, label template with {arm} and {r})
GROUPS = []
for fam in ("fg", "bmw", "h", "x"):
    GROUPS.append((f"C-4L {fam.upper()} Ordered W6", fam, "c4l-{arm}-{r}-" + SHA))
for pol, name in (("ord", "Ordered"), ("rdy", "Ready")):
    GROUPS.append((f"C-5F {name} W24", "five-finite", f"c5f-{pol}-" + "{arm}-{r}-" + SHA))
GROUPS.append(("C-HOT-sub r1a12 Ready W12", "hot", "hotsub-r1a12-{arm}-{r}-" + SHA))


def load(label, fam):
    d = RUNS / label / fam
    out = {}
    for name in ("metrics", "g2stats", "g2stats-v2", "pending", "audit", "g2verify"):
        p = d / f"{name}.json"
        if p.exists() and p.stat().st_size:
            try:
                out[name] = json.load(open(p))
            except json.JSONDecodeError:
                out[name] = {}
        else:
            out[name] = {}
    for key in ("g2_anchor_reference_kinds", "apply_by_owner"):
        if out.get("g2stats-v2") and key not in out["g2stats"]:
            out["g2stats"][key] = out["g2stats-v2"].get(key)
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


def plan_seconds(m):
    g2 = (m.get("w0_g2_donly") or {}).get("by_owner_class", {})
    return sum(v.get("plan_seconds", 0) for v in g2.values()) if g2 else None


def row(name, arm, r, rep):
    m, g, p, a = r["metrics"], r["g2stats"], r["pending"], r["audit"]
    ap, rt = native_calls(g)
    rec = m.get("recorder", {})
    closure = m.get("closure") or {}
    return (f"| {name} {rep} | {ARM_NAME[arm]} | {m.get('exit_code')} / {m.get('status')} | {m.get('frontiers')} | "
            f"{closure.get('initial_closed')}/{closure.get('initial_total')} | {fnum(ap)} / {fnum(rt)} | "
            f"{fnum(rec_seconds(g), '{:,.1f}')} | {fnum(plan_seconds(m), '{:,.1f}')} | "
            f"{fnum(m.get('slot_busy_seconds_sum'), '{:,.1f}')} | "
            f"{fnum(float(m['traversal_seconds']) if m.get('traversal_seconds') else None, '{:,.1f}')} | "
            f"{fnum(int(m['scheduled_nodes']) if m.get('scheduled_nodes') else None)} | {fnum(p.get('peak_pending_domains'))} | "
            f"{fnum(p.get('pending_growth_per_completion_slope_to_peak'), '{:.3f}')} | "
            f"{fnum(p.get('discovered_domains_per_native'), '{:.3f}')} | {a.get('audit', '-')}"
            f"{' / ' + r['g2verify']['verdict'] if r.get('g2verify') else ''} | "
            f"{fnum(rec.get('foreign_busy_cpus_mean'), '{:.1f}')} | {fnum(rec.get('schedstat_run_delay_seconds'), '{:,.0f}')} | "
            f"{fnum(rec.get('schedstat_run_seconds'), '{:,.0f}')} | "
            f"{fnum(instr(m), '{:.4g}')} | {fnum((m.get('perf') or {}).get('ipc_u'), '{:.3f}')} | "
            f"{m.get('cpus')} | {m.get('started_utc')} |")


def instr(m):
    v = (m.get("perf") or {}).get("instructions:u")
    return v if isinstance(v, float) else None


def main():
    print("| Control | Arm | Exit / status | Frontiers | Roots closed | Native calls Apply / Route | "
          "Inspector record-s | of which G2' plan-s | Slot-busy s | Traversal s | Scheduled domains | Peak pending | "
          "Pending growth / completion (slope to peak) | Discovered / native | Audit (/ g2verify) | Foreign busy CPUs (mean) | Run delay s | "
          "Run s (schedstat) | Instructions:u | IPC | CPUs | Start (UTC) |")
    print("|---|---|---|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|---|---|")
    pairs = []
    for name, fam, tmpl in GROUPS:
        for rep in REPS:
            off = load(tmpl.format(arm="off", r=rep), fam)
            if off:
                print(row(name, "off", off, rep))
            for arm in ARMS:
                on = load(tmpl.format(arm=arm, r=rep), fam)
                if on:
                    print(row(name, arm, on, rep))
                    if off:
                        pairs.append((f"{name} {rep}", arm, off, on))
    print()
    print("| Control | Arm | record-s on/off | slot-busy on/off | traversal on/off | Apply native calls on/off | "
          "Route natives on/off | scheduled domains on/off | peak pending on/off | discovered/native on/off |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for name, arm, a, b in pairs:
        def rat(f):
            try:
                x, y = f(b), f(a)
                return f"{x / y:.3f}" if x is not None and y else "-"
            except (KeyError, TypeError, ValueError):
                return "-"
        print(f"| {name} | {ARM_NAME[arm]} | {rat(lambda r: rec_seconds(r['g2stats']))} | "
              f"{rat(lambda r: float(r['metrics']['slot_busy_seconds_sum']))} | "
              f"{rat(lambda r: float(r['metrics']['traversal_seconds']))} | "
              f"{rat(lambda r: native_calls(r['g2stats'])[0])} | {rat(lambda r: native_calls(r['g2stats'])[1])} | "
              f"{rat(lambda r: int(r['metrics']['scheduled_nodes']))} | "
              f"{rat(lambda r: r['pending']['peak_pending_domains'])} | "
              f"{rat(lambda r: r['pending']['discovered_domains_per_native'])} |")
    print()
    print("| Control | Arm | Owner class | Apply records off / on | G2' residual share | G2' full-cover share | "
          "with 2nd anchor | residual point fraction (planned) | Apply record-s off / on (ratio) | distinct pts off / on | "
          "new pts per native call off / on |")
    print("|---|---|---|---|---:|---:|---:|---:|---|---|---|")
    for name, arm, a, b in pairs:
        for k in CLASSES:
            ca = a["g2stats"].get("apply_by_owner_class", {}).get(k)
            cb = b["g2stats"].get("apply_by_owner_class", {}).get(k)
            if not ca or not cb or not ca["apply_records_inspected_or_planned"]:
                continue
            sa, sb = ca["apply_record_seconds"], cb["apply_record_seconds"]
            print(f"| {name} | {ARM_NAME[arm]} | {k} | {ca['apply_records_inspected_or_planned']:,} / "
                  f"{cb['apply_records_inspected_or_planned']:,} | "
                  f"{cb['share_g2_residual']:.3f} | {cb['share_g2_full_cover']:.3f} | {fnum(cb.get('g2_with_second_anchor'))} | "
                  f"{cb['planned_residual_point_fraction']:.3f} | "
                  f"{sa:,.1f} / {sb:,.1f} ({(sb / sa if sa else float('nan')):.3f}) | "
                  f"{fnum(ca.get('distinct_inspected_points'))} / {fnum(cb.get('distinct_inspected_points'))} | "
                  f"{fnum(ca.get('distinct_points_per_native_call'), '{:.1f}')} / "
                  f"{fnum(cb.get('distinct_points_per_native_call'), '{:.1f}')} |")
    print()
    print("| Control | Arm | G2' records | anchor references | to full native inspections | to G2' residual partials | "
          "to G2' full covers | to initial-overlap partials | non-native share | records with >= 1 non-native anchor |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for name, fam, tmpl in GROUPS:
        for rep in REPS:
            for arm in ARMS:
                r = load(tmpl.format(arm=arm, r=rep), fam)
                k = r and r["g2stats"].get("g2_anchor_reference_kinds")
                if not k:
                    continue
                a = k["all"]
                print(f"| {name} {rep} | {ARM_NAME[arm]} | {k['g2_records']:,} | {a['total']:,} | {a['native_inspection']:,} | "
                      f"{a['g2_residual_partial']:,} | {a['g2_full_cover']:,} | {a['initial_overlap_partial']:,} | "
                      f"{a['non_native_share']:.4f} | {k['g2_records_with_a_non_native_anchor']:,} "
                      f"({k['g2_records_with_a_non_native_anchor'] / max(1, k['g2_records']):.3f}) |")


if __name__ == "__main__":
    main()
