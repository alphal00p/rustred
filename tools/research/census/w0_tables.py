#!/usr/bin/env python
"""Markdown tables of the W0.7 census note (docs/research/fable51_w0_census_2026-09-27.md)
from a w0_batch.sh receipt directory (read-only). usage: w0_tables.py RECEIPT_DIR"""
import collections
import json
import os
import sys

R = sys.argv[1]


def J(p):
    return json.load(open(os.path.join(R, p)))


def pct(x, d=1):
    return "-" if x is None or x != x else f"{100 * x:.{d}f}%"


def f3(x):
    return "-" if x is None or x != x else f"{x:.3f}"


def f2(x):
    return "-" if x is None or x != x else f"{x:.2f}"


def gate(s, v):
    return s["vocabularies"][v]["gate_share(resid<=10%,pieces<=8)"]


def cost(s, v, law="binned"):
    return s["vocabularies"][v][f"projected_relative_cost_{law}[E]"]


def section(t):
    print()
    print(f"### {t}")
    print()


# T1 gate
section("T1 gate 0.7 (hist_pps_seconds, anchors natives_before_dispatch)")
print("| run | scope | draws (distinct) | fully covered | mean uncovered fraction | unevaluated | gate D-only | gate A/R (C2) | gate hull | gate exact pointwise | proj. rel. cost binned D-only / hull / A/R [E] | proj. rel. cost OLS D-only / hull [E] |")
print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|")
for lab, p in [("gen7 wait 30 s", "gen7/cover-w30.json"), ("gen7 wait 300 s", "gen7/cover-w300.json"),
               ("gen7 wait 30 s, cap 2e7, 2e5 samples", "gen7/cover-w30-cap2e7.json"),
               ("gen6 wait 30 s", "gen6/cover-w30.json"), ("gen3 wait 30 s", "gen3/cover-w30.json")]:
    if not os.path.exists(os.path.join(R, p)):
        continue
    d = J(p)
    for sc in ("all_owners", "hot_owner"):
        s = d["hist_pps_seconds"][sc]["natives_before_dispatch"]
        print(f"| {lab} | {sc} | {s['draws']:.0f} ({s['distinct']}) | {pct(s['fully_covered_share'])} | {pct(s['mean_uncovered_fraction'])} | "
              f"{pct(s['unevaluated_share(infinite)'], 2)} | {pct(gate(s, 'd_only'))} | {pct(gate(s, 'ar_c2'))} | {pct(gate(s, 'hull'))} | {pct(gate(s, 'exact_pointwise'))} | "
              f"{f3(cost(s, 'd_only'))} / {f3(cost(s, 'hull'))} / {f3(cost(s, 'ar_c2'))} | {f3(cost(s, 'd_only', 'ols'))} / {f3(cost(s, 'hull', 'ols'))} |")

# T2 anchor ladder and weighting
section("T2 anchor-set ladder and weighting, gen 7 (wait 30 s)")
print("| sample | anchors | scope | draws | fully covered | mean uncovered | gate D-only | gate A/R | gate hull | proj. rel. cost binned D-only / hull [E] |")
print("|---|---|---|---:|---:|---:|---:|---:|---:|---|")
d = J("gen7/cover-w30.json")
for sm in ("hist_pps_seconds", "hist_uniform", "pending_pps_predicted", "pending_uniform"):
    for sc in ("all_owners", "hot_owner"):
        for a, s in d[sm][sc].items():
            print(f"| {sm} | {a} | {sc} | {s['draws']:.0f} | {pct(s['fully_covered_share'])} | {pct(s['mean_uncovered_fraction'])} | "
                  f"{pct(gate(s, 'd_only'))} | {pct(gate(s, 'ar_c2'))} | {pct(gate(s, 'hull'))} | {f3(cost(s, 'd_only'))} / {f3(cost(s, 'hull'))} |")
print()
print(f"population: {d['apply_natives']} Apply natives, {d['apply_native_seconds']:.0f} s; finite pending Apply {d['pending_apply_finite']}, predicted {d['pending_apply_predicted_seconds[E]']:.0f} s [E]")


# T3 pieces
def pieces(path, sample, aset):
    dh = collections.Counter()
    ah = collections.Counter()
    tot = une = inex = part = 0
    for line in open(os.path.join(R, path)):
        r = json.loads(line)
        if r["sample"] != sample or aset not in r["evals"]:
            continue
        e = r["evals"][aset]
        m = r["mult"]
        tot += m
        if e is None:
            une += m
            continue
        if not e["exact"]:
            inex += m
        if e["uncovered"] > 0:
            part += m
            dh[e["d_only"][0]] += m
            ah[e["ar"][0]] += m
    return tot, une, inex, part, dh, ah


def hist(h):
    keys = sorted(h)
    out = []
    small = [k for k in keys if k <= 3]
    for k in small:
        out.append(f"{k}: {h[k]}")
    rest = [k for k in keys if k > 3]
    if rest:
        a8 = sum(h[k] for k in rest if k <= 8)
        g8 = sum(h[k] for k in rest if k > 8)
        if a8:
            out.append(f"4-8: {a8}")
        if g8:
            out.append(f">8: {g8} (max {max(rest)})")
    return ", ".join(out)


section("T3 residual piece counts of partially covered draws (draws, PPS multiplicity)")
print("| run / sample / anchors | draws | partial (uncovered > 0) | sampled (inexact) | unevaluated | D-only pieces | A/R pieces |")
print("|---|---:|---:|---:|---:|---|---|")
for p, sm, a in [("gen7/cover-w30-rows.jsonl", "hist_pps_seconds", "natives_before_dispatch"),
                 ("gen7/cover-w30-rows.jsonl", "hist_uniform", "natives_before_dispatch"),
                 ("gen7/cover-w30-rows.jsonl", "pending_pps_predicted", "all_natives"),
                 ("gen7/cover-w30-rows.jsonl", "pending_pps_predicted", "all_other_domains"),
                 ("gen7/cover-w30-cap2e7-rows.jsonl", "hist_pps_seconds", "natives_before_dispatch"),
                 ("gen6/cover-w30-rows.jsonl", "hist_pps_seconds", "natives_before_dispatch"),
                 ("gen3/cover-w30-rows.jsonl", "hist_pps_seconds", "natives_before_dispatch")]:
    if not os.path.exists(os.path.join(R, p)):
        continue
    tot, une, inex, part, dh, ah = pieces(p, sm, a)
    print(f"| {p.split('/')[0]} {p.split('/')[1].replace('-rows.jsonl', '')} / {sm} / {a} | {tot} | {part} | {inex} | {une} | {hist(dh)} | {hist(ah)} |")


# T4 per owner
def owners(path, sample, aset, top=10):
    agg = collections.defaultdict(collections.Counter)
    for line in open(os.path.join(R, path)):
        r = json.loads(line)
        if r["sample"] != sample or aset not in r["evals"]:
            continue
        e = r["evals"][aset]
        a = agg[r["owner"]]
        m = r["mult"]
        a["draws"] += m
        if e is None:
            a["une"] += m
            a["unc"] += m
            continue
        pts = e["points"] or 1.0
        a["full"] += m * (e["uncovered"] == 0)
        d_res = 0.0 if e["uncovered"] == 0 else e["d_only"][1]
        a["gd"] += m * (d_res <= 0.1 * pts and (e["uncovered"] == 0 or e["d_only"][0] <= 8))
        ar_res = 0.0 if e["uncovered"] == 0 else e["ar"][1]
        a["ga"] += m * (ar_res <= 0.1 * pts and (e["uncovered"] == 0 or e["ar"][0] <= 8))
        h_res = 0.0 if e["uncovered"] == 0 else e["hull"]
        a["gh"] += m * (h_res <= 0.1 * pts)
        a["unc"] += m * e["uncovered"] / pts
    tot = sum(a["draws"] for a in agg.values())
    print(f"| owner | share of draws | fully covered | gate D-only | gate A/R | gate hull | mean uncovered | unevaluated |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|")
    for o, a in sorted(agg.items(), key=lambda kv: -kv[1]["draws"])[:top]:
        dd = a["draws"]
        print(f"| {o} | {pct(dd / tot)} | {pct(a['full'] / dd)} | {pct(a['gd'] / dd)} | {pct(a['ga'] / dd)} | {pct(a['gh'] / dd)} | {pct(a['unc'] / dd)} | {pct(a['une'] / dd)} |")


section("T4 per owner, gen 7, hist_pps_seconds / natives_before_dispatch (top 10 by draws)")
owners("gen7/cover-w30-rows.jsonl", "hist_pps_seconds", "natives_before_dispatch")
section("T4b per owner, pilot, hist_pps_seconds / natives_before_dispatch")
owners("pilot/pilot-cover-rows.jsonl", "hist_pps_seconds", "natives_before_dispatch")
section("T4c per owner, C-5F, hist_pps_seconds / natives_before_dispatch")
owners("c5f/cover-rows.jsonl", "hist_pps_seconds", "natives_before_dispatch")

# T5 pilot + controls
section("T5 pilot and controls (hist_pps_seconds)")
print("| run | scope (hot mask) | anchors | draws | fully covered | mean uncovered | unevaluated | gate D-only | gate A/R | gate hull | proj. rel. cost binned D-only / hull [E] |")
print("|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|")
for lab, p, hot in [("pilot", "pilot/pilot-cover.json", "011101110111000"), ("pilot", "pilot/pilot-cover-hot2.json", "000011001001011"),
                    ("C-5F", "c5f/cover.json", "011101110111000"), ("C-5F", "c5f/cover-hot2.json", "000011001001011"),
                    ("four-loop FG", "four-loop/fg-cover.json", None), ("four-loop BMW", "four-loop/bmw-cover.json", None),
                    ("four-loop H", "four-loop/h-cover.json", None), ("four-loop X", "four-loop/x-cover.json", None)]:
    if not os.path.exists(os.path.join(R, p)):
        continue
    d = J(p)
    scopes = [("all_owners", "all")] + ([("hot_owner", hot)] if hot else [])
    if hot == "000011001001011":
        scopes = [("hot_owner", hot)]
    for sc, name in scopes:
        for a, s in d["hist_pps_seconds"][sc].items():
            print(f"| {lab} | {name} | {a} | {s['draws']:.0f} | {pct(s['fully_covered_share'])} | {pct(s['mean_uncovered_fraction'])} | {pct(s['unevaluated_share(infinite)'])} | "
                  f"{pct(gate(s, 'd_only'))} | {pct(gate(s, 'ar_c2'))} | {pct(gate(s, 'hull'))} | {f3(cost(s, 'd_only'))} / {f3(cost(s, 'hull'))} |")

# T6 closure import
section("T6 hot-owner closure import (pilot natives as anchors for v2 gen 7, and reverse)")
for p in ("pilot/v2g7-vs-pilot.json", "pilot/v2g7-vs-pilot-hot2.json"):
    if not os.path.exists(os.path.join(R, p)) or os.path.getsize(os.path.join(R, p)) == 0:
        continue
    d = J(p)
    print(f"`{p}`: pilot records {d['pilot_records']}, pilot buckets {d['pilot_buckets']}; v2 Apply natives in pilot buckets {d['v2_apply_natives_in_pilot_buckets']}; "
          f"v2 Apply pending in pilot buckets {d['v2_apply_pending_in_pilot_buckets']} [pred. s E]; pilot Apply natives {d['pilot_apply_natives']}")
    print()
    print("| sample | scope | draws | fully covered | gate D-only | gate hull | mean uncovered | proj. rel. cost hull [E] | unevaluated |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    for k, v in d.items():
        if isinstance(v, dict) and "all" in v:
            for sc, s in v.items():
                print(f"| {k} | {sc} | {s['draws']:.0f} | {pct(s['fully_covered_share'])} | {pct(s['gate_d_only_share'])} | {pct(s['gate_hull_share'])} | "
                      f"{pct(s['mean_uncovered_fraction'])} | {f3(s['projected_relative_cost_hull[E]'])} | {pct(s.get('unevaluated_share(infinite)'))} |")
    ex = d["exact_identical[class|phase|hot: in_pilot_buckets, identical, total]"]
    print()
    print("exact identical domains [in pilot buckets, identical, total]: " + "; ".join(f"{k} {v}" for k, v in ex.items()))
    print()


# T7 cost
def costtab(p, top):
    d = J(p)
    pb = d["pooled"]
    print(f"`{p}`: Apply seconds {d['apply_seconds']:.0f}; pooled OLS b {f3(pb['cost_exponent'])} (r2 {f2(pb['r2'])}, n {pb['n']}); pooled bin-mean WLS b {f3(pb.get('cost_exponent_binmean_wls'))}")
    print()
    print("| owner | Apply s share | natives | OLS b (r2) | bin-mean WLS b | OLS b < 1e5 pts | bin-mean b < 1e5 pts | top-decade b | max decade | OLS b by gen | bin-mean b by gen |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|")
    for o in d["owners"][:top]:
        g = o.get("cost_exponent_by_gen[b,r2,n,b_binmean_wls]", {})
        lt = o.get("cost_exponent_ols_pts_lt_1e5[b,r2,n]", [None] * 3)
        mx = max(int(k) for k in o["decades"]) if o["decades"] else None
        print(f"| {o['owner']} | {pct(o['seconds_share'])} | {o['natives']} | {f2(o['cost_exponent'])} ({f2(o['cost_r2'])}) | {f2(o.get('cost_exponent_binmean_wls'))} | "
              f"{f2(lt[0])} | {f2(o.get('cost_exponent_binmean_wls_pts_lt_1e5'))} | {f2(o['top_decade_exponent'])} | {mx} | "
              f"{', '.join(f'g{k} {f2(v[0])}' for k, v in g.items())} | {', '.join(f'g{k} {f2(v[3])}' for k, v in g.items())} |")
    print()


section("T7 cost laws")
costtab("gen7/cost.json", 8)
costtab("pilot/cost.json", 10)
if os.path.exists(os.path.join(R, "c5f/cost.json")):
    costtab("c5f/cost.json", 5)

# T8 composition
section("T8 native-pending composition, gen 7")
d = J("gen7/compose.json")
t = d["tables(count, points, infinite, predicted_apply_seconds[E])"]
print(f"pending_native_total {d['pending_native_total']}; committed natives {d['committed_native_count']}; delegated {d['delegated_records']}; ledger cursor {d['ledger_cursor']}")
print()
for key in ("phase|state|live", "phase|admission_gen", "phase|queue_position", "phase|log10points", "phase|t", "phase|rank"):
    print(f"`{key}` (count, points, infinite, predicted Apply seconds [E]):")
    print()
    print("| key | count | points | infinite | pred. Apply s [E] |")
    print("|---|---:|---:|---:|---:|")
    for k, v in t[key].items():
        print(f"| {k} | {v[0]} | {v[1]:.3e} | {v[2]} | {v[3]:.0f} |")
    print()
ao = sorted(t["apply_owner"].items(), key=lambda kv: -kv[1][3])[:8]
tot = sum(v[3] for v in t["apply_owner"].values())
print("top Apply owners by predicted pending seconds [E]:")
print()
print("| owner | pending domains | points | pred. s [E] | share |")
print("|---|---:|---:|---:|---:|")
for k, v in ao:
    print(f"| {k} | {v[0]} | {v[1]:.3e} | {v[3]:.0f} | {pct(v[3] / tot)} |")
print()
print("escape: " + json.dumps(d["apply_level_escape[pending_points, outside_set_points, outside_staircase_points, domains, touching_outside_set, touching_outside_staircase]"]))
print("extrema escape: " + json.dumps(d["extrema_escape_by_phase[n, Amax>committed, Rmax>committed, Pmax>committed, Dmin<committed]"]))

# T9 saturation
section("T9 saturation (union of natives by record generation)")
for g in (3, 6, 7):
    p = f"gen{g}/saturation.json"
    d = J(p)
    print(f"`{p}` (draws per window {d['draws']}):")
    print()
    print("| gen | natives | est. new points | new points per native | union after |")
    print("|---:|---:|---:|---:|---:|")
    for w in d["totals_natives"]:
        print(f"| {w['gen']} | {w['natives']:.0f} | {w['new_points']:.3e} | {w['new_points_per_native']:.1f} | {w['union_after']:.3e} |")
    print()
    o = d["owners"].get("011101110111000")
    if o:
        print("hot owner 011101110111000: " + "; ".join(
            f"g{w['gen']} n {w['window']['members']} new frac pts {pct(w['window']['new_fraction_points'])} cpu {pct(w['window']['new_fraction_cpu'])}" for w in o["natives"]))
        print()

# T10 potential
section("T10 potential check")
for p in ("gen3/potential.json", "gen6/potential.json", "gen7/potential.json", "c5f/potential.json",
          "four-loop/fg-potential.json", "four-loop/bmw-potential.json", "four-loop/h-potential.json", "four-loop/x-potential.json"):
    d = J(p)
    gs = d["graphs"]
    cells = "; ".join(f"{g}: pairs {v['node_pairs']}, pos {v['positive_pairs']}, cycle {v['positive_cycle']}, max {v['max_potential']}" for g, v in sorted(gs.items()))
    print(f"- `{p}` nodes {d['nodes']}: {cells}")

# T11 stats
section("T11 checkpoint facts")
for g in (3, 6, 7):
    d = J(f"gen{g}/stats.json")
    print(f"- gen{g}: domains {d['domains']}, records {d['records']}, live {d['live_candidates']}, buckets {d['index_buckets']}, phase {json.dumps(d['domains_by_phase[count,infinite,empty,finite_points]'])}, "
          f"lag q {d['native_id_lag_quantiles[0.5,0.9,0.99,0.999,max]']}, out of order {d['native_records_out_of_id_order']}")
