#!/usr/bin/env python
"""Offline piece-level certification (W0.11 D1(b)) composed from real walks.

The legacy engine cannot keep nested helper pieces apart (initial_orthants.rs
keeps one fast-path orthant per owner, the largest rank; a newer containing
admission retires the older candidate; initial queries alias into the live
container), so input-only piece queries collapse onto the largest piece
(measured: `shells`/`aslabs` variants). This script evaluates the idealized
piece-aware rule offline instead, from drained helper-first walks that each
have ONE helper per owner at a given rank level:

  levels r_1 < r_2 < ... (runs with helpers {R <= r_k}, A as in the base), and
  a top run with unbounded helpers (the frontier fixture shape).

Piece S(o, r_k) is the helper of owner o in run k; T(o) the unbounded helper of
owner o in the top run. The piece graph has an edge S(o, r_k) -> S(o', r_k)
when the helper of o reaches the helper of o' in run k through nodes of rank
<= r_k, and S(o, r_k) -> S(o', r_j) for every escape node (rank bound > r_k or
None) of owner o' first reached that way, with r_j the smallest level >= the
escape's rank bound (T(o') if none). Attribution uses the escape node's own
rank bound, which for an aliased container can exceed the successor's rank
(conservative: it can only move a successor to a larger piece). T nodes carry
the top run's closure: T(o) is bad iff the top run's helper of o is not
descendant-closed. A shell piece is bad iff its own run left a frontier, error
or pending node in its non-escape cone. A physics query certifies iff its
root piece S(o, r_1) cannot reach a bad piece.

Usage: piece_graph.py --level 12=RUN [--level 14=RUN ...] --top RUN   (JSON out)
"""
import argparse
import json
from collections import defaultdict
from pathlib import Path

import importlib.util
_spec = importlib.util.spec_from_file_location("pc", Path(__file__).with_name("piece_cones.py"))
pc = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pc)


def load(run):
    run = Path(run)
    r = json.load(open(run / "result.json"))
    by_id = {d["id"]: d for d in r["domains"]}
    src, dst = pc.edges(run)
    adj = defaultdict(list)
    for s, t in zip(src, dst):
        adj[s].append(t)
    helpers, physics = {}, {}
    for inp in r["inputs"]:
        o = inp["id"].rsplit("-", 1)[1]
        if "anchor" in inp["id"]:
            helpers.setdefault(o, inp["domain"])
        else:
            physics[inp["id"]] = (o, inp["domain"])
    return {"run": str(run), "by_id": by_id, "adj": adj, "helpers": helpers, "physics": physics,
            "status": r.get("status"), "frontiers": r.get("frontiers")}


def bad_node(d):
    if d is None:
        return True  # no record: pending at a stop
    if d.get("frontiers") or d.get("error"):
        return True
    return d.get("record_kind") == "native_inspection" and not d.get("local_inspection_finished", True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--level", action="append", required=True)
    p.add_argument("--top", required=True)
    a = p.parse_args()
    levels = sorted((int(x.split("=")[0]), load(x.split("=")[1])) for x in a.level)
    top = load(a.top)
    ranks = [r for r, _ in levels]

    def piece_for(rank):
        if rank is None:
            return "T"
        for r in ranks:
            if rank <= r:
                return r
        return "T"

    # Piece graph.
    succ = defaultdict(set)
    bad = set()
    info = {}
    inline_nodes = {}
    for r, run in levels:
        by_id, adj = run["by_id"], run["adj"]
        helper_node = {v: o for o, v in run["helpers"].items()}
        for o, h in run["helpers"].items():
            node = (o, r)
            seen = {h}
            stack = [h]
            esc = 0
            while stack:
                v = stack.pop()
                d = by_id.get(v)
                if bad_node(d):
                    bad.add(node)
                for t in adj.get(v, ()):
                    if t in seen:
                        continue
                    seen.add(t)
                    dt = by_id.get(t)
                    if t in helper_node and helper_node[t] != o:
                        succ[node].add((helper_node[t], r))
                        continue
                    rank = None if dt is None else dt.get("rank")
                    if dt is not None and (rank is None or rank > r):
                        esc += 1
                        owner = dt["owner"]
                        tgt = piece_for(rank)
                        succ[node].add((owner, tgt))
                        continue
                    stack.append(t)
            info[node] = esc
            nat = [by_id[v] for v in seen if v != h and v not in helper_node and by_id.get(v)
                   and by_id[v].get("record_kind") == "native_inspection"
                   and not ((by_id[v].get("rank") is None) or by_id[v]["rank"] > r)]
            inline_nodes[node] = (len(nat), sum(d.get("seconds") or 0 for d in nat))
    for o, h in top["helpers"].items():
        d = top["by_id"].get(h)
        if not (d and d.get("descendant_closed")):
            bad.add((o, "T"))
    # Reverse reachability to bad pieces.
    rev = defaultdict(set)
    nodes = set(succ) | {t for s in succ.values() for t in s} | bad
    for s, ts in succ.items():
        for t in ts:
            rev[t].add(s)
    reach_bad = set(bad)
    stack = list(bad)
    while stack:
        v = stack.pop()
        for u in rev.get(v, ()):
            if u not in reach_bad:
                reach_bad.add(u)
                stack.append(u)
    reach_top = set(n for n in nodes if n[1] == "T")
    stack = list(reach_top)
    while stack:
        v = stack.pop()
        for u in rev.get(v, ()):
            if u not in reach_top:
                reach_top.add(u)
                stack.append(u)
    r1, run1 = levels[0]
    phys = run1["physics"]
    cert = [q for q, (o, _) in phys.items() if (o, r1) not in reach_bad]
    touches_top = [q for q, (o, _) in phys.items() if (o, r1) in reach_top]
    # Work of the piece model: every piece reachable from a physics root is one
    # native inspection of the whole piece region (its seconds are the helper
    # record's seconds in its own run); non-escape non-helper nodes traversed
    # inside a piece's cone are inspected as in that run.
    reach = set()
    stack = [(o, r1) for (o, _) in phys.values()]
    reach.update(stack)
    while stack:
        v = stack.pop()
        for t in succ.get(v, ()):
            if t not in reach:
                reach.add(t)
                stack.append(t)
    sec = 0.0
    inline = 0
    inline_sec = 0.0
    run_of = dict(levels)
    for (o, lvl) in reach:
        if lvl == "T":
            d = top["by_id"].get(top["helpers"].get(o))
            sec += (d or {}).get("seconds") or 0
        else:
            run = run_of[lvl]
            d = run["by_id"].get(run["helpers"][o])
            sec += (d or {}).get("seconds") or 0
    for (o, lvl), cnt in inline_nodes.items():
        if (o, lvl) in reach:
            inline += cnt[0]
            inline_sec += cnt[1]
    base_natives = sum(1 for d in run1["by_id"].values() if d.get("record_kind") == "native_inspection")
    base_sec = sum((d.get("seconds") or 0) for d in run1["by_id"].values())
    work = {"pieces_reached": len(reach),
            "pieces_reached_by_level": {str(l): sum(1 for n in reach if n[1] == l) for l in ranks + ["T"]},
            "piece_native_seconds": sec,
            "inline_non_escape_natives": inline, "inline_non_escape_native_seconds": inline_sec,
            "level1_run_natives": base_natives, "level1_run_native_seconds": base_sec,
            "note": "piece seconds are single whole-region inspections measured in each run; "
                    "inline nodes are counted once per piece that traverses them (upper bound)"}
    by_level = {str(r): {"pieces": sum(1 for n in nodes if n[1] == r), "bad": sum(1 for n in bad if n[1] == r),
                         "pieces_with_escapes": sum(1 for n, e in info.items() if n[1] == r and e > 0)}
                for r in ranks}
    by_level["T"] = {"pieces": sum(1 for n in nodes if n[1] == "T"), "bad": sum(1 for n in bad if n[1] == "T")}
    out = {"levels": ranks, "runs": {str(r): run["run"] for r, run in levels}, "top": top["run"],
           "physics_queries": len(phys), "physics_certified_piece_level": len(cert),
           "physics_cone_reaches_top_piece": len(touches_top),
           "top_run_helpers_closed": sum(1 for o, h in top["helpers"].items()
                                         if top["by_id"].get(h, {}).get("descendant_closed")),
           "top_run_helpers": len(top["helpers"]),
           "piece_graph_edges": sum(len(v) for v in succ.values()), "by_level": by_level,
           "work": work,
           "attribution": "escape -> smallest level >= escape rank bound (conservative for aliased containers)"}
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
