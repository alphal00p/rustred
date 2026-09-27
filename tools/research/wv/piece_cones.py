#!/usr/bin/env python
"""Piece-level cone analysis of one drained (or stopped) four-loop walk (W0.11 D1(b)).

Reads RUN/result.json (records, query -> domain map) and the CP5 dependency
edges RUN/checkpoint/edges-*.bin (u32 source, u32 target: source depends on
target). Stdlib only; read-only.

A physics query certifies at piece granularity iff its root's forward cone
contains no frontier- or error-bearing node and no uninspected node. This is
computed by reverse reachability from the "bad" nodes (O(edges)). For every
piece level (piece index in the variant) it also reports how many physics
roots reach a piece of that level, how many dependency edges land on pieces
of each level (the fast-path collapse shows up here), which piece levels
carry frontiers, and how many roots reach a node with a rank bound above 12
or no rank bound (an escape that a rank-12 piece cannot hold).

Usage: piece_cones.py RUN_DIR   (JSON to stdout)
"""
import json
import re
import struct
import sys
from array import array
from collections import Counter, defaultdict
from pathlib import Path


def edges(run):
    ck = run / "checkpoint"
    latest = json.load(open(ck / "latest.json"))
    src, dst = array("I"), array("I")
    for seg in latest["sections"]["edges"]["segments"]:
        b = (ck / seg["file"]).read_bytes()
        assert b[:4] == b"RRW5" and b[4:8] == b"EDGE"
        payload = array("I")
        payload.frombytes(b[32:])
        assert len(payload) == 2 * seg["count"]
        src.extend(payload[0::2])
        dst.extend(payload[1::2])
    return src, dst


def reverse_reach(n, src, dst, seeds):
    """Nodes from which some seed is reachable (seeds included)."""
    radj = defaultdict(list)
    for s, t in zip(src, dst):
        radj[t].append(s)
    seen = bytearray(n)
    stack = [s for s in seeds]
    for s in stack:
        seen[s] = 1
    while stack:
        v = stack.pop()
        for u in radj.get(v, ()):
            if not seen[u]:
                seen[u] = 1
                stack.append(u)
    return seen


def main():
    run = Path(sys.argv[1])
    r = json.load(open(run / "result.json"))
    if "domains" not in r:
        # A cooperatively stopped (paused) walk keeps its records in the CP5
        # sidecar and has no query map in result.json: use walk-verify-closure.
        print(json.dumps({"run": str(run), "status": r.get("status"), "paused": True,
                          "note": "no domains/inputs in result.json; see the walk-verify-closure report"}))
        return
    doms = r["domains"]
    n = max(d["id"] for d in doms) + 1
    by_id = {d["id"]: d for d in doms}
    src, dst = edges(run)
    n = max(n, (max(dst) + 1) if dst else 0, (max(src) + 1) if src else 0)
    # Query classes and piece levels.
    physics, pieces = {}, defaultdict(list)
    level_of = {}
    for inp in r["inputs"]:
        qid, dom = inp["id"], inp["domain"]
        if "anchor" in qid:
            m = re.match(r"piece(\d+)-(a\w+)-(r\w+)-anchor", qid)
            lvl = f"{m.group(1)}:{m.group(2)}-{m.group(3)}" if m else "helper"
            pieces[lvl].append(dom)
            level_of.setdefault(dom, lvl)
        else:
            physics[qid] = dom
    bad = [i for i, d in by_id.items() if d.get("frontiers") or d.get("error")]
    # A node is unresolved if it has no record yet (pending at a stop), or is a
    # native whose local inspection did not finish. Alias records resolve
    # through their dependency edge to the representative.
    uninspected = [i for i in range(n) if i not in by_id
                   or (by_id[i].get("record_kind") == "native_inspection"
                       and not by_id[i].get("local_inspection_finished", True))]
    frontier_nodes = set(bad)
    reach_bad = reverse_reach(n, src, dst, bad + uninspected)
    reach_frontier = reverse_reach(n, src, dst, bad)
    out = {"run": str(run), "status": r.get("status"), "domains": len(doms), "id_space": n,
           "edges": len(src), "frontier_or_error_nodes": len(bad), "uninspected_nodes": len(uninspected),
           "natives": sum(1 for d in doms if d.get("record_kind") in ("native_inspection", "partial_initial_overlap_inspection")),
           "physics_queries": len(physics), "physics_roots": len(set(physics.values()))}
    cert = [q for q, d in physics.items() if not reach_bad[d]]
    engine_closed = [q for q, d in physics.items() if by_id.get(d, {}).get("descendant_closed")]
    out["physics_certified_piece_level"] = len(cert)
    out["physics_engine_descendant_closed"] = len(engine_closed)
    out["physics_cone_has_frontier"] = sum(1 for d in physics.values() if reach_frontier[d])
    # Root aliasing: which piece level holds each physics root.
    root_level = Counter(level_of.get(d, "own-node") for d in physics.values())
    out["physics_root_held_by"] = dict(root_level)
    # Per piece level: pieces, frontier-bearing pieces, in-edges, physics roots reaching it.
    lv = {}
    indeg = Counter(dst)
    for lvl, ds in sorted(pieces.items()):
        dset = set(ds)
        reach = reverse_reach(n, src, dst, list(dset))
        lv[lvl] = {
            "pieces": len(dset),
            "frontier_bearing_pieces": sum(1 for d in dset if d in frontier_nodes),
            "in_edges": sum(indeg[d] for d in dset),
            "physics_roots_reaching": sum(1 for d in set(physics.values()) if reach[d]),
            "physics_roots_reaching_frontier_bearing_piece": None,
        }
        fb = [d for d in dset if d in frontier_nodes]
        if fb:
            rf = reverse_reach(n, src, dst, fb)
            lv[lvl]["physics_roots_reaching_frontier_bearing_piece"] = sum(1 for d in set(physics.values()) if rf[d])
        else:
            lv[lvl]["physics_roots_reaching_frontier_bearing_piece"] = 0
    out["piece_levels"] = lv
    # Rank escapes: nodes with no rank bound or a rank bound above 12.
    esc = [i for i, d in by_id.items() if d.get("rank") is None or d["rank"] > 12]
    reach_esc = reverse_reach(n, src, dst, esc)
    out["nodes_rank_above_12_or_unbounded"] = len(esc)
    out["physics_roots_reaching_rank_above_12_or_unbounded"] = sum(1 for d in set(physics.values()) if reach_esc[d])
    ranks = Counter("inf" if d.get("rank") is None else d["rank"] for d in doms if d.get("record_kind") == "native_inspection")
    out["native_rank_bound_histogram"] = {str(k): v for k, v in sorted(ranks.items(), key=lambda kv: (kv[0] == "inf", kv[0] if kv[0] != "inf" else 0))}
    out["native_seconds"] = sum(d.get("seconds") or 0 for d in doms)
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
