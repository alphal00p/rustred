#!/usr/bin/env python
"""W0.4.2 pre-registered gate table for the W0 G2' falsifier (read-only).

usage: gate.py [RUNS_DIR]
Per control (C-5F Ordered/Ready W24, C-HOT-sub r1a12 Ready W12) and binary,
every G2' arm against the flag-off arm of the same binary: sums over the
repeats (ratio of means) and the spread of all on/off cross ratios, for
inspector record-seconds (the gate metric), slot-busy seconds, schedstat run
seconds (CPU on-core time of the walk threads), Apply native calls, Route
natives, scheduled domains, peak pending and inspected Apply lattice points.
Gate (master plan W4.2, pre-registered): drained with 0 frontiers, audit
PASS (and g2verify PASS), record-s ratio of means <= 0.5, and scheduled
domains and peak pending ratios <= 1.
"""
import json
import sys
from pathlib import Path

RUNS = Path(sys.argv[1] if len(sys.argv) > 1 else "/common/dev/rustred/TMP/w0/g2falsify/runs")
CLASSES = ("000011001001011", "011101110111000", "other")
NAME = {"on": "1 anchor (B1)", "m1": "1 anchor", "m2": "<=2 anchors", "u": "union per D level"}
CONTROLS = [
    ("C-5F Ordered W24", "five-finite", "c5f-ord-{arm}-{r}-{sha}"),
    ("C-5F Ready W24", "five-finite", "c5f-rdy-{arm}-{r}-{sha}"),
    ("C-HOT-sub r1a12 Ready W12", "hot", "hotsub-r1a12-{arm}-{r}-{sha}"),
]
SHAS = ("13a3271a", "8918122a", "b04d00b2")


def load(d):
    out = {}
    for name in ("metrics", "g2stats", "pending", "audit", "g2verify"):
        p = d / f"{name}.json"
        try:
            out[name] = json.load(open(p)) if p.exists() and p.stat().st_size else {}
        except json.JSONDecodeError:
            out[name] = {}
    return out if out["metrics"] else None


def metrics(r):
    m, g, p = r["metrics"], r["g2stats"], r["pending"]
    cls = g.get("apply_by_owner_class", {})
    return {
        "record_s": sum(v["record_seconds"] for v in g.get("native_by_phase", {}).values()) if g else None,
        "slot_busy_s": float(m.get("slot_busy_seconds_sum") or 0) or None,
        "run_s": (m.get("recorder") or {}).get("schedstat_run_seconds"),
        "apply_calls": sum(cls[k]["native_calls"] for k in CLASSES) if cls else None,
        "route_natives": g.get("native_by_phase", {}).get("Route", {}).get("records") if g else None,
        "scheduled": int(m["scheduled_nodes"]) if m.get("scheduled_nodes") else None,
        "peak_pending": p.get("peak_pending_domains"),
        "inspected_points": sum(cls[k]["inspected_points"] for k in CLASSES) if cls else None,
    }


def ok(r):
    m = r["metrics"]
    v = r.get("g2verify") or {}
    return (m.get("exit_code") == 0 and str(m.get("frontiers")) == "0" and r["audit"].get("audit") == "PASS"
            and (not v or v.get("verdict") == "PASS"))


def main():
    keys = ("record_s", "slot_busy_s", "run_s", "apply_calls", "route_natives", "scheduled", "peak_pending",
            "inspected_points")
    print("| Control | Binary | Arm | n on/off | all drained, 0 frontiers, audit/g2verify PASS | "
          + " | ".join(f"{k} (ratio of means; cross min-max)" for k in keys) + " | Gate |")
    print("|---|---|---|---|---|" + "---:|" * len(keys) + "---|")
    for cname, fam, tmpl in CONTROLS:
        for sha in SHAS:
            off = [load(RUNS / tmpl.format(arm="off", r=r, sha=sha) / fam) for r in ("r1", "r2")]
            off = [x for x in off if x]
            if not off:
                continue
            for arm in ("on", "m1", "m2", "u"):
                on = [load(RUNS / tmpl.format(arm=arm, r=r, sha=sha) / fam) for r in ("r1", "r2")]
                on = [x for x in on if x]
                if not on:
                    continue
                good = all(ok(x) for x in off + on)
                cells, ratio = [], {}
                for k in keys:
                    a = [metrics(x)[k] for x in off]
                    b = [metrics(x)[k] for x in on]
                    if None in a or None in b or not all(a):
                        cells.append("-")
                        continue
                    ratio[k] = (sum(b) / len(b)) / (sum(a) / len(a))
                    cross = [y / x for x in a for y in b]
                    cells.append(f"{ratio[k]:.3f} ({min(cross):.3f}-{max(cross):.3f})")
                gate = ("PASS" if good and ratio.get("record_s", 9) <= 0.5 and ratio.get("scheduled", 9) <= 1
                        and ratio.get("peak_pending", 9) <= 1 else "FAIL")
                print(f"| {cname} | {sha} | {NAME[arm]} | {len(on)}/{len(off)} | {'yes' if good else 'NO'} | "
                      + " | ".join(cells) + f" | {gate} |")


if __name__ == "__main__":
    main()
