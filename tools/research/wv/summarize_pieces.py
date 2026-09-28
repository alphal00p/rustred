#!/usr/bin/env python
"""Table of a pieces batch: per run the native counts, wall, frontiers, drain,
physics certification (engine closure, Python audit, walk-verify-closure,
piece-cone analysis), per-level in-edges and foreign load.

Usage: summarize_pieces.py BATCH_DIR [--json OUT.json]
"""
import json
import sys
from pathlib import Path


def load(p):
    try:
        return json.load(open(p))
    except (OSError, ValueError):
        return None


def main():
    d = Path(sys.argv[1])
    rows = []
    for m in sorted(d.glob("*/metrics.json")):
        name = m.parent.name
        met = load(m) or {}
        aud = load(d / f"{name}.audit.json") or {}
        ver = load(d / f"{name}.verify.json") or {}
        con = load(d / f"{name}.cones.json") or {}
        acert = (aud.get("certification") or {}).get("queries", {}).get("physics", {})
        vcert = (ver.get("certification") or {}).get("classes", {}).get("physics", {})
        levels = con.get("piece_levels") or {}
        row = {
            "run": name,
            "exit": met.get("exit_code"),
            "status": met.get("status"),
            "drained": met.get("recursive_worklist_exhausted"),
            "stopped": met.get("cooperative_stop_requested"),
            "natives": met.get("native_processed_nodes"),
            "scheduled": met.get("scheduled_nodes"),
            "pending": met.get("queued_nodes"),
            "frontiers": met.get("frontiers"),
            "max_rank": met.get("max_scheduled_finite_rank"),
            "wall_s": met.get("whole_command_seconds"),
            "traversal_s": met.get("traversal_seconds"),
            "child_cpu_s": met.get("child_cpu_seconds"),
            "peak_rss_gb": round((met.get("peak_rss_bytes") or 0) / 1e9, 3),
            "foreign_load": met.get("foreign_load_share"),
            "physics_total": con.get("physics_queries") or acert.get("total"),
            "physics_engine_closed": con.get("physics_engine_descendant_closed"),
            "physics_certified_cones": con.get("physics_certified_piece_level"),
            "physics_audit_closed": acert.get("closed"),
            "audit_verdict": aud.get("audit"),
            "physics_verify_certified": vcert.get("certified"),
            "physics_verify_independent": vcert.get("independently_verified"),
            "verify_verdict": ver.get("verdict"),
            "roots_reaching_rank_gt12": con.get("physics_roots_reaching_rank_above_12_or_unbounded"),
            "in_edges_by_level": {k: v.get("in_edges") for k, v in levels.items()},
            "frontier_pieces_by_level": {k: v.get("frontier_bearing_pieces") for k, v in levels.items()},
            "root_held_by": con.get("physics_root_held_by"),
        }
        rows.append(row)
    if "--json" in sys.argv:
        json.dump(rows, open(sys.argv[sys.argv.index("--json") + 1], "w"), indent=1)
    cols = ["run", "exit", "drained", "natives", "pending", "frontiers", "max_rank", "wall_s", "traversal_s",
            "foreign_load", "physics_total", "physics_engine_closed", "physics_certified_cones",
            "physics_audit_closed", "audit_verdict", "physics_verify_certified", "verify_verdict",
            "roots_reaching_rank_gt12"]
    print("\t".join(cols))
    for r in rows:
        print("\t".join(str(r.get(c)) for c in cols))
    print()
    for r in rows:
        print(r["run"], "in_edges", r["in_edges_by_level"], "frontier_pieces", r["frontier_pieces_by_level"],
              "roots_held_by", r["root_held_by"])


if __name__ == "__main__":
    main()
