#!/usr/bin/env python3
"""Round-3 calibration tables from TMP/w0/oracle/r3/verify/*.json and r3/audits/*.json (markdown on stdout):
the gate table, then the partial-record and real-data union-cover validation table."""
import json
from pathlib import Path

R3 = Path("/common/dev/rustred/TMP/w0/oracle/r3")
NAMES = ["c4l-ordered-fg", "c4l-ordered-bmw", "c4l-ordered-h", "c4l-ordered-x", "c4l-ready-fg", "c4l-ready-bmw",
         "c4l-ready-h", "c4l-ready-x", "c5f-ordered", "c5f-ready", "frontier-fixture-fg"]


def load(path):
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError):
        return None


def gate(v):
    return (v.get("verdict") == "PASS" and isinstance(v.get("roots_total"), int) and v["roots_total"] >= 1
            and v.get("roots_independently_verified") == v["roots_total"] and v.get("family_closure_claim") is False)


print("| Case | Verify (req. closure) | Gate | Roots verified / total | Re-inspected | Aliases | Partials "
      "| Admitted successor / routed | Uncovered | Exact vs enum. disagreements | Verify s | Peak RSS GB | Audit (paired) |")
print("|" + "---|" * 13)
for name in NAMES:
    v = load(R3 / "verify" / f"{name}.json")
    a = load(R3 / "audits" / f"{name}.json")
    if v is None:
        print(f"| {name} | missing |" + " |" * 11)
        continue
    t = v["reinspection"]["tally"]
    c = v["containment"]
    rec = v["counts"]["records"]
    peak = (v.get("memory") or {}).get("at_end", {}).get("peak_rss_bytes")
    classes = v.get("violations_by_class") or {}
    verdict = v["verdict"] + ("" if not classes else " " + ",".join(f"{k} {n}" for k, n in sorted(classes.items())))
    audit = "n/a" if a is None else a.get("audit")
    pairing = None if a is None else (a.get("verifier_pairing") or {}).get("paired")
    print(f"| {name} | {verdict} | {'PASS' if gate(v) else 'fail'} | {v['roots_independently_verified']} / {v['roots_total']} "
          f"| {t['inspected']:,} | {rec.get('aliases', 0):,} | {rec.get('partials', 0):,} "
          f"| {t['admitted_successor_domains']:,} / {t['admitted_routed_domains']:,} | {t['uncovered']} "
          f"| {c['exact_vs_brute_force_disagreements']} | {v['timing']['total_seconds']:.1f} "
          f"| {'' if peak is None else f'{peak / 1e9:.2f}'} | {audit}{' (paired)' if pairing else ''} |")
print()
print("| Case | Axes | Partial D-cut checks covered / not / undecided (disagree) | Union sample: sampled / enumerable "
      "| Real covers: decided (covered / not) | ... only by a union (max records) | ... by one record | no earlier native "
      "| Synthetic partition2 / partition3 covered | Gap2 / gap3 not covered | Undecided | Disagreements | Union s |")
print("|" + "---|" * 13)
totals = {"real_union": 0, "real_not": 0, "real": 0, "synthetic": 0, "disagree": 0, "undecided": 0}
for name in NAMES:
    v = load(R3 / "verify" / f"{name}.json")
    if v is None:
        continue
    p = v["containment"]["partial_union_cover"]
    u = v.get("union_sample") or {}
    if not u:
        continue
    r = u["real_earlier_natives"]
    d = r["decisions"]
    s = u["synthetic"]
    fams = [d, s["partition2_with_real_distractors"], s["partition3_with_real_distractors"], s["one_layer_gap2"],
            s["one_layer_gap3"]]
    undecided = sum(f["undecided"] for f in fams)
    disagree = sum(f["disagreements_with_enumeration"] for f in fams)
    totals["real_union"] += r["covered_only_by_a_union"]
    totals["real_not"] += d["not_covered"]
    totals["real"] += d["checks"]
    totals["synthetic"] += sum(f["checks"] for f in fams[1:])
    totals["disagree"] += disagree + r["greedy_bookkeeping_mismatches"] + s["partitions_not_covered_by_enumeration"]
    totals["undecided"] += undecided
    print(f"| {name} | {u['axes']} | {p['covered']} / {p['not_covered']} / {p['undecided']} ({p['disagreements_with_slice_inclusion']}) "
          f"| {u['sampled']:,} / {u['enumerable']:,} | {d['checks']:,} ({d['covered']:,} / {d['not_covered']:,}) "
          f"| {r['covered_only_by_a_union']:,} ({r['covered_only_by_a_union_max_records']}) | {r['covered_by_one_record']:,} "
          f"| {r['no_intersecting_earlier_native']:,} "
          f"| {s['partition2_with_real_distractors']['covered']:,} / {s['partition3_with_real_distractors']['covered']:,} "
          f"| {s['one_layer_gap2']['not_covered']:,} / {s['one_layer_gap3']['not_covered']:,} | {undecided} "
          f"| {disagree + r['greedy_bookkeeping_mismatches'] + s['partitions_not_covered_by_enumeration']} | {u['seconds']:.1f} |")
print()
print("totals:", json.dumps(totals))
