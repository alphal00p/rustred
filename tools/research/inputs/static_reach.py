#!/usr/bin/env python3
"""Static over-approximation of owner-level reachability (L_static).

Inputs (read-only):
  --selection   selection.json (owners, initial_frontier_routes = route maps)
  --scan        `rustred owner-domain-scan --unbounded-rank` result.json: every
                owner's rule RHS sign regions with requested rank None and
                unbounded positive powers (a superset of the target sectors of
                any Apply successor of any domain of that owner; guard validity
                and first-applicable priority are NOT resolved, which only adds
                edges)
  --guards      comma-separated guard-sensitive owner masks

Model (each rule is a superset of the walk's behaviour, walking/inspection.rs,
walking/routing.rs, routed/domain_overcover/visit.rs):
  Apply(o)  -> Sector(s) for every target sector s of a scan successor group of o
  Sector(s) -> nothing if the scan flags s as an exact-zero sector (ZeroSector
                 event: no domain is emitted);
               Apply(s) if s is an owner (installed target / literal route);
               Apply(root) and Sector(m) for every proper sub-mask m of root if
                 s has a route with owner root (route_cover emits the root
                 Apply cover and Route re-entries for sub-masks of the root
                 with at most `rank` removed axes; unbounded rank => all);
               nothing else (MissingRoute: a frontier, not a domain).
Owner o is in L_static iff no guard owner is reachable from Apply(o) and o is
not itself a guard owner.

Refinement (--one-hop HOP_TSV, repeatable): the scan is geometric only (it does
not test whether a term coefficient vanishes on a sign region), so its
unsupported support-change regions connect every owner to every other. A
native one-hop inspection of an owner's full orthant (rank <= R, A unbounded;
tools/research/inputs/one_hop.py) lists the target sectors that survive rule
selection and the exact coefficient tests. When the hop covers an owner with
0 frontiers and 0 errors, its Apply successors are replaced by those targets.
The refined closure is then an over-approximation for domains of rank <= R
of the refined owners (successor ranks can exceed R; that escape is not
covered and must be monitored).

Validation: --observed TRANSITIONS.tsv (census.rs / cp4scan key convention:
Apply owner index, 100000+mask non-owner Apply, 200000+mask Route, where mask
bit i = coordinate i) checks that every observed owner-level edge is inside the
static closure; any violation is printed and makes the exit status 1.

Outputs --output JSON: per owner the reachable owners, the reached guards,
the L_static list, edge counts and the validation result.
"""
import argparse
import json
import sys
from collections import defaultdict


def bits(mask):
    return sum(1 << i for i, c in enumerate(mask) if c == "1")


def mask_str(value, width):
    return "".join("1" if value >> i & 1 else "0" for i in range(width))


def proper_submasks(value):
    sub = (value - 1) & value
    while True:
        yield sub
        if sub == 0:
            return
        sub = (sub - 1) & value


def build(selection, scan):
    owners = [o["mask"] for o in selection["owners"]]
    width = len(owners[0])
    owner_bits = {bits(m): m for m in owners}
    routes = {}
    for r in selection["initial_frontier_routes"]:
        routes[bits(r["source_mask"])] = bits(r["owner_mask"])
    targets = defaultdict(set)
    zero = set()
    kinds = defaultdict(int)
    for row in scan["owners"]:
        if not row["scan_complete"] or row["error"] is not None:
            raise SystemExit(f"scan incomplete for owner {row['owner']}")
        for group in row["successor_groups"]:
            t = bits(group["target_sector"])
            targets[bits(row["owner"])].add(t)
            if group["exact_zero_sector"]:
                zero.add(t)
            kinds[group["transition"]] += 1
    if scan.get("requested_max_numerator_rank") is not None or not scan.get("positive_powers_unbounded"):
        raise SystemExit("scan must be rank-unbounded with unbounded positive powers")
    return owners, width, owner_bits, routes, targets, zero, kinds


def closure(owners, owner_bits, routes, targets, zero):
    """reach[o] = owners (bits) whose Apply node is reachable from Apply(o)."""
    sector_memo = {}

    def sector_succ(s):
        if s in sector_memo:
            return sector_memo[s]
        if s in zero:
            out = ((), ())
        elif s in owner_bits:
            out = ((s,), ())
        elif s in routes:
            root = routes[s]
            out = ((root,), tuple(proper_submasks(root)))
        else:
            out = ((), ())
        sector_memo[s] = out
        return out

    reach = {}
    missing = defaultdict(set)
    for o in owners:
        ob = bits(o)
        seen_apply = {ob}
        seen_sector = set()
        stack = [("A", ob)]
        while stack:
            kind, v = stack.pop()
            nxt = targets[v] if kind == "A" else [v]
            for s in nxt:
                if s in seen_sector:
                    continue
                seen_sector.add(s)
                applies, subs = sector_succ(s)
                if not applies and not subs and s not in zero and s not in owner_bits:
                    missing[ob].add(s)
                for a in applies:
                    if a not in seen_apply:
                        seen_apply.add(a)
                        stack.append(("A", a))
                for m in subs:
                    if m not in seen_sector:
                        stack.append(("S", m))
        reach[ob] = (seen_apply, seen_sector)
    return reach, missing


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--selection", required=True)
    p.add_argument("--scan", required=True)
    p.add_argument("--guards", required=True)
    p.add_argument("--observed", action="append", default=[])
    p.add_argument("--one-hop", action="append", default=[],
                   help="hop.tsv from one_hop.py/cp5hop; replaces the listed owners' Apply successors")
    p.add_argument("--one-hop-receipt", action="append", default=[],
                   help="receipt.json of each --one-hop run; refused if an initial record has frontiers/errors")
    p.add_argument("--output", required=True)
    args = p.parse_args(argv)
    selection = json.load(open(args.selection))
    scan = json.load(open(args.scan))
    owners, width, owner_bits, routes, targets, zero, kinds = build(selection, scan)
    refined = {}
    for path in args.one_hop_receipt:
        receipt = json.load(open(path))
        if receipt.get("initial_with_frontiers_or_errors") or receipt.get("initial_records") != receipt.get("query_count"):
            raise SystemExit(f"one-hop receipt {path} is incomplete or has frontiers/errors")
    for path in args.one_hop:
        for line in open(path).read().splitlines()[1:]:
            f = line.split("\t")
            if f[1] != "Apply" or f[3] != "1":
                raise SystemExit(f"one-hop source must be an inspected Apply domain: {line}")
            refined.setdefault(bits(f[2]), set()).add(bits(f[5]))
    for o, t in refined.items():
        targets[o] = t
    guards = [g for g in args.guards.split(",") if g]
    unknown = [g for g in guards if g not in owners]
    if unknown:
        raise SystemExit(f"guards are not owners: {unknown}")
    gbits = {bits(g) for g in guards}
    reach, missing = closure(owners, owner_bits, routes, targets, zero)
    rows = []
    l_static = []
    for i, o in enumerate(owners):
        applies, sectors = reach[bits(o)]
        reached_guards = sorted(mask_str(g, width) for g in applies & gbits if g != bits(o))
        is_guard = bits(o) in gbits
        in_l = not is_guard and not reached_guards
        if in_l:
            l_static.append(o)
        rows.append({"index": i, "owner": o, "t": o.count("1"), "guard": is_guard,
                     "reachable_owners": len(applies), "reachable_sectors": len(sectors),
                     "reached_guards": reached_guards, "in_L_static": in_l,
                     "missing_route_sectors": len(missing[bits(o)]), "refined_by_one_hop": bits(o) in refined})
    validation = []
    violations = 0
    for path in args.observed:
        checked = 0
        bad = []
        for line in open(path).read().splitlines()[1:]:
            s, t, c = map(int, line.split())
            if s >= 100000:
                continue  # Route/non-owner sources: covered by the owner closure below
            src = bits(owners[s])
            applies, sectors = reach[src]
            if t < 100000:
                ok = bits(owners[t]) in applies
            elif t < 200000:
                ok = False  # an Apply domain on a non-owner mask is outside the model
            else:
                m = t - 200000
                ok = m in sectors or m in applies
            checked += 1
            if not ok:
                bad.append({"source": owners[s], "target_key": t, "count": c})
        violations += len(bad)
        # owner-level closure of the observed graph must be inside the static closure
        adj = defaultdict(set)
        for line in open(path).read().splitlines()[1:]:
            s, t, c = map(int, line.split())
            adj[s].add(t)
        closure_bad = []
        for i, o in enumerate(owners):
            seen = {i}
            stack = [i]
            while stack:
                v = stack.pop()
                for u in adj[v]:
                    if u not in seen:
                        seen.add(u)
                        stack.append(u)
            obs_owners = {bits(owners[x]) for x in seen if x < 100000}
            extra = obs_owners - reach[bits(o)][0]
            if extra:
                closure_bad.append({"owner": o, "observed_not_static": sorted(mask_str(x, width) for x in extra)})
        violations += len(closure_bad)
        validation.append({"observed": path, "owner_source_edges_checked": checked, "edge_violations": bad[:50],
                           "edge_violation_count": len(bad), "closure_violations": closure_bad})
    doc = {"schema": "rustred.static-owner-reach.v1",
           "model": "scan target sectors (rank None, A unbounded) + owner/route/zero routing + all proper sub-masks of route roots",
           "inputs": {"selection": args.selection, "scan": args.scan},
           "guards": guards, "owner_count": len(owners), "route_count": len(routes),
           "one_hop": args.one_hop, "refined_owners": sorted(mask_str(o, width) for o in refined),
           "scan_group_kinds": dict(kinds), "zero_target_sectors": len(zero),
           "L_static": l_static, "L_static_count": len(l_static),
           "owners": rows, "validation": validation, "violations": violations}
    json.dump(doc, open(args.output, "w"), indent=1, sort_keys=True)
    print(json.dumps({"L_static_count": len(l_static), "violations": violations,
                      "guard_reaching": sum(1 for r in rows if r["reached_guards"])}, sort_keys=True))
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
