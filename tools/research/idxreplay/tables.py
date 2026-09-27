#!/usr/bin/env python
"""Markdown tables from idxreplay JSONL outputs (W0.4 RESULTS.md).

Usage: tables.py static|thin|threads|dynamic|pipeline|streams|lag FILE...
       tables.py pipeline_k 1,16,64 FILE...
       tables.py ratios FILE...   (per-set CPU/candidate ratios, today over SoA)
"""
import json
import math
import sys


def rows(paths, kind=None):
    for p in paths:
        for line in open(p):
            d = json.loads(line)
            if kind is None or d.get("kind") == kind:
                yield d


def fmt(x, nd=0):
    if x is None:
        return "-"
    if isinstance(x, float) and nd:
        return f"{x:,.{nd}f}"
    return f"{x:,.0f}"


def static(paths):
    print("| layout | request set | n | found | candidates tested / request | p50 | p99 | exact verifies / request | CPU µs / request | CPU ns / tested candidate |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for d in rows(paths, "stats"):
        if d["frac"] != 1024:
            continue
        print(f"| {d['layout']} | {d['set']} | {d['n']:,} | {d['found_frac']:.3f} | {fmt(d['tested_per_q'])} | {d['tested_p50']:,} | {d['tested_p99']:,} | {d['exact_calls_per_q']:.1f} | {d['cpu_ns_per_q']/1000:,.1f} | {d['cpu_ns_per_tested']:.2f} |")


def thin(paths):
    by = {}
    for d in rows(paths, "stats"):
        by.setdefault((d["layout"].replace("l0-stored", "l0-rebuilt") if d["frac"] < 1024 else d["layout"], d["set"]), {})[d["frac"]] = d
    print("| layout | request set | tested/request at 25% / 50% / 100% live | exponent (tested) | CPU µs/request at 25% / 50% / 100% | exponent (CPU) |")
    print("|---|---|---|---:|---|---:|")
    for (lay, st), m in sorted(by.items()):
        if lay == "l0-stored" or st not in ("miss", "reverse", "word-only") or len(m) < 3:
            continue
        xs = [math.log(m[f]["live"]) for f in (256, 512, 1024)]
        def slope(key):
            ys = [math.log(max(m[f][key], 1e-9)) for f in (256, 512, 1024)]
            mx, my = sum(xs) / 3, sum(ys) / 3
            return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
        t = " / ".join(fmt(m[f]["tested_per_q"]) for f in (256, 512, 1024))
        c = " / ".join(f"{m[f]['cpu_ns_per_q']/1000:,.1f}" for f in (256, 512, 1024))
        print(f"| {lay} | {st} | {t} | {slope('tested_per_q'):.2f} | {c} | {slope('cpu_ns_per_q'):.2f} |")


def threads(paths):
    print("| layout | request set | threads | requests/s | CPU ns / tested candidate | CPU µs / request | candidates tested / request |")
    print("|---|---|---:|---:|---:|---:|---:|")
    for d in rows(paths, "throughput"):
        print(f"| {d['layout']} | {d['set']} | {d['threads']} | {d['queries_per_s']:,.0f} | {d['cpu_ns_per_tested']:.2f} | {d['cpu_ns_per_q']/1000:,.1f} | {fmt(d['tested_per_q'])} |")


def dynamic(paths):
    print("| trace | admissions | exact / contained / new | outcome mismatches | forward-check mismatches (records) | forward checks engine / replay / first-found | maintenance engine = replay | retired mismatches |")
    print("|---|---:|---|---:|---:|---|---|---:|")
    for d in rows(paths, "dynamic-validation"):
        print(f"| {d['label']} | {d['admissions']:,} | {d['exact']:,} / {d['contained']:,} / {d['new']:,} | {d['outcome_mismatches']} | {d['forward_mismatches']:,} | {d['forward_checks_engine']:,} / {d['forward_checks_replay']:,} / {d['forward_checks_first_found']:,} | {d['maintenance_engine']:,} = {d['maintenance_replay']:,} | {d['retired_mismatches']} |")
    print()
    print("| trace | outcome | n | candidates min-ID / first-found | reverse candidates | p50 / p99 (min-ID) | exponent vs bucket live (R²) |")
    print("|---|---|---:|---|---:|---|---|")
    for d in rows(paths, "dynamic-cost"):
        print(f"| {d['label']} | {d['outcome']} | {d['n']:,} | {d['forward_minid_per_q']:.1f} / {d['forward_firstfound_per_q']:.1f} | {d['reverse_tested_per_q']:.1f} | {d['forward_p50']} / {d['forward_p99']} | {d['exponent_vs_bucket_live']:.2f} ({d['exponent_r2']:.2f}) |")
    print()
    print("| trace | joined requests | layer-reaching | forward candidates today (all commits, min-ID) | after pipeline, min-ID | after pipeline, first-found |")
    print("|---|---:|---:|---:|---:|---:|")
    for d in rows(paths, "dynamic-pipeline-work"):
        print(f"| {d['label']} | {d['joined_requests']:,} | {d['layer_requests']:,} | {d['forward_candidates_today_all_commits']:,} | {d['forward_candidates_pipeline_minid']:,} | {d['forward_candidates_pipeline_firstfound']:,} |")


def pipeline(paths, ks=()):
    tiers = ["exact-job", "self", "local", "mru", "exact-store", "helper", "layer-hit", "miss"]
    print("| trace | scope | MRU k | jobs | requests | requests / job | native ms / job | " + " | ".join(tiers) + " | cheap tiers | MRU tests / request |")
    print("|---|---|---:|---:|---:|---:|---:|" + "---:|" * len(tiers) + "---:|---:|")
    for d in rows(paths, "pipeline"):
        if d["requests"] == 0 or (ks and d["mru_k"] not in ks):
            continue
        sh = " | ".join(f"{100*d['share_'+t]:.1f}%" for t in tiers)
        nm = f"{d['native_ms_per_job']:.3f}" if "native_ms_per_job" in d else "-"
        print(f"| {d['label']} | {d['scope']} | {d['mru_k']} | {d['jobs']:,} | {d['requests']:,} | {d['requests']/max(d['jobs'],1):,.1f} | {nm} | {sh} | {100*d['cheap_share']:.1f}% | {d['mru_tests_per_req']:.2f} |")


def pipeline_k(paths):
    """pipeline_k K1,K2 FILE...: pipeline table restricted to the given MRU k."""
    pipeline(paths[1:], [int(k) for k in paths[0].split(",")])


def streams(paths):
    for d in rows(paths):
        if d["kind"] in ("streams-input", "streams-layer-requests"):
            print(json.dumps(d))
    print()
    print("| engine class | layout | set | n | found | candidates / request | p50 | p99 | CPU µs / request | CPU ns / tested |")
    print("|---|---|---|---:|---:|---:|---:|---:|---:|---:|")
    for d in rows(paths, "streams-layer-cost"):
        print(f"| {d['class']} | {d['layout']} | {d['set']} | {d['n']:,} | {d['found_frac']:.3f} | {fmt(d['tested_per_q'])} | {d['tested_p50']:,} | {d['tested_p99']:,} | {d['cpu_ns_per_q']/1000:,.1f} | {d['cpu_ns_per_tested']:.2f} |")
    for d in rows(paths, "stale-lag-requests"):
        ks = [k for k in d if k.startswith("stale_share_lag_lt_")]
        print()
        print(f"layer-hit requests at MRU k=16: {d['layer_hit_requests_k16']:,} (container age = commit watermark minus container ID)")
        print("| lag L < (new IDs) | " + " | ".join(f"{int(k.split('_lt_')[1]):,}" for k in ks) + " |")
        print("|---|" + "---:|" * len(ks))
        print("| stale share of layer-hit requests | " + " | ".join(f"{100*d[k]:.2f}%" for k in ks) + " |")


def lag(paths):
    for d in rows(paths, "lag"):
        print(f"edges {d['edges']:,}; creation {d['creation_edges']:,}; self {d['self_edges']:,}; helper {d['helper_edges']:,}; layer-hit edges {d['layer_hit_edges']:,}")
        ks = [k for k in d if k.startswith("stale_share_lag_lt_")][:-1]
        print("| lag L < (admissions) | " + " | ".join(f"{int(k.split('_lt_')[1]):,}" for k in ks) + " |")
        print("|---|" + "---:|" * len(ks))
        print("| stale share of hit edges (all generations) | " + " | ".join(f"{100*d[k]:.2f}%" for k in ks) + " |")
    for d in rows(paths, "lag-segment"):
        ks = [k for k in d if k.startswith("stale_share_lag_lt_")][:-1]
        print(f"| {d['segment'].split('/')[-1]} ({d['hit_edges']:,} hit edges) | " + " | ".join(f"{100*d[k]:.2f}%" for k in ks) + " |")


def ratios(paths):
    """CPU ns per tested candidate, today's layout (l0-stored) over each SoA layout,
    per request set and thread count, from `stats` (1 thread, marked s) and
    `throughput` rows at 100% live, and from `streams-layer-cost` rows (real streams)."""
    sets = ["miss", "hit-minid", "hit-firstfound", "reverse", "word-only"]
    print("| source | threads | layout | " + " | ".join(sets) + " | range (all sets) | range (forward: miss, hits) |")
    print("|---|---|---|" + "---:|" * len(sets) + "---|---|")
    for p in paths:
        name = p.rsplit("/", 1)[-1]
        by = {}
        for d in rows([p]):
            if d.get("kind") in ("stats", "throughput") and d.get("frac", 1024) == 1024:
                th = f"{d['threads']}" + ("s" if d["kind"] == "stats" else "")
                by.setdefault(th, {})[(d["layout"], d["set"])] = d["cpu_ns_per_tested"]
            elif d.get("kind") == "streams-layer-cost":
                by.setdefault("1 (real streams)", {})[(d["layout"], d["set"])] = d["cpu_ns_per_tested"]
        for th, m in by.items():
            for lay in ("soa-id", "soa-pattern"):
                rs = {st: m[("l0-stored", st)] / m[(lay, st)] for st in sets if ("l0-stored", st) in m and (lay, st) in m}
                if not rs:
                    continue
                cells = " | ".join(f"{rs[st]:.2f}" if st in rs else "-" for st in sets)
                fw = [rs[st] for st in ("miss", "hit-minid", "hit-firstfound") if st in rs]
                print(f"| {name} | {th} | {lay} | {cells} | {min(rs.values()):.1f}-{max(rs.values()):.1f}x | {min(fw):.1f}-{max(fw):.1f}x |")


if __name__ == "__main__":
    globals()[sys.argv[1]](sys.argv[2:])
