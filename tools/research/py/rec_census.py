import json, sys, collections, math, time

qpath = "/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2/inputs/queries.json"
q = json.load(open(qpath))["queries"]
helper = {}
for e in q:
    if e["id"].startswith("owner-anchor"):
        helper[e["owner"]] = (e["max_numerator_rank"], e["power_bounds"]["max_positive_power"])

path = sys.argv[1]
limit = int(sys.argv[2]) if len(sys.argv) > 2 else None
t0 = time.time()
n = 0
kinds = collections.Counter()
phase_n = collections.Counter()
phase_sec = collections.Counter()
phase_succ = collections.Counter()
phase_events = collections.Counter()
secs = []
rank_rel = collections.Counter()   # Apply: rank - helper rank (installed owners)
a_rel = collections.Counter()
rank_hist = collections.Counter()
owner_n = collections.Counter()
owner_sec = collections.Counter()
installed_apply = 0
escaped_rank = 0
escaped_a = 0
escaped_either = 0
escaped_sec = 0.0
inside_sec = 0.0
cond = 0
succ_total = 0
sec_by_rank = collections.Counter()
with open(path, "rb") as f:
    for line in f:
        try:
            r = json.loads(line)
        except Exception:
            continue
        n += 1
        k = r.get("record_kind")
        kinds[k] += 1
        if k != "native_inspection":
            if limit and n >= limit:
                break
            continue
        ph = r.get("phase")
        s = r.get("seconds") or 0.0
        st = r.get("stats") or {}
        succ = st.get("successors", 0)
        phase_n[ph] += 1
        phase_sec[ph] += s
        phase_succ[ph] += succ
        phase_events[ph] += r.get("accepted_events", 0)
        cond += st.get("conditional_successors", 0)
        secs.append(s)
        owner = r.get("owner")
        rank = r.get("rank")
        amax = (r.get("power_bounds") or {}).get("max_positive_power")
        if ph == "Apply" and owner in helper:
            installed_apply += 1
            hr, ha = helper[owner]
            rr = None if rank is None else rank - hr
            rank_rel[rr if rr is None else max(-6, min(rr, 12))] += 1
            er = rank is None or rank > hr
            ea = ha is not None and (amax is None or amax > ha)
            if ha is not None and amax is not None:
                a_rel[max(-10, min(amax - ha, 12))] += 1
            escaped_rank += er
            escaped_a += ea
            if er or ea:
                escaped_either += 1
                escaped_sec += s
            else:
                inside_sec += s
            owner_n[owner] += 1
            owner_sec[owner] += s
            rank_hist[rank] += 1
            sec_by_rank[rank] += s
        if limit and n >= limit:
            break

secs.sort()
tot = sum(secs)
def q_(p):
    return secs[min(len(secs) - 1, int(p * len(secs)))] if secs else None
top = secs[int(0.99 * len(secs)):]
out = {
    "path": path, "lines": n, "elapsed_s": round(time.time() - t0, 1),
    "kinds": kinds,
    "native_by_phase": {p: {"n": phase_n[p], "sum_seconds": round(phase_sec[p], 1),
                             "successors": phase_succ[p], "events": phase_events[p]} for p in phase_n},
    "native_seconds_total": round(tot, 1),
    "seconds_quantiles": {p: q_(p) for p in (0.1, 0.5, 0.9, 0.99, 0.999)},
    "seconds_max": secs[-1] if secs else None,
    "top1pct_share_of_seconds": round(sum(top) / tot, 4) if tot else None,
    "frac_below_1ms": sum(1 for s in secs if s < 0.001) / len(secs),
    "frac_below_10ms": sum(1 for s in secs if s < 0.01) / len(secs),
    "conditional_successors": cond,
    "installed_apply": installed_apply,
    "apply_rank_minus_helper_rank_hist": dict(sorted(rank_rel.items(), key=lambda x: (x[0] is None, x[0] or 0))),
    "apply_A_minus_helper_A_hist(bounded helpers)": dict(sorted(a_rel.items())),
    "apply_escaped_helper_rank": escaped_rank,
    "apply_escaped_helper_A": escaped_a,
    "apply_escaped_either": escaped_either,
    "apply_seconds_escaped_vs_inside": [round(escaped_sec, 1), round(inside_sec, 1)],
    "apply_rank_hist": dict(sorted(rank_hist.items(), key=lambda x: (x[0] is None, x[0] or 0))),
    "apply_seconds_by_rank": {k: round(v, 1) for k, v in sorted(sec_by_rank.items(), key=lambda x: (x[0] is None, x[0] or 0))},
    "top_owners_by_seconds": [(o, owner_n[o], round(owner_sec[o], 1), helper[o]) for o, _ in owner_sec.most_common(8)],
}
print(json.dumps(out, indent=1, default=str))
