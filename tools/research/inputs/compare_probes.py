#!/usr/bin/env python3
"""Compare probe.py runs at matched natives (completed nodes).

For each probe directory: interpolated discovered domains, pending, dependency
edges and closed roots at a list of native counts, plus the census phase split
and Route->Route edges per Route native at the stop.

Usage: compare_probes.py NAME=DIR [NAME=DIR ...] [--at 500000,1000000,...]
"""
import argparse
import csv
import json
from pathlib import Path


def interp(series, c, key):
    prev = None
    for r in series:
        if (r.get("completed_nodes") or 0) >= c:
            if prev is None or r[key] is None or prev[key] is None:
                return r[key]
            c0, c1 = prev["completed_nodes"], r["completed_nodes"]
            f = (c - c0) / (c1 - c0) if c1 > c0 else 0.0
            return prev[key] + f * (r[key] - prev[key])
        prev = r
    return None


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("runs", nargs="+")
    p.add_argument("--at", default="500000,1000000,1500000,2000000,2500000")
    args = p.parse_args(argv)
    points = [int(x) for x in args.at.split(",")]
    out = {}
    for spec in args.runs:
        name, _, d = spec.partition("=")
        d = Path(d)
        series = [json.loads(line) for line in open(d / "timeseries.jsonl") if line.strip()]
        summary = json.loads((d / "census/summary.json").read_text())
        trans = list(csv.DictReader(open(d / "census/transitions.tsv"), delimiter="\t"))
        rr = sum(int(t["edges"]) for t in trans if t["source_phase"] == "Route" and t["target_phase"] == "Route")
        row = {"at": {}, "final": series[-1] if series else None, "census": summary,
               "route_route_edges_per_route_native": rr / max(1, summary["route_inspected"])}
        for c in points:
            v = {k: interp(series, c, k) for k in ("scheduled_nodes", "queued_nodes", "edges", "t")}
            v["roots_closed"] = next((r.get("roots_closed") for r in series if (r.get("completed_nodes") or 0) >= c), None)
            row["at"][c] = v
        out[name] = row
    names = list(out)
    print("natives | " + " | ".join(f"{n}: discovered / pending / roots / t" for n in names))
    for c in points:
        cells = []
        for n in names:
            v = out[n]["at"][c]
            if v["scheduled_nodes"] is None:
                cells.append("-")
            else:
                cells.append(f"{v['scheduled_nodes']/1e6:.2f}M / {v['queued_nodes']/1e6:.2f}M / {v['roots_closed']} / {v['t']:.0f}s")
        print(f"{c/1e6:.1f}M | " + " | ".join(cells))
    for n in names:
        f = out[n]["final"]
        s = out[n]["census"]
        print(f"{n}: final natives {f['completed_nodes']}, discovered {f['scheduled_nodes']}, pending {f['queued_nodes']}, "
              f"roots {f['roots_closed']}, frontiers {f['frontiers']}, rank {f['max_scheduled_finite_rank']}, t {f['t']:.0f}s; "
              f"Apply natives {s['apply_inspected']}, Route natives {s['route_inspected']}, "
              f"R->R per Route native {out[n]['route_route_edges_per_route_native']:.2f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
