#!/usr/bin/env python3
"""Later hits on domains avoided at admission, and calibrated pending creation weights (routecensus fix round).

usage: route_hits.py RECEIPT_DIR [HITS_DIR] [REPS]

Joins RECEIPT_DIR/gen7-route-rows.jsonl (census route --rows) with
HITS_DIR/gen7-route-hits-rows.jsonl (census route-hits --rows; HITS_DIR defaults
to RECEIPT_DIR/hits), and reads HITS_DIR/gen7-route-hits.json (exact later-hit
totals) and HITS_DIR/gen7-route-hits-pps.jsonl (census route-hits --pps-adm:
admitted domains drawn PPS by their later hits, with their coverage at
admission). Only prints.

1. Later hits. A domain that an admission-time union cover never creates cannot
   receive the requests that later land on it (distinct transition in-edges from
   inspected sources and alias in-edges, excluding its creator edge and self edges).
   Each of them needs its own admission under the lever: a single-container hit
   elsewhere (k' = 1 anchor) or a union test and a k'-anchor alias (k' <= a, the
   anchors of the domain's own cover). The later hits are heavy-tailed (old hub
   domains), so the redirected total T comes from the PPS-by-later-hits sample
   (Hansen-Hurwitz: exact total x covered share of the draws), h = T / N_cov with
   N_cov from the uniform admission sample. Reports T, h, the admission-request
   count and the net RSS factor
       1 / ((1 - c) + c [(a - 1) + h (k' - 1)] e / B)
   at the late (admission g7) mix for k' = 1 and k' = a, with B = 0.5-0.84 KB/domain
   and e = 8-16 B/edge [E].
2. Calibrated pending creation weights. The pending predictor (per-(owner, rank bound)
   means over g>=6 Route natives) ignores that, within a group, covered natives create
   fewer domains. Ratio calibration by coverage status on the historical g6-g7 Route
   natives (leave-one-out predictor): R_s = sum ht*created / sum ht*pred_loo over draws
   with status s under the analogous historical anchor rule; the calibrated pending
   weight is pred * R_status [E]. Out-of-sample check: fit on record g6, predict g7.
"""
import collections
import json
import math
import os
import random
import sys

d = sys.argv[1]
hd = sys.argv[2] if len(sys.argv) > 2 and not sys.argv[2].isdigit() else os.path.join(d, "hits")
REPS = int(sys.argv[-1]) if sys.argv[-1].isdigit() else 200
KB = (0.5, 0.84)
EB = (8, 16)
ADM = ["natives_before_creator_commit", "earlier_non_delegated", "all_earlier_ids"]
PEND = ["all_natives", "earlier_non_delegated", "all_earlier_ids", "all_other_domains"]
HIST = {"all_natives": "natives_before_dispatch", "earlier_non_delegated": "earlier_non_delegated",
        "all_earlier_ids": "all_earlier_ids", "all_other_domains": "all_earlier_ids"}

hits = {}
for line in open(os.path.join(hd, "gen7-route-hits-rows.jsonl")):
    h = json.loads(line)
    hits[(h["sample"], h["id"])] = h
rows = collections.defaultdict(lambda: collections.defaultdict(list))
n = 0
for line in open(os.path.join(d, "gen7-route-rows.jsonl")):
    r = json.loads(line)
    k = (r["sample"], r["id"])
    assert k in hits, k
    assert "h" not in r
    r["h"] = hits[k]
    rows[r["sample"]][r["stratum"]].append(r)
    n += 1
assert n == len(hits), (n, len(hits))
hj = json.load(open(os.path.join(hd, "gen7-route-hits.json")))
exact = hj["later_hits_exact(admitted domains; excludes the creator edge and self edges)"]


def cov(r, s):
    e = r["evals"][s]
    return e is not None and e["uncovered"] == 0


def flat(strata, flt=None):
    return [r for v in strata.values() for r in v if flt is None or flt(r)]


def later(r):
    return r["h"]["later_transition"] + r["h"]["later_alias"]


def table(head, body):
    out = ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
    out += ["| " + " | ".join(str(c) for c in b) + " |" for b in body]
    print("\n".join(out))
    print()


def num(x):
    return f"{x / 1e6:.1f}M" if abs(x) >= 1e6 else f"{x / 1e3:.1f}k" if abs(x) >= 1e4 else f"{x:.3g}"


# ------------------------------------------------------------------ 1. later hits
A = rows["all_domains_at_admission"]
PH = {"both": lambda r: True, "Route": lambda r: r["stratum"].startswith("Route"), "Apply": lambda r: r["stratum"].startswith("Apply")}
late = lambda r: r["stratum"].endswith("_g7")
early = lambda r: not r["stratum"].endswith("_g7")

print("## Later hits on domains at their admission (gen 7; census route-hits joined to census route rows)")
print()
print("Consistency: HT estimate over all admission draws vs the exact mean over every admitted domain.")
print()
body = []
for ph in ["Route", "Apply"]:
    rs = flat(A, PH[ph])
    W = sum(r["ht"] for r in rs)
    ex = exact[f"{ph}|all"]
    body.append([ph, num(W), num(ex["domains"]), f"{sum(r['ht'] * r['h']['later_transition'] for r in rs) / W:.3f}", f"{ex['mean_later_transition']:.3f}",
                 f"{sum(r['ht'] * r['h']['later_alias'] for r in rs) / W:.3f}", f"{ex['mean_later_alias']:.3f}",
                 f"{100 * sum(r['ht'] * (later(r) == 0) for r in rs) / W:.1f}%", f"{100 * ex['share_without_later_hits']:.1f}%"])
table(["phase", "HT domains", "exact domains", "later transition HT", "exact", "later alias HT", "exact", "no later hit HT", "exact"], body)

print("Exact later hits per admitted domain by admission generation (every domain; later hits of recent admissions are censored):")
print()
body = []
for k, v in sorted(exact.items(), key=lambda kv: kv[0]):
    if "admission_g" in k and v["domains"] >= 1000:
        body.append([k, num(v["domains"]), f"{v['mean_later_transition']:.2f}", f"{v['mean_later_alias']:.3f}", f"{100 * v['share_without_later_hits']:.1f}%"])
table(["phase|admission gen", "domains", "mean later transition in-edges", "mean later alias in-edges", "no later hit"], body)

# PPS-by-later-hits sample (census route-hits --pps-adm): hit-weighted coverage at admission.
PPS = collections.defaultdict(list)
for line in open(os.path.join(hd, "gen7-route-hits-pps.jsonl")):
    q = json.loads(line)
    PPS[q["phase"]].append(q)
PJ = hj["pps_by_later_hits_at_admission"]
K = PJ["draws_per_phase"]
qcov = lambda q, s: q["evals"][s] is not None and q["evals"][s]["uncovered"] == 0
qa = lambda q, s: max(q["evals"][s]["anchors_used"], 1)


def pps_mean(ph, f):
    """Hansen-Hurwitz mean over the K PPS draws of phase ph (with multiplicity) and its standard error."""
    ys = [(f(q), q["mult"]) for q in PPS[ph]]
    k = sum(m for _, m in ys)
    assert k == K, (k, K)
    m = sum(y * w for y, w in ys) / k
    v = sum(w * (y - m) ** 2 for y, w in ys) / (k - 1)
    return m, math.sqrt(v / k)


H = {}  # (phase, set) -> dict
print(f"Later hits landing on domains covered at admission. Redirected hits T = (exact later hits of the phase) x (share of the {K} PPS-by-later-hits")
print("draws per phase that are covered) [M, Hansen-Hurwitz; SE from the binomial draw]; h = T / N_cov with N_cov from the uniform admission sample.")
print("The uniform-sample h (HT mean over the covered uniform draws) under-samples the heavy tail (see the consistency table) and is shown for comparison.")
print()
body = []
for ph in ["Route", "Apply", "both"]:
    for s in ADM:
        rs = [r for r in flat(A, PH[ph]) if cov(r, s)]
        Nc = sum(r["ht"] for r in rs)
        hu = sum(r["ht"] * later(r) for r in rs) / Nc
        z = sum(r["ht"] * (later(r) == 0) for r in rs) / Nc
        x1 = sum(r["ht"] * (max(r["evals"][s]["anchors_used"], 1) - 1) for r in rs) / Nc
        phs = ["Route", "Apply"] if ph == "both" else [ph]
        T = Tv = Z = Zv = an = 0.0
        for p in phs:
            Wp = PJ["by_phase"][p]["later_hits_total"]
            m, se = pps_mean(p, lambda q: float(qcov(q, s)))
            T += Wp * m
            Tv += (Wp * se) ** 2
            mz, sz = pps_mean(p, lambda q: (qa(q, s) - 1) if qcov(q, s) else 0.0)
            Z += Wp * mz
            Zv += (Wp * sz) ** 2
            ma, _ = pps_mean(p, lambda q: float(qcov(q, "natives_before_creator_commit") and not qcov(q, "all_earlier_ids")))
            an += Wp * ma
        Wt = sum(PJ["by_phase"][p]["later_hits_total"] for p in phs)
        H[(ph, s)] = {"covered": Nc, "T": T, "T_se": math.sqrt(Tv), "h": T / Nc, "h_uniform": hu, "x1": x1, "Z": Z, "Z_se": math.sqrt(Zv)}
        body.append([ph, s, num(Nc), f"{100 * T / Wt:.1f}% ± {100 * math.sqrt(Tv) / Wt:.1f}", f"**{num(T)}** ± {num(math.sqrt(Tv))}", f"**{T / Nc:.2f}**", f"{hu:.2f}",
                     f"{100 * z:.1f}%", f"{x1:.1f}", f"{Z / Nc:.1f}", f"{100 * an / Wt:.2f}%"])
table(["phase", "anchor set at admission", "covered domains N_cov (uniform)", "share of later hits on covered domains (PPS)", "redirected later hits T (history)",
       "h = T / N_cov", "h, uniform sample", "covered without later hit (uniform)", "E[a_i - 1] (uniform)", "E_hits[(a_i - 1)] x T / N_cov",
       "hits with merged-native cover but no earlier-ID cover (set-nesting anomaly)"], body)

# ------------------------------------------------------------ admission requests
tot_adm = exact["Route|all"]["domains"] + exact["Apply|all"]["domains"]
tot_hits = PJ["by_phase"]["Route"]["later_hits_total"] + PJ["by_phase"]["Apply"]["later_hits_total"]
print(f"Admission requests under an admission-time union cover (history, first level [E]; today's misses = the {num(tot_adm)} admitted domains;")
print(f"later hits of all admitted domains {num(tot_hits)} distinct edges). A redirected hit needs a single-container search elsewhere and, when that fails")
print("(k' > 1), a union test; the redirected count is in distinct edges, a lower bound on requests (repeats of one source collapse onto one edge).")
print()
body = []
for s in ADM:
    x = H[("both", s)]
    body.append([s, num(tot_adm), num(x["covered"]), f"{num(x['T'])} ± {num(x['T_se'])}", f"{num(tot_adm)} - {num(tot_adm + x['T'])}",
                 f"{(tot_adm + x['T']) / tot_adm:.2f}x"])
table(["anchor set", "misses today (each runs a union test)", "domains avoided", "redirected later hits T",
       "union tests: misses only (every redirected hit single-contained) - misses + every redirected hit", "upper / today's misses"], body)

# ------------------------------------------------------------ net RSS factor


def late_share(ph, s):
    rs = flat(A, lambda r: late(r) and PH[ph](r))
    allg7 = flat(A, late)
    W = sum(r["ht"] for r in allg7)
    return sum(r["ht"] for r in rs if cov(r, s)) / W  # share of ALL g7 admissions avoided by the lever of this phase


def factor(c, extra, B, e):
    return 1.0 / ((1 - c) + c * extra * e / (B * 1000))


print("Net RSS factor at the late (admission g7) mix [E, first level]: 1 / ((1 - c) + c x e / B), B = 0.5-0.84 KB/domain, e = 8-16 B/edge,")
print("c = late avoided share. x = extra edges per avoided domain over its history: k' = 1 (every redirected hit finds a single container):")
print("x = E[a_i - 1] (uniform sample; a_i = anchors of the cover, greedy first-hit, single container = 1); k' = a_i (every redirected hit")
print("needs as many anchors as the cover; worst case): x = E[a_i - 1] + sum_cov (a_i - 1) h_i / N_cov, the sum from the PPS-by-later-hits sample.")
print("h is the later hits of a covered domain of any admission generation (history); a g7 admission's own future hits are not observed.")
print()
body = []
RSS = {}
for ph in ["both", "Route"]:
    for s in ADM:
        c = late_share(ph, s)
        x = H[(ph, s)]
        x1 = x["x1"]
        xa = x1 + x["Z"] / x["covered"]
        k1 = (factor(c, x1, KB[0], EB[1]), factor(c, x1, KB[1], EB[0]))
        ka = (factor(c, xa, KB[0], EB[1]), factor(c, xa, KB[1], EB[0]))
        RSS[(ph, s)] = (c, k1, ka)
        body.append([ph, s, f"{100 * c:.1f}%", f"{1 / (1 - c):.2f}x", f"{x1:.1f}", f"{x['h']:.2f}", f"{xa:.1f}",
                     f"{k1[0]:.2f}-{k1[1]:.2f}x", f"{ka[0]:.2f}-{ka[1]:.2f}x", f"**{ka[0]:.2f}-{k1[1]:.2f}x**",
                     f"{x1 * EB[0]:.0f}-{x1 * EB[1]:.0f} B", f"{xa * EB[0]:.0f}-{xa * EB[1]:.0f} B"])
table(["lever phases", "anchor set", "late avoided share c", "factor without edges", "x at k'=1", "h", "x at k'=a",
       "net, k'=1", "net, k'=a", "stated range (worst k'=a, B 0.5, e 16 - best k'=1, B 0.84, e 8)",
       "edge bytes per avoided domain, k'=1", "edge bytes per avoided domain, k'=a"], body)
print("Break-even: the lever saves RSS only while x e < B, i.e. x < 31-105 extra edges per avoided domain (B 0.5-0.84 KB, e 16-8 B).")
print()

# ------------------------------------------------ 2. calibrated pending creation weights
J = json.load(open(os.path.join(d, "gen7-route.json")))
N = rows["route_natives"]
P = rows["route_pending"]
g67 = lambda r: r["stratum"][:2] in ("g6", "g7")
WK = {"created": ("created", "created"), "successors": ("successors", "successors"), "created_route": ("created_route", "created_route"),
      "created_apply": ("created_apply", "created_apply")}


def ratios(strata, hs, meas, pk, flt=g67):
    num_ = {True: 0.0, False: 0.0}
    den = {True: 0.0, False: 0.0}
    for r in flat(strata, flt):
        c = cov(r, hs)
        num_[c] += r["ht"] * r["w"][meas]
        den[c] += r["ht"] * r["h"]["pred_loo"][pk]
    return {c: num_[c] / den[c] for c in (True, False)}


def hist_shares(strata, hs, meas, pk, flt=g67):
    rs = flat(strata, flt)
    W = sum(r["ht"] for r in rs)
    M = sum(r["ht"] * r["w"][meas] for r in rs)
    Pp = sum(r["ht"] * r["h"]["pred_loo"][pk] for r in rs)
    return (sum(r["ht"] for r in rs if cov(r, hs)) / W, sum(r["ht"] * r["w"][meas] for r in rs if cov(r, hs)) / M,
            sum(r["ht"] * r["h"]["pred_loo"][pk] for r in rs if cov(r, hs)) / Pp)


print("## Calibrated creation weights of the Route native-pending (gen 7) [E]")
print()
print("Historical Route natives of record g6-g7 (the model generations; leave-one-out predictor): coverage-status ratios.")
print()
body = []
OFF = {}
for hs in ["natives_before_dispatch", "natives_before_commit", "earlier_non_delegated", "all_earlier_ids"]:
    for w in ["created", "successors", "created_route", "created_apply"]:
        cs, ms, ps = hist_shares(N, hs, *WK[w])
        R = ratios(N, hs, *WK[w])
        # out-of-sample: fit on g6, predict g7
        R6 = ratios(N, hs, *WK[w], flt=lambda r: r["stratum"].startswith("g6"))
        g7 = flat(N, lambda r: r["stratum"].startswith("g7"))
        pc = sum(r["ht"] * r["h"]["pred_loo"][WK[w][1]] * R6[True] for r in g7 if cov(r, hs))
        pu = sum(r["ht"] * r["h"]["pred_loo"][WK[w][1]] * R6[False] for r in g7 if not cov(r, hs))
        m7 = sum(r["ht"] * r["w"][WK[w][0]] for r in g7 if cov(r, hs)) / sum(r["ht"] * r["w"][WK[w][0]] for r in g7)
        raw7 = sum(r["ht"] * r["h"]["pred_loo"][WK[w][1]] for r in g7 if cov(r, hs)) / sum(r["ht"] * r["h"]["pred_loo"][WK[w][1]] for r in g7)
        OFF[(hs, w)] = m7 - pc / (pc + pu)
        body.append([hs, w, f"{100 * cs:.1f}%", f"{100 * ms:.1f}%", f"{100 * ps:.1f}%", f"{R[True]:.3f}", f"{R[False]:.3f}",
                     f"{100 * m7:.1f}%", f"{100 * raw7:.1f}%", f"{100 * pc / (pc + pu):.1f}%"])
table(["historical anchor set", "weight", "count share", "measured weight share", "raw predicted share (LOO)", "R covered", "R not covered",
       "g7: measured share", "g7: raw predicted", "g7: calibrated on g6"], body)

# in-sample reproduction check of the pending predictor
for r in flat(P):
    assert abs(r["h"]["pred"]["created"] - r["w"]["pred_created[E]"]) <= 1e-9 * max(1.0, r["w"]["pred_created[E]"]), r["id"]

pop = J["route_pending"]["population"]
print(f"Route native-pending ({num(pop)} domains): covered share by predicted weight, raw vs calibrated by the coverage status of the mapped historical rule.")
print()
body = []
for ps in PEND:
    hs = HIST[ps]
    for w in ["created", "successors", "created_route", "created_apply"]:
        meas, pk = WK[w]

        def calc(nst, pst, ps=ps, hs=hs, meas=meas, pk=pk):
            R = ratios(nst, hs, meas, pk)
            rs = flat(pst)
            c_raw = sum(r["ht"] * r["h"]["pred"][pk] for r in rs if cov(r, ps))
            u_raw = sum(r["ht"] * r["h"]["pred"][pk] for r in rs if not cov(r, ps))
            return c_raw, u_raw, R[True] * c_raw, R[False] * u_raw
        c_raw, u_raw, c_cal, u_cal = calc(N, P)
        # joint bootstrap of both samples
        rnd = random.Random(7)
        vals = []
        tots = []
        for _ in range(REPS):
            bn = {st: [rnd.choice(v) for _ in v] for st, v in N.items()}
            bp = {st: [rnd.choice(v) for _ in v] for st, v in P.items()}
            a1, b1, a2, b2 = calc(bn, bp)
            vals.append(a2 / (a2 + b2))
            tots.append(a2)
        m = sum(vals) / len(vals)
        sd = math.sqrt(sum((x - m) ** 2 for x in vals) / (len(vals) - 1))
        mt = sum(tots) / len(tots)
        sdt = math.sqrt(sum((x - mt) ** 2 for x in tots) / (len(tots) - 1))
        off = OFF[(hs, w)]
        c_up = (c_cal / (c_cal + u_cal) + off) * (c_cal + u_cal)
        gb = f"{c_cal * KB[0] / 1e6:.1f}-{c_cal * KB[1] / 1e6:.1f} GB" if w.startswith("created") else "-"
        gbu = f"{c_up * KB[0] / 1e6:.1f}-{c_up * KB[1] / 1e6:.1f} GB" if w.startswith("created") else "-"
        body.append([ps, hs, w, f"{100 * c_raw / (c_raw + u_raw):.1f}%", f"**{100 * c_cal / (c_cal + u_cal):.1f}%** ± {100 * sd:.1f}",
                     f"{100 * off:+.1f} pp", f"{100 * (c_cal / (c_cal + u_cal) + off):.1f}%",
                     num(c_raw), f"{num(c_cal)} ± {num(sdt)}", num(c_up), gb, gbu])
table(["pending anchor set", "calibrated by (historical rule)", "weight", "raw predicted share", "calibrated share",
       "g6->g7 out-of-sample offset (measured - calibrated)", "calibrated + offset",
       "raw covered total", "calibrated covered total", "covered total at calibrated + offset", "bytes (calibrated)", "bytes (calibrated + offset)"], body)
