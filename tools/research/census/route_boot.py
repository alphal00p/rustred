#!/usr/bin/env python3
"""Stratified bootstrap standard errors of the routecensus key shares.

usage: route_boot.py RECEIPT_DIR/gen7-route-rows.jsonl[,APPLY_ROWS.jsonl] [REPS]

Resamples distinct draws within each stratum (Horvitz-Thompson weights kept)
and reports each share with its bootstrap standard deviation. Also reports
the share of verdicts that used sampled points (|Q| > --cap), whose
"fully covered" may be overestimated. Only prints.
"""
import collections
import json
import math
import random
import sys

rows = collections.defaultdict(lambda: collections.defaultdict(list))
for path in sys.argv[1].split(","):
    for line in open(path):
        r = json.loads(line)
        rows[r["sample"]][r["stratum"]].append(r)
REPS = int(sys.argv[2]) if len(sys.argv) > 2 else 200


def share(strata, s, w, flt=None):
    num = den = 0.0
    for v in strata.values():
        for r in v:
            if flt and not flt(r):
                continue
            x = r["ht"] * r["w"][w]
            den += x
            ev = r["evals"][s]
            if ev is not None and ev["uncovered"] == 0:
                num += x
    return num / den if den else float("nan")


def boot(sample, s, w, flt=None):
    strata = rows[sample]
    base = share(strata, s, w, flt)
    rnd = random.Random(1)
    vals = []
    for _ in range(REPS):
        bs = {st: [rnd.choice(v) for _ in v] for st, v in strata.items()}
        vals.append(share(bs, s, w, flt))
    m = sum(vals) / len(vals)
    sd = math.sqrt(sum((x - m) ** 2 for x in vals) / (len(vals) - 1))
    return base, sd


route = lambda r: r["stratum"].startswith("Route")
apply_ = lambda r: r["stratum"].startswith("Apply")
g7 = lambda r: r["stratum"].endswith("_g7")
rg7 = lambda r: r["stratum"] == "Route|admission_g7"
ag7 = lambda r: r["stratum"] == "Apply|admission_g7"
cases = [
    ("route_pending", "all_natives", "count", None, ""),
    ("route_pending", "all_natives", "pred_created[E]", None, ""),
    ("route_pending", "earlier_non_delegated", "count", None, ""),
    ("route_pending", "all_earlier_ids", "count", None, ""),
    ("route_pending", "all_other_domains", "count", None, ""),
    ("route_natives", "natives_before_dispatch", "count", None, ""),
    ("route_natives", "natives_before_dispatch", "successors", None, ""),
    ("route_natives", "natives_before_dispatch", "created", None, ""),
    ("route_natives", "earlier_non_delegated", "created", None, ""),
    ("route_natives", "all_earlier_ids", "created", None, ""),
]
if "apply_natives" in rows:  # census route-apply --rows
    for s, w in [("natives_before_dispatch", "count"), ("natives_before_dispatch", "successors"), ("natives_before_dispatch", "created"),
                 ("earlier_non_delegated", "created"), ("all_earlier_ids", "created")]:
        cases.append(("apply_natives", s, w, None, ""))
    cases.append(("apply_natives", "natives_before_dispatch", "created", lambda r: r["stratum"].startswith("g7"), "g7"))
for s in ["natives_before_creator_commit", "earlier_non_delegated", "all_earlier_ids"]:
    for flt, lab in [(route, "Route"), (apply_, "Apply"), (rg7, "Route g7"), (ag7, "Apply g7"), (g7, "g7 both")]:
        cases.append(("all_domains_at_admission", s, "count", flt, lab))
print(f"bootstrap reps {REPS}")
for sample, s, w, flt, lab in cases:
    b, sd = boot(sample, s, w, flt)
    print(f"| {sample} | {lab} | {s} | {w} | {100 * b:.2f}% | {100 * sd:.2f} pp |")
print()
agg = collections.defaultdict(lambda: [0.0, 0.0, 0.0, 0.0])
for sample, strata in rows.items():
    for v in strata.values():
        for r in v:
            for s, ev in r["evals"].items():
                a = agg[(sample, s)]
                a[0] += r["ht"]
                if ev is None:
                    continue
                sampled = not ev["exact"]
                full = ev["uncovered"] == 0
                a[1] += r["ht"] * sampled
                a[2] += r["ht"] * full
                a[3] += r["ht"] * (full and sampled)
for (sample, s), a in sorted(agg.items()):
    print(f"| {sample} | {s} | sampled verdicts {100 * a[1] / a[0]:.3f}% | fully covered {100 * a[2] / a[0]:.2f}% | of which sampled {100 * a[3] / a[0]:.3f}% |")
print()
# Route pending by owner (HT-estimated domains; top owners), fully covered shares.
own = collections.defaultdict(lambda: [0.0, 0.0, 0.0, 0.0, 0])
for v in rows["route_pending"].values():
    for r in v:
        o = own[r["owner"]]
        o[0] += r["ht"]
        o[4] += 1
        for i, s in enumerate(["all_natives", "earlier_non_delegated", "all_earlier_ids"]):
            ev = r["evals"][s]
            o[1 + i] += r["ht"] * (ev is not None and ev["uncovered"] == 0)
tot = sum(o[0] for o in own.values())
print(f"route_pending owners in the sample: {len(own)}; estimated domains {tot:.0f}")
print("| owner | est. pending | share | draws | all_natives | earlier_non_delegated | all_earlier_ids |")
print("|---|---|---|---|---|---|---|")
for k, o in sorted(own.items(), key=lambda kv: -kv[1][0])[:12]:
    print(f"| {k} | {o[0]:.0f} | {100 * o[0] / tot:.1f}% | {o[4]} | {100 * o[1] / o[0]:.1f}% | {100 * o[2] / o[0]:.1f}% | {100 * o[3] / o[0]:.1f}% |")
