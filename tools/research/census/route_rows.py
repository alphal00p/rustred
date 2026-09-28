#!/usr/bin/env python3
"""Row-level quantiles of a routecensus rows file (census route --rows / route-apply --rows).

usage: route_rows.py ROWS.jsonl

For every sample and anchor set (admission rows split by phase): HT-weighted quantiles
(p50/p90/p99) of the candidate anchors met by the query (all draws and fully covered draws),
the share of exact (non-sampled) verdicts, and the query size in points. Only prints.
"""
import collections
import json
import sys

rows = collections.defaultdict(list)
for line in open(sys.argv[1]):
    r = json.loads(line)
    rows[r["sample"]].append(r)
def wq(pairs, qs):
    pairs = sorted(pairs)
    tot = sum(w for _, w in pairs)
    out = []
    for q in qs:
        acc = 0
        for v, w in pairs:
            acc += w
            if acc >= q * tot:
                out.append(v); break
    return out
for sample, rs in rows.items():
    sets = list(rs[0]["evals"].keys())
    for s in sets:
        for ph in (["Route", "Apply"] if sample == "all_domains_at_admission" else [None]):
            sel = [r for r in rs if ph is None or r["stratum"].startswith(ph)]
            c = [(r["evals"][s]["candidates"], r["ht"]) for r in sel if r["evals"][s] is not None]
            full = [(r["evals"][s]["candidates"], r["ht"]) for r in sel if r["evals"][s] is not None and r["evals"][s]["uncovered"] == 0]
            ex = sum(r["ht"] for r in sel if r["evals"][s] is not None and r["evals"][s]["exact"]) / sum(r["ht"] for r in sel)
            pts = [(r["evals"][s]["points"], r["ht"]) for r in sel if r["evals"][s] is not None]
            print(f"{sample:26s} {str(ph):5s} {s:30s} candidates p50/p90/p99 all {wq(c,[.5,.9,.99])} fully-covered {wq(full,[.5,.9,.99])} exact-share {ex:.4f} points p50/p90/p99 {wq(pts,[.5,.9,.99])}")
