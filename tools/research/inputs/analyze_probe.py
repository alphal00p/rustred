#!/usr/bin/env python3
"""Summarise a probe.py run for the W0.6 gate.

Reads DIR/receipt.json, DIR/timeseries.jsonl, DIR/census/{envelope,escapes}.tsv
and reports: frontiers (count, first heartbeat), trajectory rows at 10/30/45
minutes and at the end, and for the guard-sensitive owners the Apply domains
admitted in their masks, split into those inside the owner's helper box of
the probed input set and those outside it (escapes by rank / by A; any
unbounded-A domain). Optionally compares with a reference heartbeat series
(e.g. the v2 campaign) at matched completed nodes.

Usage: analyze_probe.py DIR --guards MASKS [--reference SERIES.json] [--output OUT.json]
"""
import argparse
import csv
import json
from pathlib import Path


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("dir", type=Path)
    p.add_argument("--guards", required=True)
    p.add_argument("--reference", type=Path)
    p.add_argument("--output", type=Path)
    args = p.parse_args(argv)
    d = args.dir
    receipt = json.loads((d / "receipt.json").read_text())
    series = [json.loads(line) for line in open(d / "timeseries.jsonl") if line.strip()]
    guards = set(args.guards.split(","))
    out = {"receipt": {k: receipt.get(k) for k in ("binary_sha256", "queries_sha256", "cpus", "workers", "policy",
                                                    "exit_code", "wall_seconds", "stop_requested_at_seconds",
                                                    "first_frontier_heartbeat_seconds", "result_frontiers",
                                                    "result_completed_nodes", "result_scheduled_nodes",
                                                    "result_queued_nodes", "result_max_scheduled_finite_rank",
                                                    "checkpoint_generation")}}
    marks = {}
    for minutes in (10, 30, 45):
        rows = [r for r in series if r.get("t") is not None and r["t"] <= minutes * 60]
        if rows:
            marks[f"{minutes}min"] = rows[-1]
    if series:
        marks["last"] = series[-1]
    out["trajectory"] = marks
    out["max_frontiers_seen"] = max((r.get("frontiers") or 0) for r in series) if series else None
    env = {r["mask"]: r for r in csv.DictReader(open(d / "census/envelope.tsv"), delimiter="\t")}
    esc_path = d / "census/escapes.tsv"
    esc = {r["mask"]: r for r in csv.DictReader(open(esc_path), delimiter="\t")} if esc_path.exists() else {}
    g_rows = {}
    for g in sorted(guards):
        e = env.get(g, {})
        x = esc.get(g, {})
        g_rows[g] = {"apply_domains": int(e.get("apply_domains", 0)), "inspected": int(e.get("inspected", 0)),
                     "unbounded_A": int(e.get("unbounded_A", 0)), "max_finite_A": e.get("max_finite_A"),
                     "max_finite_rank": e.get("max_finite_rank"),
                     "helper_rank": x.get("helper_rank"), "helper_A": x.get("helper_A"),
                     "outside_helper": int(x.get("escapes", 0)), "outside_by_rank": int(x.get("escapes_by_rank", 0)),
                     "outside_by_A": int(x.get("escapes_by_A", 0)),
                     "outside_inspected": int(x.get("inspected_escapes", 0))}
    out["guard_owners"] = g_rows
    out["guard_totals"] = {k: sum(v[k] for v in g_rows.values())
                           for k in ("apply_domains", "unbounded_A", "outside_helper", "outside_by_rank",
                                     "outside_by_A", "outside_inspected")}
    out["unbounded_A_domains_by_owner"] = {m: int(r["unbounded_A"]) for m, r in env.items() if int(r["unbounded_A"])}
    out["all_owners_outside_helper"] = {k: sum(int(r[k]) for r in esc.values())
                                        for k in ("apply_domains", "escapes", "escapes_by_rank", "escapes_by_A")} if esc else None
    if args.reference and series:
        ref = json.loads(args.reference.read_text())
        rows = []
        for r in series[::max(1, len(series) // 8)] + [series[-1]]:
            c = r.get("completed_nodes") or 0
            match = next((x for x in ref if (x.get("completed_nodes") or 0) >= c), None)
            if match:
                rows.append({"completed": c, "probe_scheduled": r["scheduled_nodes"], "ref_scheduled": match["scheduled_nodes"],
                             "probe_queued": r["queued_nodes"], "ref_queued": match["queued_nodes"],
                             "probe_roots": r.get("roots_closed"), "ref_roots": match.get("roots"),
                             "probe_rank": r.get("max_scheduled_finite_rank"),
                             "ref_rank": match.get("max_scheduled_finite_rank")})
        out["matched_completed_vs_reference"] = rows
    text = json.dumps(out, indent=1, sort_keys=True)
    if args.output:
        args.output.write_text(text + "\n")
    print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
