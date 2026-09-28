#!/usr/bin/env python3
"""Round-2 calibration table from r2/verify/*.json and r2/audits/*.json (markdown on stdout)."""
import json
import sys
from pathlib import Path

R2 = Path(__file__).resolve().parent
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


rows = []
print("| Case | Verify (req. closure) | Gate | Roots verified / total | Natives + partials re-inspected | Aliases | Partials "
      "| Admitted (successor + routed) | Successor events | Uncovered | Exact checks / inclusions | Enumerated incl. / non-incl. "
      "| Exact vs enum. disagreements | Partial union cover (agree / checks) | Verify s | Peak RSS GB | Audit (paired) | Audit indep. verified |")
print("|" + "---|" * 18)
for name in NAMES:
    v = load(R2 / "verify" / f"{name}.json")
    a = load(R2 / "audits" / f"{name}.json")
    if v is None:
        print(f"| {name} | missing |" + " |" * 16)
        continue
    t = v["reinspection"]["tally"]
    c = v["containment"]
    u = c.get("partial_union_cover", {})
    rec = v["counts"]["records"]
    peak = (v.get("memory") or {}).get("at_end", {}).get("peak_rss_bytes")
    classes = v.get("violations_by_class") or {}
    verdict = v["verdict"] + ("" if not classes else " " + ",".join(f"{k} {n}" for k, n in sorted(classes.items())))
    audit = "n/a" if a is None else a.get("audit")
    pairing = None if a is None else (a.get("verifier_pairing") or {}).get("paired")
    indep = None if a is None else (a.get("certification") or {}).get("independently_verified")
    print(f"| {name} | {verdict} | {'PASS' if gate(v) else 'fail'} | {v['roots_independently_verified']} / {v['roots_total']} "
          f"| {t['inspected']:,} | {rec.get('aliases', 0):,} | {rec.get('partials', 0):,} | {t['admitted_domains']:,} "
          f"| {t['successor_events']:,} | {t['uncovered']} | {c['exact_checks']:,} / {c['exact_inclusions']:,} "
          f"| {c['brute_force_confirmed_inclusions']:,} / {c['brute_force_confirmed_non_inclusions']:,} "
          f"| {c['exact_vs_brute_force_disagreements']} | {u.get('checks', 0) - u.get('disagreements_with_slice_inclusion', 0)} / {u.get('checks', 0)} "
          f"| {v['timing']['total_seconds']:.1f} | {'' if peak is None else f'{peak / 1e9:.2f}'} "
          f"| {audit}{' (paired)' if pairing else ''} | {indep} |")
if len(sys.argv) > 1:
    print()
    for name in NAMES:
        v = load(R2 / "verify" / f"{name}.json")
        if v is not None:
            print(name, json.dumps(v["certification"]["classes"]), v["counts"]["roots_oracle_closed"], "/", v["counts"]["roots"])
