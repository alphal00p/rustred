#!/usr/bin/env python3
"""Markdown tables of the routecensus receipt (census route / route-saturation).

usage: route_tables.py RECEIPT_DIR            # every table, gen7 first
       route_tables.py RECEIPT_DIR --step c5f # one step's route tables

Reads RECEIPT_DIR/<step>-route.json, gen7-route-saturation.json and
<step>.metrics.txt. Only prints; never writes.
"""
import json
import os
import sys

KB_PER_DOMAIN = (0.5, 0.84)  # measured legacy marginal / restore range (handoff 0.1 item 4)


def load(d, name):
    p = os.path.join(d, name)
    if not os.path.exists(p):
        return None
    with open(p) as f:
        return json.load(f)


def pct(x, nd=1):
    if x is None:
        return "-"
    return f"{100.0 * x:.{nd}f}%"


def num(x):
    if x is None:
        return "-"
    if abs(x) >= 1e6:
        return f"{x / 1e6:.2f}M"
    if abs(x) >= 1e4:
        return f"{x / 1e3:.1f}k"
    if abs(x) >= 100:
        return f"{x:.0f}"
    return f"{x:.3g}"


def table(head, rows):
    out = ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
    for r in rows:
        out.append("| " + " | ".join(str(c) for c in r) + " |")
    return "\n".join(out)


def fanout(j):
    rows = []
    for label, key in [("Route natives", "fanout_route_natives"), ("Apply natives", "fanout_apply_natives")]:
        a = j[key]["all"]
        q = lambda k, i: a[k]["quantiles[0.5,0.9,0.99,0.999,max]"][i]
        rows.append([label, num(a["natives"]), f"{a['seconds']['sum']:.0f}", f"{1e6 * q('seconds', 0):.1f}",
                     f"{a['successors(stats)']['mean']:.2f}", f"{q('successors(stats)', 0):.0f} / {q('successors(stats)', 2):.0f}",
                     f"{a['out_edges_to_route']['mean']:.2f}", f"{a['out_edges_to_apply']['mean']:.3f}",
                     f"{a['created_route']['mean']:.3f}", f"{a['created_apply']['mean']:.3f}",
                     num(a['created_route']['sum'] + a['created_apply']['sum']), f"{a['descendants(creator forest)']['mean']:.2f}"])
    print(table(["natives", "count", "seconds", "median us", "successors/native", "succ p50 / p99", "edges->Route", "edges->Apply",
                 "created Route/native", "created Apply/native", "created total", "descendants/native (nested)"], rows))
    print()
    rows = []
    for g, a in j["fanout_route_natives"]["by_record_generation"].items():
        rows.append([g, num(a["natives"]), f"{a['seconds']['sum']:.0f}", f"{a['successors(stats)']['mean']:.2f}",
                     f"{a['out_edges_to_apply']['mean']:.3f}", f"{a['created_route']['mean']:.3f}", f"{a['created_apply']['mean']:.3f}",
                     num(a['created_route']['sum']), num(a['created_apply']['sum'])])
    print(table(["Route natives by record gen", "count", "seconds", "successors/native", "edges->Apply/native",
                 "created Route/native", "created Apply/native", "created Route", "created Apply"], rows))
    print()


def creators(j):
    c = j["creator_kind_by_target[phase|class]"]
    rows = []
    for k in sorted(c):
        v = c[k]
        tot = sum(v.values())
        rows.append([k, num(tot), num(v.get("route_native", 0)), pct(v.get("route_native", 0) / tot), num(v.get("apply_native", 0)),
                     pct(v.get("apply_native", 0) / tot), v.get("none(initial)", 0), v.get("non_native", 0)])
    print(table(["target phase|class", "domains", "by Route natives", "share", "by Apply natives", "share", "no creator", "non-native creator"], rows))
    print()


def coverage(block, sets, weights, title):
    cov = block["coverage"]
    print(f"{title}: population {num(block['population'])}, strata {block['strata']}, distinct draws {block['distinct_draws']}")
    print()
    head = ["anchor set"] + [w for w in weights]
    rows = []
    for s in sets:
        bw = cov[s]["by_weight"]
        rows.append([s] + [pct(bw[w]["fully_covered_share"]) if w in bw else "-" for w in weights])
    print("Fully covered share by weight:")
    print()
    print(table(head, rows))
    print()
    rows = []
    for s in sets:
        c = cov[s]["by_weight"]["count"]
        v = c["vocabularies"]
        rows.append([s, pct(c["fully_covered_share"]), pct(c["mean_uncovered_fraction"]),
                     pct(v["d_only"]["gate_share(resid<=10%,pieces<=8)"]), pct(v["d_only"]["gate_share(resid<=10%,pieces<=2)"]),
                     pct(v["hull"]["gate_share(resid<=10%,pieces<=8)"]), pct(v["ar_c2"]["gate_share(resid<=10%,pieces<=8)"]),
                     pct(v["exact_pointwise"]["gate_share(resid<=10%,pieces<=8)"]), pct(c["unevaluated_share"], 2)])
    print("Count-weighted residual view (gate = residual <= 10% of |Q| in <= 8 pieces):")
    print()
    print(table(["anchor set", "fully covered", "mean uncovered fraction", "gate D-only", "gate D-only <=2 pieces", "gate hull",
                 "gate A/R (C2)", "gate exact", "unevaluated"], rows))
    print()
    rows = []
    for s in sets:
        p = cov[s]["partial(count-weighted)"]
        h = p["pieces_histogram"]
        qd = p["residual_fraction_quantiles[0.1,0.25,0.5,0.75,0.9]"]
        fmt = lambda d: ", ".join(f"{k}: {num(v)}" for k, v in sorted(d.items(), key=lambda kv: int(kv[0])))
        rows.append([s, num(p["estimated_domains"]), p["distinct_draws"], fmt(h.get("d_only", {})), fmt(h.get("ar_c2", {})),
                     " / ".join(f"{x:.3f}" for x in qd.get("d_only", [])), " / ".join(f"{x:.3f}" for x in qd.get("hull", [])),
                     " / ".join(f"{x:.3f}" for x in qd.get("exact_pointwise", []))])
    print("Partially covered domains (count-weighted estimates; residual fraction quantiles 0.1/0.25/0.5/0.75/0.9 over distinct draws):")
    print()
    print(table(["anchor set", "partial domains", "draws", "D-only pieces", "A/R pieces", "D-only resid frac", "hull resid frac",
                 "exact resid frac"], rows))
    print()


def by_group(block, sets, weights, title):
    cov = block["coverage"]
    groups = sorted(cov[sets[0]]["by_group"])
    head = ["group", "anchor set"] + weights
    rows = []
    for g in groups:
        for s in sets:
            bg = cov[s]["by_group"].get(g, {})
            rows.append([g, s] + [pct(bg[w]["fully_covered_share"]) if w in bg else "-" for w in weights])
    print(f"{title} (fully covered share by group and weight):")
    print()
    print(table(head, rows))
    print()


def creator_view(j):
    pb = j["pending_by_creator_coverage"]
    sets = ["natives_before_dispatch", "natives_before_commit", "earlier_non_delegated", "all_earlier_ids"]
    rows = []
    for name, v in pb.items():
        s = v["shares_of_draws"]
        rows.append([name, num(v["population"]), num(v["draws"]), pct(s.get("creator:route_native", 0)),
                     pct(s.get("creator:apply_native", 0))]
                    + [pct(s.get(f"route_native_creator_fully_covered_by:{x}", 0)) for x in sets]
                    + [pct(s.get("self_fully_covered_by:all_natives=true", 0)), pct(s.get("self_fully_covered_by:all_earlier_ids=true", 0))])
    print(table(["sample", "population", "draws", "creator Route native", "creator Apply native"]
                + [f"creator covered: {x}" for x in sets] + ["self covered: all_natives", "self covered: all_earlier_ids"], rows))
    print()
    rows = []
    for name, v in pb.items():
        s = v["shares_of_draws"]
        for x in ["natives_before_dispatch", "all_earlier_ids"]:
            for selfset in ["all_natives", "all_earlier_ids"]:
                cc = [s.get(f"joint[creator_covered_by:{x}={a}|self_covered_by:{selfset}={b}]", 0) for a, b in
                      [("true", "true"), ("true", "false"), ("false", "true"), ("false", "false")]]
                rows.append([name, x, selfset] + [pct(c) for c in cc])
    print("Joint shares (of all draws; Route-native creators only):")
    print()
    print(table(["sample", "creator anchor set", "self anchor set", "creator cov & self cov", "creator cov & self not",
                 "creator not & self cov", "neither"], rows))
    print()


def admission(j):
    a = j["all_domains_at_admission"]
    sets = ["natives_before_creator_commit", "earlier_non_delegated", "all_earlier_ids"]
    print(f"Admission census: population {num(a['population'])} admitted (non-initial) domains, strata {a['strata']}, distinct draws "
          f"{a['distinct_draws']}, ancestors evaluated {a['ancestors_evaluated']}, max_up {a['max_up']}, sampled depth quantiles "
          f"{a['sampled_depth_quantiles[0.5,0.9,0.99,max]']}")
    print()
    by_group(a, sets, ["count", "count_native", "count_delegated", "count_pending", "descendants", "points"], "Covered at admission")
    t = a["ancestor_cascade(estimated domains, HT)"]
    rows = []
    for ph in ["Apply", "Route"]:
        dom = t.get(f"{ph}|domains", 0)
        for s in sets:
            sc = t.get(f"{ph}|{s}|self_covered", 0)
            an = t.get(f"{ph}|{s}|ancestor_covered_only", 0)
            so = t.get(f"{ph}|{s}|self_or_ancestor_covered", 0)
            rows.append([ph, s, num(dom), f"{num(sc)} ({pct(sc / dom)})", f"{num(an)} ({pct(an / dom)})", f"{num(so)} ({pct(so / dom)})",
                         pct(t.get(f"{ph}|chain_truncated", 0) / dom)])
    print(table(["phase", "anchor set", "domains", "covered at admission", "ancestor covered only", "self or ancestor [E]", "chain truncated"], rows))
    print()
    return t


def saturation(s):
    for kind, rows_in in s["windows"].items():
        rows = []
        for r in rows_in:
            rows.append([r["gen"], num(r["members"]), num(r["infinite"]), f"{r['sum_points']:.3e}", pct(r["new_fraction_points"], 2),
                         f"{r['new_points_est']:.3e}", f"{r['union_after']:.3e}", num(r["new_points_per_member"]),
                         pct(r["new_fraction_uniform_domain"], 1), r["draws_points"]])
        print(f"Route saturation, {kind} (draws per estimate {s['draws']}):")
        print()
        print(table(["gen", "members", "infinite", "sum points", "new fraction (points)", "new points", "union after",
                     "new points/member", "new fraction (uniform domain)", "draws"], rows))
        print()


def metrics(d):
    rows = []
    for f in sorted(os.listdir(d)):
        if f.endswith(".metrics.txt"):
            m = dict(l.rstrip("\n").rsplit(" ", 1) for l in open(os.path.join(d, f)) if " " in l)
            rows.append([f[:-12], m.get("wall_s"), m.get("user_sys_s"), m.get("max_rss_kb"), m.get("foreign_busy_cpus_avg"),
                         m.get("schedstat_run_delay_s(last sample, summed over threads)")])
    print(table(["step", "wall s", "user+sys s", "max RSS kB", "foreign busy CPUs (avg)", "run delay s"], rows))
    print()


def ram(j, t):
    """Domains a Route union-cover lever avoids, and bytes at 0.5-0.84 KB/domain [E]."""
    fr = j["fanout_route_natives"]["all"]
    created = fr["created_route"]["sum"] + fr["created_apply"]["sum"]
    cov = j["route_natives"]["coverage"]
    rows = []
    for s in ["natives_before_dispatch", "earlier_non_delegated", "all_earlier_ids"]:
        share = cov[s]["by_weight"]["created"]["fully_covered_share"]
        rows.append([f"Route natives covered ({s}): their first-level creations", num(created), pct(share), num(created * share)])
    pc = j["route_pending"]["coverage"]
    for s in ["all_natives", "earlier_non_delegated", "all_earlier_ids", "all_other_domains"]:
        bw = pc[s]["by_weight"]["pred_created[E]"]
        rows.append([f"Route pending covered at resume ({s}): their predicted first-level creations [E]", num(bw["total_weight"]),
                     pct(bw["fully_covered_share"]), num(bw["total_weight"] * bw["fully_covered_share"])])
    for ph in ["Route", "Apply"]:
        for s in ["natives_before_creator_commit", "earlier_non_delegated", "all_earlier_ids"]:
            dom = t.get(f"{ph}|domains", 0)
            sc = t.get(f"{ph}|{s}|self_covered", 0)
            so = t.get(f"{ph}|{s}|self_or_ancestor_covered", 0)
            rows.append([f"{ph} domains covered at admission ({s})", num(dom), pct(sc / dom), num(sc)])
            rows.append([f"{ph} domains covered at admission or with a covered ancestor ({s}) [E upper]", num(dom), pct(so / dom), num(so)])
    out = []
    for r in rows:
        n = float(r[3][:-1]) * 1e6 if r[3].endswith("M") else float(r[3][:-1]) * 1e3 if r[3].endswith("k") else float(r[3])
        out.append(r + [f"{n * KB_PER_DOMAIN[0] / 1e6:.1f}-{n * KB_PER_DOMAIN[1] / 1e6:.1f} GB"])
    print(table(["avoidable set (gen 7 history)", "base", "share", "domains", "bytes at 0.5-0.84 KB/domain [E]"], out))
    print()


ANCH = "anchors_used_by_fully_covered(greedy first-hit count)"


def anchors(j):
    """Anchors a union cover of a fully covered draw uses (greedy first-hit count: an upper bound on the
    minimum cover size; for |Q| > cap the points are sampled, a lower bound on the first-hit count)."""
    rows = []
    for block, sets in [("route_pending", ["all_natives", "earlier_non_delegated", "all_earlier_ids", "all_other_domains"]),
                        ("route_natives", ["natives_before_dispatch", "earlier_non_delegated", "all_earlier_ids"]),
                        ("all_domains_at_admission", ["natives_before_creator_commit", "earlier_non_delegated", "all_earlier_ids"])]:
        cov = j[block]["coverage"]
        for x in sets:
            a = cov[x].get(ANCH)
            if a is None:
                continue
            h = a["histogram(count-weighted)"]
            tot = sum(h.values())
            small = sum(v for k, v in h.items() if k in ("0(single)", "1", "2", "3", "4"))
            q = a["quantiles_distinct_draws[0.5,0.9,0.99,max]"]
            rows.append([block, x, f"{a['count_weighted_mean']:.1f}", f"{q[0]:.0f} / {q[1]:.0f} / {q[2]:.0f} / {q[3]:.0f}",
                         pct(small / tot if tot else None), pct(h.get(">64", 0) / tot if tot else None)])
    print("Anchors used by the fully covered draws (count-weighted mean; distinct-draw quantiles p50/p90/p99/max; share <= 4 anchors; share > 64):")
    print()
    print(table(["sample", "anchor set", "mean anchors", "p50 / p90 / p99 / max", "<= 4 anchors", "> 64 anchors"], rows))
    print()


def admission_by_gen(j):
    """Covered-at-admission share (count weight) per phase and admission generation."""
    cov = j["all_domains_at_admission"]["coverage"]
    sets = ["natives_before_creator_commit", "earlier_non_delegated", "all_earlier_ids"]
    keys = sorted(cov[sets[0]]["by_stratum"], key=lambda k: (k.split("|")[0], int(k.rsplit("g", 1)[1])))
    rows = []
    for k in keys:
        st = cov[sets[0]]["by_stratum"][k]
        if st["population"] < 1000:
            continue
        rows.append([k, num(st["population"]), st["distinct_draws"]] + [pct(cov[x]["by_stratum"][k]["fully_covered_share"]) for x in sets])
    print("Covered at admission by phase and admission generation (count weight; uniform draws within the stratum):")
    print()
    print(table(["phase|admission gen", "domains", "draws"] + sets, rows))
    print()
    return {k: {x: cov[x]["by_stratum"][k]["fully_covered_share"] for x in sets} | {"population": cov[sets[0]]["by_stratum"][k]["population"]}
            for k in keys}


def ram_rate(j, bygen):
    """Domain-creation rate factor of an admission-time union cover at the late (gen-7) mix [E, first level].

    The net RSS column charges only the cover's own (anchors - 1) extra edges. It omits the requests
    that later land on the avoided domain (each needs its own admission and a k'-anchor alias); the
    full range (k' = 1 .. a) is in route_hits.py."""
    sets = ["natives_before_creator_commit", "earlier_non_delegated", "all_earlier_ids"]
    anc = {x: j["all_domains_at_admission"]["coverage"][x].get(ANCH, {}).get("count_weighted_mean") for x in sets}
    g = [k for k in bygen if k.endswith("admission_g%d" % j["generation"])]
    rows = []
    for x in sets:
        pop = sum(bygen[k]["population"] for k in g)
        cov = sum(bygen[k]["population"] * bygen[k][x] for k in g) / pop
        rt = [k for k in g if k.startswith("Route")]
        rcov = sum(bygen[k]["population"] * bygen[k][x] for k in rt) / sum(bygen[k]["population"] for k in rt)
        ronly = sum(bygen[k]["population"] * bygen[k][x] for k in rt) / pop
        a = anc[x] or float("nan")
        # net bytes per avoided domain: 0.5-0.84 KB minus (anchors - 1) extra edges at 8-16 B/edge
        lo = KB_PER_DOMAIN[0] * 1000 - (a - 1) * 16
        hi = KB_PER_DOMAIN[1] * 1000 - (a - 1) * 8
        # RSS per unit of exploration relative to today when each avoided domain costs (anchors - 1) extra edges:
        # (1 - c) + c (a - 1) e / B, worst case (B = 0.5 KB, e = 16 B) and best case (B = 0.84 KB, e = 8 B).
        nw = 1 / ((1 - cov) + cov * (a - 1) * 16 / (KB_PER_DOMAIN[0] * 1000))
        nb = 1 / ((1 - cov) + cov * (a - 1) * 8 / (KB_PER_DOMAIN[1] * 1000))
        rows.append([x, num(pop), pct(rcov), pct(ronly), f"{1 / (1 - ronly):.2f}x", pct(cov), f"{1 / (1 - cov):.2f}x", f"{a:.1f}",
                     f"{lo:.0f}-{hi:.0f} B", f"{nw:.2f}-{nb:.2f}x"])
    print(f"Late-generation (admission g{j['generation']}) domain-creation view [E, first level, no cascade]:")
    print()
    print(table(["anchor set at admission", "g-last admitted domains", "Route covered", "Route-only lever: avoided share of all",
                 "Route-only factor", "both phases: covered", "both phases: factor 1/(1-c)",
                 "mean anchors (all gens)", "net bytes saved per avoided domain (0.5-0.84 KB minus (anchors-1) x 8-16 B)",
                 "both phases: net RSS factor incl. the cover's anchor edges only (k'=1; later hits: route_hits.py)"], rows))
    print()


def dispatch_view(j, a, bygen):
    """First-level domain creations avoided by a dispatch-time full-cover alias (a covered job emits no successors),
    Apply jobs (route-apply) and Route jobs (route), next to the admission-time union cover [E]."""
    fa = j["fanout_apply_natives"]["by_record_generation"]
    fr = j["fanout_route_natives"]["by_record_generation"]
    rc = j["route_natives"]["coverage"]
    ac = a["apply_natives"]["coverage"]
    ndom = sum(v["population"] for v in bygen.values())
    last = "g%d" % j["generation"]
    ca_last = fa[last]["created_route"]["sum"] + fa[last]["created_apply"]["sum"]
    cr_last = fr[last]["created_route"]["sum"] + fr[last]["created_apply"]["sum"]
    cra = j["fanout_route_natives"]["all"]
    cr_tot = cra["created_route"]["sum"] + cra["created_apply"]["sum"]
    ca_tot = a["apply_natives"]["created_exact_total"]
    rows = []
    for x, adm in [("natives_before_dispatch", "natives_before_creator_commit"), ("earlier_non_delegated", "earlier_non_delegated"),
                   ("all_earlier_ids", "all_earlier_ids")]:
        sa = ac[x]["by_weight"]["created"]["fully_covered_share"]
        sr = rc[x]["by_weight"]["created"]["fully_covered_share"]
        ga = ac[x]["by_group"]["record_" + last]["created"]["fully_covered_share"]
        gr = rc[x]["by_group"]["record_" + last]["created"]["fully_covered_share"]
        hist = ca_tot * sa + cr_tot * sr
        late = (ca_last * ga + cr_last * gr) / (ca_last + cr_last)
        late_a = ca_last * ga / (ca_last + cr_last)
        adm_hist = sum(v["population"] * v[adm] for v in bygen.values())
        adm_late = [k for k in bygen if k.endswith("admission_" + last)]
        al = sum(bygen[k]["population"] * bygen[k][adm] for k in adm_late) / sum(bygen[k]["population"] for k in adm_late)
        rows.append([x, f"{pct(ac[x]['by_weight']['count']['fully_covered_share'])} / {pct(rc[x]['by_weight']['count']['fully_covered_share'])}",
                     f"{pct(sa)} / {pct(sr)}", f"{num(ca_tot * sa)} + {num(cr_tot * sr)} = {num(hist)} ({pct(hist / ndom)})",
                     f"{hist * KB_PER_DOMAIN[0] / 1e6:.1f}-{hist * KB_PER_DOMAIN[1] / 1e6:.1f} GB",
                     f"{pct(late_a)} ({1 / (1 - late_a):.2f}x)", f"{pct(late)} ({1 / (1 - late):.2f}x)",
                     f"{adm}: {num(adm_hist)} ({pct(adm_hist / ndom)}); {pct(al)} ({1 / (1 - al):.2f}x)"])
    print(f"Dispatch-time full-cover alias vs admission-time union cover (first level [E]; history = all {num(ndom)} admitted domains, "
          f"late = creations by record-g{j['generation']} natives / admissions in g{j['generation']}):")
    print()
    print(table(["anchor set (dispatch)", "jobs covered Apply / Route (count)", "creations covered Apply / Route",
                 "history: creations avoided Apply + Route", "bytes at 0.5-0.84 KB", "late: Apply jobs only (G2' alias)",
                 "late: Apply + Route jobs", "admission-time same rule: history; late"], rows))
    print()


def route_step(d, step, full=True):
    j = load(d, f"{step}-route.json")
    if j is None:
        return None, None
    print(f"### {step}: {j['dir']} (generation {j['generation']})")
    print()
    e = j["edges"]
    print("Edges:", json.dumps(e))
    print()
    print("Route buckets:", json.dumps(j["route_buckets"]))
    print()
    print("Model [E]:", json.dumps(j["model[E]"]))
    print()
    fanout(j)
    creators(j)
    if j["route_pending"]["population"] > 0:
        coverage(j["route_pending"], ["all_natives", "earlier_non_delegated", "all_earlier_ids", "all_other_domains"],
                 ["count", "pred_seconds[E]", "pred_successors[E]", "pred_created[E]", "pred_created_apply[E]", "pred_created_route[E]", "points"],
                 "Route native-pending")
        if full:
            by_group(j["route_pending"], ["all_natives", "all_earlier_ids"], ["count", "pred_created[E]"], "Route pending by admission generation")
    coverage(j["route_natives"], ["natives_before_dispatch", "natives_before_commit", "earlier_non_delegated", "all_earlier_ids"],
             ["count", "seconds", "successors", "out_edges", "created", "created_apply", "created_route", "descendants", "points"],
             "Route natives (historical)")
    if full:
        by_group(j["route_natives"], ["natives_before_dispatch", "all_earlier_ids"], ["count", "successors", "created", "created_apply"],
                 "Route natives by record generation")
    if j["route_pending"]["population"] > 0:
        creator_view(j)
    t = admission(j)
    bygen = admission_by_gen(j)
    if full:
        anchors(j)
        ram_rate(j, bygen)
        a = load(d, f"{step}-route-apply.json") or load(os.path.join(d, "apply"), f"{step}-route-apply.json")
        if a is not None:
            coverage(a["apply_natives"], ["natives_before_dispatch", "natives_before_commit", "earlier_non_delegated", "all_earlier_ids"],
                     ["count", "seconds", "successors", "out_edges", "created", "created_apply", "created_route", "descendants", "points"],
                     "Apply natives (historical; census route-apply; seconds weight from uniform strata, see W0.7 PPS for CPU)")
            by_group(a["apply_natives"], ["natives_before_dispatch", "all_earlier_ids"], ["count", "seconds", "created"],
                     "Apply natives by record generation")
            dispatch_view(j, a, bygen)
    return j, t


def main():
    d = sys.argv[1]
    if "--step" in sys.argv:
        route_step(d, sys.argv[sys.argv.index("--step") + 1])
        return
    j, t = route_step(d, "gen7")
    if j is not None:
        print("### RAM view (gen 7)")
        print()
        ram(j, t)
    for name in ["gen7-route-saturation.json", "gen7-route-saturation-seed2.json"]:
        s = load(d, name)
        if s is not None:
            print(f"### Route point-space saturation (gen 7 checkpoint; {name})")
            print()
            saturation(s)
    for step in ["gen6", "gen3", "c5f"]:
        route_step(d, step, full=False)
    print("### Run metrics")
    print()
    metrics(d)


if __name__ == "__main__":
    main()
