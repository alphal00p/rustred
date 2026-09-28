#!/usr/bin/env python
"""Aggregate the owner-domain-scan --factor-census document (plan W0.7 Q3),
optionally weighting owners by their Apply native seconds from `census cost`.

usage: q3_summary.py FACTOR_CENSUS.json [COST.json]
"""
import collections
import json
import sys

doc = json.load(open(sys.argv[1]))
share = {}
if len(sys.argv) > 2:
    for o in json.load(open(sys.argv[2]))["owners"]:
        share[o["owner"]] = o["seconds_share"]
roles = collections.defaultdict(lambda: collections.Counter())
wroles = collections.defaultdict(lambda: collections.Counter())
rules = terms = affine_cases = 0
mismatches = None
for o in doc["owners"]:
    if "coefficient_denominator_mismatches" in o:
        mismatches = (mismatches or 0) + o["coefficient_denominator_mismatches"]
    rules += o["rules"]
    terms += o["rhs_terms"]
    affine_cases += o["affine_cases"]
    w = share.get(o["owner"], 0.0)
    for role, t in o["roles"].items():
        c = roles[role]
        for k in ("occurrences", "distinct", "skipped_above_max_terms", "factorization_failed",
                  "distinct_affine_only", "occurrences_affine_only", "distinct_index_affine",
                  "occurrences_index_affine", "distinct_factors"):
            c[k] += t[k]
        for cls, (d, occ) in t["factor_classes(distinct_pairs,occurrence_weighted)"].items():
            c["factor_occ:" + cls] += occ
            c["factor_distinct:" + cls] += d
        if t["occurrences"]:
            wroles[role]["affine_only"] += w * t["occurrences_affine_only"] / t["occurrences"]
            wroles[role]["index_affine"] += w * t["occurrences_index_affine"] / t["occurrences"]
            wroles[role]["weight"] += w
print(f"owners {len(doc['owners'])}, rules {rules}, affine cases {affine_cases}, rhs terms {terms}; limits {doc['factor_census_limits']}; "
      f"prepare {doc['prepared_seconds']:.1f} s, census {doc['factor_census_seconds']:.1f} s")
print()
if mismatches is None:
    print("coefficient denominators: legacy output (separate 'coefficient_denominator' role; it duplicates 'term_denominator' by construction)")
else:
    print(f"coefficient denominators differing from the stored term denominator: {mismatches} of {terms} rhs terms "
          "(the term denominator is stored as a copy of the coefficient denominator, so 'term_denominator' covers both)")
print()
print("| role | occurrences | distinct polys | skipped/failed | polys all-factors affine in n, integer coefficients (occ.) | polys all-factors at most affine in n (occ.) | CPU-weighted affine-only | CPU-weighted index-affine | distinct irreducible factors (sum over owners) |")
print("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
for role, c in sorted(roles.items()):
    w = wroles[role]
    wa = w["affine_only"] / w["weight"] if w["weight"] else float("nan")
    wi = w["index_affine"] / w["weight"] if w["weight"] else float("nan")
    print(f"| {role} | {c['occurrences']} | {c['distinct']} | {c['skipped_above_max_terms']}/{c['factorization_failed']} | "
          f"{100 * c['occurrences_affine_only'] / c['occurrences']:.1f}% | {100 * c['occurrences_index_affine'] / c['occurrences']:.1f}% | "
          f"{100 * wa:.1f}% | {100 * wi:.1f}% | {c['distinct_factors']} |")
print()
print("Irreducible factors by class (occurrence-weighted: each factor counted once per rule occurrence of its polynomial):")
print()
classes = ["constant", "base_only", "affine_index", "affine_index_base_offset", "affine_index_base_slope", "nonlinear_index"]
print("| role | " + " | ".join(classes) + " |")
print("|---|" + "---:|" * len(classes))
for role, c in sorted(roles.items()):
    tot = sum(c["factor_occ:" + k] for k in classes)
    print(f"| {role} | " + " | ".join(f"{100 * c['factor_occ:' + k] / tot:.1f}%" for k in classes) + " |")
