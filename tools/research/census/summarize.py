#!/usr/bin/env python
"""Markdown tables from census JSON outputs (read-only).

usage: summarize.py cover OUT-cover.json [OUT-cover-rows.jsonl]
       summarize.py potential OUT-potential.json
       summarize.py saturation OUT-saturation.json [owner ...]
       summarize.py pilot OUT-pilot.json
"""
import collections
import json
import sys

VOC = ["exact_pointwise", "d_only", "hull", "hull_per_d_run", "ar_c2", "hull_c2"]


def pct(x):
    return "-" if x is None else f"{100 * x:.1f}%"


def cover(path, rows=None):
    d = json.load(open(path))
    print(f"source `{path}`; generation {d.get('generation')}; params {d.get('params')}")
    print()
    for sample, v in d.items():
        if not isinstance(v, dict) or "all_owners" not in v:
            continue
        for scope in ("all_owners", "hot_owner"):
            for aset, s in v[scope].items():
                print(f"**{sample} / {scope} / {aset}**: draws {s['draws']:.0f} (distinct {s['distinct']}), "
                      f"fully covered {pct(s['fully_covered_share'])}, mean uncovered fraction "
                      f"{pct(s['mean_uncovered_fraction'])}, unevaluated (infinite) {pct(s['unevaluated_share(infinite)'])}")
                print()
                print("| vocabulary | gate share (resid <= 10%, <= 8 pieces) | mean residual fraction | mean pieces | projected rel. cost, binned law [E] | projected rel. cost, OLS law [E] |")
                print("|---|---:|---:|---:|---:|---:|")
                for name in VOC:
                    e = s["vocabularies"].get(name)
                    if not e:
                        continue
                    binned = e.get("projected_relative_cost_binned[E]")
                    ols = e.get("projected_relative_cost_ols[E]", e.get("projected_relative_cost[E]"))
                    print(f"| {name} | {pct(e['gate_share(resid<=10%,pieces<=8)'])} | {pct(e['mean_residual_fraction'])} | "
                          f"{e['mean_pieces']:.2f} | {'-' if binned is None else f'{binned:.3f}'} | {'-' if ols is None else f'{ols:.3f}'} |")
                print()
    if rows:
        by = collections.defaultdict(lambda: collections.defaultdict(collections.Counter))
        inexact = collections.Counter()
        draws = collections.Counter()
        for line in open(rows):
            r = json.loads(line)
            for aset, e in r["evals"].items():
                if e is None:
                    continue
                key = (r["sample"], aset)
                draws[key] += r["mult"]
                if not e["exact"]:
                    inexact[key] += r["mult"]
                if e["uncovered"] > 0:
                    by[key]["d_only"][e["d_only"][0]] += r["mult"]
                    by[key]["ar"][e["ar"][0]] += r["mult"]
        print("| sample / anchor set | sampled-point (inexact) draws | D-only pieces of partial residuals | A/R pieces of partial residuals |")
        print("|---|---:|---|---|")
        for key in sorted(draws):
            dh = dict(sorted(by[key]["d_only"].items()))
            ah = dict(sorted(by[key]["ar"].items()))
            print(f"| {key[0]} / {key[1]} | {pct(inexact[key] / draws[key])} | {dh} | {ah} |")
        print()


def potential(path):
    d = json.load(open(path))
    k = [x for x in d if x.startswith("edge_counts")][0]
    print(f"source `{path}`; generation {d['generation']}; nodes {d['nodes']}; {k} = {d[k]}")
    print()
    print("| graph / weight | node pairs | positive pairs | positive self-loops | positive cycle | max potential |")
    print("|---|---:|---:|---:|---|---:|")
    for g, v in sorted(d["graphs"].items()):
        print(f"| {g} | {v['node_pairs']} | {v['positive_pairs']} | {v['positive_self_loops']} | {v['positive_cycle']} | {v['max_potential']} |")
    print()


def saturation(path, owners):
    d = json.load(open(path))
    print(f"source `{path}`; draws per window {d['draws']}")
    print()
    print("| generation | natives in window | est. new points | new points per native | union after (natives) |")
    print("|---:|---:|---:|---:|---:|")
    for w in d["totals_natives"]:
        print(f"| {w['gen']} | {w['natives']:.0f} | {w['new_points']:.3e} | {w['new_points_per_native']:.1f} | {w['union_after']:.3e} |")
    print()
    for o in owners:
        print(f"owner `{o}` natives by record generation:")
        print()
        print("| gen | natives | sum points | new fraction (points-weighted) | new fraction (CPU-weighted) | new points per native | union after |")
        print("|---:|---:|---:|---:|---:|---:|---:|")
        for w in d["owners"][o]["natives"]:
            x = w["window"]
            print(f"| {w['gen']} | {x['members']} | {x['sum_points']:.3e} | {pct(x['new_fraction_points'])} | {pct(x['new_fraction_cpu'])} | "
                  f"{w['new_points_per_native']:.1f} | {w['union_after']:.3e} |")
        print()


def pilot(path):
    d = json.load(open(path))
    for k, v in d.items():
        if isinstance(v, dict) and "all" in v:
            print(f"**{k}**")
            print()
            print("| scope | draws | fully covered | gate hull | gate D-only | mean uncovered fraction | projected rel. cost hull [E] |")
            print("|---|---:|---:|---:|---:|---:|---:|")
            for scope, s in v.items():
                proj = s.get("projected_relative_cost_hull[E]")
                print(f"| {scope} | {s['draws']:.0f} | {pct(s['fully_covered_share'])} | {pct(s['gate_hull_share'])} | {pct(s['gate_d_only_share'])} | "
                      f"{pct(s['mean_uncovered_fraction'])} | {proj:.3f} |")
            print()
        elif not isinstance(v, dict) or k.startswith("exact"):
            print(f"- {k}: {json.dumps(v)}")
    print()


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "cover":
        cover(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)
    elif mode == "potential":
        potential(sys.argv[2])
    elif mode == "saturation":
        saturation(sys.argv[2], sys.argv[3:])
    elif mode == "pilot":
        pilot(sys.argv[2])


def owners(rows, sample, aset, top=8):
    """Per-owner breakdown of one sample / anchor set from the cover rows."""
    agg = collections.defaultdict(lambda: collections.Counter())
    for line in open(rows):
        r = json.loads(line)
        if r["sample"] != sample:
            continue
        e = r["evals"].get(aset)
        if e is None:
            continue
        a = agg[r["owner"]]
        m = r["mult"]
        a["draws"] += m
        pts = e["points"] or 1.0
        a["full"] += m * (e["uncovered"] == 0)
        d_res = 0.0 if e["uncovered"] == 0 else e["d_only"][1]
        a["gate_d"] += m * (d_res <= 0.1 * pts and (e["uncovered"] == 0 or e["d_only"][0] <= 8))
        h_res = 0.0 if e["uncovered"] == 0 else e["hull"]
        a["gate_h"] += m * (h_res <= 0.1 * pts)
        a["unc"] += m * e["uncovered"] / pts
    total = sum(a["draws"] for a in agg.values())
    print(f"per owner, {sample} / {aset} (share of draws = share of the sample's weight):")
    print()
    print("| owner | share of draws | fully covered | gate D-only | gate hull | mean uncovered fraction |")
    print("|---|---:|---:|---:|---:|---:|")
    for o, a in sorted(agg.items(), key=lambda kv: -kv[1]["draws"])[:top]:
        d = a["draws"]
        print(f"| {o} | {pct(d / total)} | {pct(a['full'] / d)} | {pct(a['gate_d'] / d)} | {pct(a['gate_h'] / d)} | {pct(a['unc'] / d)} |")
    print()


if __name__ == "__main__" and sys.argv[1] == "owners":
    owners(sys.argv[2], sys.argv[3], sys.argv[4])
