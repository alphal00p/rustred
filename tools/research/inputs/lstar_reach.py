#!/usr/bin/env python3
"""Observed reachability from L* owners in a probe census (mask-level graph).

Reads DIR/census/transitions.tsv (cp5hop: source_phase, source_mask,
target_phase, target_mask, edges) and builds the directed graph on
(phase, mask) nodes. Every domain-level dependency edge maps to one mask-level
edge, so if no guard Apply mask is reachable from the Apply masks of the L*
owners in this graph, no domain descended from an L* domain lies in a guard
owner (the converse does not hold: the mask graph merges ancestries). Also
reports every owner Apply mask reached from L* that is not in L*, and the
Apply census of the non-L* owners (DIR/census/envelope.tsv).

Usage: lstar_reach.py DIR --lstar LSTAR.json --guards MASKS --owners OWNERS.txt [--output OUT.json]
"""
import argparse
import csv
import json
from collections import defaultdict, deque
from pathlib import Path


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("dir", type=Path)
    p.add_argument("--lstar", type=Path, required=True)
    p.add_argument("--guards", required=True)
    p.add_argument("--owners", type=Path, required=True)
    p.add_argument("--output", type=Path)
    args = p.parse_args(argv)
    lstar = set(json.loads(args.lstar.read_text()))
    guards = set(args.guards.split(","))
    owners = [line.strip() for line in open(args.owners) if line.strip()]
    succ = defaultdict(set)
    edge_count = 0
    for row in csv.DictReader(open(args.dir / "census/transitions.tsv"), delimiter="\t"):
        succ[(row["source_phase"], row["source_mask"])].add((row["target_phase"], row["target_mask"]))
        edge_count += int(row["edges"])
    start = [("Apply", m) for m in lstar]
    seen = set(start)
    queue = deque(start)
    while queue:
        node = queue.popleft()
        for nxt in succ.get(node, ()):
            if nxt not in seen:
                seen.add(nxt)
                queue.append(nxt)
    reached_apply = {m for ph, m in seen if ph == "Apply"}
    reached_owners = reached_apply & set(owners)
    env = {r["mask"]: r for r in csv.DictReader(open(args.dir / "census/envelope.tsv"), delimiter="\t")}
    non_lstar = {m: {k: env[m][k] for k in ("apply_domains", "inspected", "unbounded_A", "max_finite_A",
                                               "max_finite_rank")}
                 for m in owners if m not in lstar and m in env}
    out = {"dir": str(args.dir), "mask_graph_nodes": len(succ), "domain_edges": edge_count,
           "lstar_owners": len(lstar), "reached_nodes_from_lstar": len(seen),
           "reached_owner_apply_masks": len(reached_owners),
           "reached_owners_outside_lstar": sorted(reached_owners - lstar),
           "reached_guards": sorted(reached_owners & guards),
           "reached_non_owner_apply_masks": len(reached_apply - set(owners)),
           "lstar_unbounded_A_domains": sum(int(env[m]["unbounded_A"]) for m in lstar if m in env),
           "non_lstar_owner_census": non_lstar,
           "non_lstar_unbounded_A_domains": sum(int(v["unbounded_A"]) for v in non_lstar.values()),
           "guard_unbounded_A_domains": sum(int(env[m]["unbounded_A"]) for m in guards if m in env)}
    text = json.dumps(out, indent=1, sort_keys=True)
    if args.output:
        args.output.write_text(text + "\n")
    print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
