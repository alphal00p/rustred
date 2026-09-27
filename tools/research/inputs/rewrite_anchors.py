#!/usr/bin/env python3
"""Rewrite the full-orthant anchor (helper) rows of an owner-domain query document.

For query documents not produced by plan_renormalization_entry_queries.py
(the four-loop controls), apply the same `rustred.helper-bounds.json.v1`
overrides the planner accepts: each listed owner's anchor gets the given
max_numerator_rank and max_positive_power (null = unbounded) and the id
`<prefix>r<R>-a<A|none>-<mask>`. Refuses unless every root row of the owner
stays contained in its anchor (walking/queue.rs Domain::contains) and the
anchor stays a full orthant (lower 0, upper null, no D bounds).

Usage: rewrite_anchors.py --queries IN.json --anchor-prefix rank12-anchor-
         --bounds BOUNDS.json --new-prefix hyb-anchor- --output OUT.json
"""
import argparse
import json
import sys


def contains(container, candidate):
    if container["owner"] != candidate["owner"]:
        return False
    r1, r2 = container["max_numerator_rank"], candidate["max_numerator_rank"]
    if r1 is not None and (r2 is None or r2 > r1):
        return False
    p1, p2 = container["power_bounds"], candidate["power_bounds"]
    for key, later in (("max_positive_power", False), ("min_power_difference", True), ("max_power_difference", False)):
        if p1[key] is not None and (p2[key] is None or (p2[key] < p1[key] if later else p2[key] > p1[key])):
            return False
    return (all(a <= b for a, b in zip(container["lower"], candidate["lower"]))
            and all(a is None or (b is not None and b <= a) for a, b in zip(container["upper"], candidate["upper"])))


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--queries", required=True)
    p.add_argument("--anchor-prefix", required=True, action="append")
    p.add_argument("--bounds", required=True)
    p.add_argument("--new-prefix", required=True)
    p.add_argument("--output", required=True)
    args = p.parse_args(argv)
    document = json.load(open(args.queries))
    bounds = json.load(open(args.bounds))
    if bounds.get("schema") != "rustred.helper-bounds.json.v1":
        sys.exit("bounds schema")
    owners = bounds["owners"]
    rows = document["queries"]
    anchors = {}
    for row in rows:
        if any(row["id"].startswith(prefix) for prefix in args.anchor_prefix):
            if row["owner"] in anchors:
                sys.exit(f"two anchors for {row['owner']}")
            anchors[row["owner"]] = row
    missing = sorted(set(owners) - set(anchors))
    if missing:
        sys.exit(f"owners without an anchor row: {missing}")
    for mask, b in owners.items():
        row = anchors[mask]
        arity = len(mask)
        if row["lower"] != [0] * arity or row["upper"] != [None] * arity:
            sys.exit(f"{row['id']} is not a full orthant")
        row["max_numerator_rank"] = b["max_numerator_rank"]
        row["power_bounds"] = {"max_positive_power": b["max_positive_power"], "min_power_difference": None,
                               "max_power_difference": None}
        power = "none" if b["max_positive_power"] is None else b["max_positive_power"]
        row["id"] = f"{args.new_prefix}r{b['max_numerator_rank']}-a{power}-{mask}"
    for row in rows:
        anchor = anchors.get(row["owner"])
        if anchor is not None and row is not anchor and not contains(anchor, row):
            sys.exit(f"root {row['id']} is not contained in its anchor {anchor['id']}")
    ids = [r["id"] for r in rows]
    if len(set(ids)) != len(ids):
        sys.exit("ids collide")
    with open(args.output, "w") as f:
        f.write(json.dumps(document, sort_keys=True, indent=2) + "\n")
    print(json.dumps({"rows": len(rows), "rewritten": len(owners),
                      "unbounded_A_anchors": sum(1 for r in anchors.values()
                                                 if r["power_bounds"]["max_positive_power"] is None)}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
