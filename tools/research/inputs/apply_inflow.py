#!/usr/bin/env python3
"""Apply-domain inflow per owner by frame kind (census transitions of a walk).

For every owner mask the dependency edges INTO its Apply domains are split by
the frame in which they arrive:
  A-self       Apply successors of the same owner (inherit the parent's frame)
  A-other      Apply successors of another owner (installed target owner; global,
               identity frame)
  R-self       Route domain on the owner's own mask (identity route)
  R-transport  Route domain on another mask, transported by that mask's route
               witness (the frame the I2 rewrite changes)
With one automorphism per owner (route_witness_rewrite --frame owner) routed
images into an owner stay mutually consistent; A-other and R-self arrivals stay
in the identity frame and can stop sharing containers with the routed ones.

Inputs: census/transitions.tsv and census/envelope.tsv of cp5hop (run_four.py
or probe.py output). Usage:
  apply_inflow.py LABEL=CENSUS_DIR [LABEL=CENSUS_DIR ...] [--top N] [--output OUT.json]
"""
import argparse
import collections
import csv
import json
from pathlib import Path

KINDS = ("A-self", "A-other", "R-self", "R-transport")


def inflow(census):
    by_owner = collections.defaultdict(collections.Counter)
    for r in csv.DictReader(open(census / "transitions.tsv"), delimiter="\t"):
        if r["target_phase"] != "Apply":
            continue
        owner, edges = r["target_mask"], int(r["edges"])
        same = r["source_mask"] == owner
        if r["source_phase"] == "Apply":
            by_owner[owner]["A-self" if same else "A-other"] += edges
        else:
            by_owner[owner]["R-self" if same else "R-transport"] += edges
    envelope = {r["mask"]: r for r in csv.DictReader(open(census / "envelope.tsv"), delimiter="\t")}
    rows = {}
    for owner in sorted(set(by_owner) | set(envelope)):
        e = envelope.get(owner, {})
        c = by_owner.get(owner, collections.Counter())
        rows[owner] = {"apply_domains": int(e.get("apply_domains") or 0), "inspected": int(e.get("inspected") or 0),
                       **{k: c.get(k, 0) for k in KINDS}}
    return rows


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("arms", nargs="+")
    p.add_argument("--top", type=int, default=15)
    p.add_argument("--output", type=Path)
    args = p.parse_args(argv)
    out = {}
    for spec in args.arms:
        label, _, census = spec.partition("=")
        out[label] = inflow(Path(census))
    if args.output:
        args.output.write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")
    first = next(iter(out.values()))
    order = sorted(first, key=lambda o: -first[o]["inspected"])[: args.top]
    print("owner\t" + "\t".join(f"{label}:inspected\t{label}:" + "/".join(KINDS) for label in out))
    for owner in order:
        cells = []
        for rows in out.values():
            r = rows.get(owner, {})
            cells.append(f"{r.get('inspected', 0)}\t" + "/".join(str(r.get(k, 0)) for k in KINDS))
        print(owner + "\t" + "\t".join(cells))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
