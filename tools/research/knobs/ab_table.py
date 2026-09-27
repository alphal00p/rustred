#!/usr/bin/env python
"""Group W0.8 knob runs by (family, arm) and compare every arm with a
reference arm. The arm of a run is its label without the trailing `-r<k>`
repeat suffix. Prints mean [min..max] per metric and the ratio of means.

Usage: ab_table.py --reference ARM [--runs DIR] [--metrics m1,m2] [--json OUT] LABEL_GLOB...
Metrics are keys of summarize.row() (e.g. mistakes_per_native,
transfers_per_native, natives, peak_pending, inspector_cpu, auc, cpu50,
inspector_ms_per_native, wall, foreign_load).
"""
import argparse
import fnmatch
import json
import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import summarize  # noqa: E402

DEFAULT = ("natives", "mistakes_per_native", "transfers_per_native", "peak_unreserved", "peak_pending",
           "inspector_cpu", "inspector_ms_per_native", "auc", "cpu50", "wall", "foreign_load")


def arm_of(label):
    return re.sub(r"-r\d+$", "", label)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--runs", default=str(summarize.RUNS))
    p.add_argument("--reference", required=True, help="reference arm name (label without -rN)")
    p.add_argument("--metrics", default=",".join(DEFAULT))
    p.add_argument("--json")
    p.add_argument("labels", nargs="+")
    args = p.parse_args()
    metrics = args.metrics.split(",")
    runs = sorted(r for r in Path(args.runs).glob("*/*") if (r / "metrics.json").exists()
                  and any(fnmatch.fnmatch(r.parent.name, g) for g in args.labels))
    horizon = defaultdict(list)
    for r in runs:
        if arm_of(r.parent.name) == args.reference:
            m = json.loads((r / "metrics.json").read_text())
            if m.get("inspector_cpu_seconds"):
                horizon[r.name].append(m["inspector_cpu_seconds"])
    groups = defaultdict(list)
    for r in runs:
        h = statistics.mean(horizon[r.name]) if horizon.get(r.name) else None
        row = summarize.row(r, h)
        groups[(r.name, arm_of(r.parent.name))].append(row)
    out = []
    for family in sorted({f for f, _ in groups}):
        ref = groups.get((family, args.reference), [])
        print(f"\n### {family} (reference {args.reference}, n={len(ref)})\n")
        print("| arm | n | exits | frontiers | " + " | ".join(metrics) + " |")
        print("|---|---|---|---|" + "---|" * len(metrics))
        for (fam, arm), rows in sorted(groups.items()):
            if fam != family:
                continue
            cells, record = [], {"family": family, "arm": arm, "n": len(rows)}
            for key in metrics:
                vals = [r[key] for r in rows if isinstance(r.get(key), (int, float))]
                refs = [r[key] for r in ref if isinstance(r.get(key), (int, float))]
                if not vals:
                    cells.append("-")
                    continue
                mean = statistics.mean(vals)
                ratio = mean / statistics.mean(refs) if refs and statistics.mean(refs) else None
                record[key] = {"mean": mean, "min": min(vals), "max": max(vals), "ratio": ratio}
                span = f" [{min(vals):.4g}..{max(vals):.4g}]" if len(vals) > 1 else ""
                rtxt = f" (x{ratio:.3f})" if ratio is not None and arm != args.reference else ""
                cells.append(f"{mean:.4g}{span}{rtxt}")
            exits = ",".join(str(r["exit"]) + ("T" if r.get("timed_out") else "") for r in rows)
            fr = ",".join(str(r["frontiers"]) for r in rows)
            print(f"| {arm} | {len(rows)} | {exits} | {fr} | " + " | ".join(cells) + " |")
            out.append(record)
    if args.json:
        json.dump(out, open(args.json, "w"), indent=1)


if __name__ == "__main__":
    main()
