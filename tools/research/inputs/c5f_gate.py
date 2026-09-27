#!/usr/bin/env python3
"""W1.3 witness gate on C-5F A/B arms (run_four.py --family five-finite).

For each arm directory (ROOT/<label>/five-finite): the audit verdict
(audit.json), run metrics (metrics.json), the census phase split
(census/summary.json) and phase-to-phase edge totals (census/transitions.tsv),
the inspector slot busy seconds and coordinator buckets from the tail of
result.json, and the foreign load of the arm's CPU set (<label>.load.jsonl
summary row). Pairs are given as ORIG:I2 labels; for each pair the gate metric
(Route->Route edges per Route native, >= 20 % fewer with both audits PASS)
and the total-work ratios (natives, Apply natives, discovered domains, edges,
own CPU) are reported.

Usage: c5f_gate.py ROOT ORIG:I2 [ORIG:I2 ...] [--output OUT.json]
"""
import argparse
import csv
import json
from pathlib import Path
import re


def arm(root, label):
    d = root / label / "five-finite"
    audit = json.loads((d / "audit.json").read_text())
    metrics = json.loads((d / "metrics.json").read_text())
    summary = json.loads((d / "census/summary.json").read_text())
    edges = {}
    for row in csv.DictReader(open(d / "census/transitions.tsv"), delimiter="\t"):
        key = f"{row['source_phase']}->{row['target_phase']}"
        edges[key] = edges.get(key, 0) + int(row["edges"])
    size = (d / "result.json").stat().st_size
    with open(d / "result.json", "rb") as f:
        f.seek(max(0, size - 40000))
        tail = f.read().decode("utf-8", "replace")
    busy = re.search(r'"slot_busy_seconds": \[([^\]]*)\]', tail)
    coord = {}
    for key in ("coordinator_elapsed_seconds", "ordered_commit_seconds", "publication_seconds", "wait_seconds"):
        m = re.search(r'"%s": ([0-9.]+)' % key, tail)
        coord[key] = float(m.group(1)) if m else None
    load = None
    load_path = root / f"{label}.load.jsonl"
    if load_path.exists():
        rows = [json.loads(line) for line in open(load_path) if line.strip()]
        load = next((r for r in rows if r.get("summary")), None)
    natives = int(metrics["completed_nodes"])
    return {
        "label": label, "audit": audit.get("audit"), "violations": len(audit.get("violations") or []),
        "all_local_obligations_discharged": audit.get("all_local_obligations_discharged"),
        "exit_code": metrics["exit_code"], "drained": metrics["exit_code"] == 0 and metrics.get("queued_nodes") == "0",
        "cpus": metrics["cpus"], "manifest_sha256": metrics["manifest_sha256"], "binary_sha256": metrics["binary_sha256"],
        "policy": next((a for a in json.loads((d / "command.json").read_text())
                        if a in ("ready", "ordered")), None),
        "natives": natives, "discovered": int(metrics["scheduled_nodes"]),
        "apply_natives": summary["apply_inspected"], "route_natives": summary["route_inspected"],
        "apply_domains": summary["apply_domains"], "route_domains": summary["route_domains"],
        "edges": summary["edges"], "edges_by_phase": edges,
        "route_route_per_route_native": edges.get("Route->Route", 0) / max(1, summary["route_inspected"]),
        "edges_per_native": summary["edges"] / max(1, natives),
        "aliases": audit.get("aliases"), "containment_checks": int(metrics["containment_checks"]),
        "traversal_seconds": float(metrics["traversal_seconds"]), "own_cpu_seconds": metrics["own_cpu_seconds"],
        "inspector_slot_busy_seconds": sum(float(x) for x in busy.group(1).split(",") if x.strip()) if busy else None,
        "coordinator": coord, "peak_rss_bytes": metrics["peak_rss_bytes"],
        "foreign_share": load.get("foreign_share") if load else metrics.get("foreign_share_of_cpuset"),
    }


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("root", type=Path)
    p.add_argument("pairs", nargs="+")
    p.add_argument("--output", type=Path)
    args = p.parse_args(argv)
    out = {"pairs": []}
    for spec in args.pairs:
        o_label, _, i_label = spec.partition(":")
        o, i = arm(args.root, o_label), arm(args.root, i_label)
        ratio = {k: i[k] / o[k] for k in ("natives", "apply_natives", "route_natives", "discovered", "apply_domains",
                                         "edges", "own_cpu_seconds", "inspector_slot_busy_seconds",
                                         "containment_checks") if o.get(k) and i.get(k) is not None}
        change = i["route_route_per_route_native"] / o["route_route_per_route_native"] - 1.0
        gate = change <= -0.20 and o["audit"] == "PASS" and i["audit"] == "PASS"
        out["pairs"].append({"orig": o, "i2": i, "route_route_per_route_native_change": change,
                             "w13_gate_as_worded": "PASS" if gate else "FAIL", "i2_over_orig": ratio})
    text = json.dumps(out, indent=1, sort_keys=True)
    if args.output:
        args.output.write_text(text + "\n")
    for pair in out["pairs"]:
        o, i, r = pair["orig"], pair["i2"], pair["i2_over_orig"]
        print(f"{o['label']} vs {i['label']} ({o['policy']}; CPUs {o['cpus']} / {i['cpus']}; foreign "
              f"{o['foreign_share']:.3f} / {i['foreign_share']:.3f}); audits {o['audit']} / {i['audit']}; drained "
              f"{o['drained']} / {i['drained']}")
        print(f"  R->R per Route native {o['route_route_per_route_native']:.3f} -> {i['route_route_per_route_native']:.3f} "
              f"({100 * pair['route_route_per_route_native_change']:+.1f} %): gate as worded {pair['w13_gate_as_worded']}")
        print(f"  natives {o['natives']:,} -> {i['natives']:,} (x{r['natives']:.3f}); Apply natives {o['apply_natives']:,} -> "
              f"{i['apply_natives']:,} (x{r['apply_natives']:.3f}); Route natives {o['route_natives']:,} -> {i['route_natives']:,} "
              f"(x{r['route_natives']:.3f})")
        print(f"  discovered {o['discovered']:,} -> {i['discovered']:,} (x{r['discovered']:.3f}); edges {o['edges']:,} -> "
              f"{i['edges']:,} (x{r['edges']:.3f}); own CPU {o['own_cpu_seconds']} -> {i['own_cpu_seconds']} s "
              f"(x{r['own_cpu_seconds']:.3f}); inspector busy {o['inspector_slot_busy_seconds']:.0f} -> "
              f"{i['inspector_slot_busy_seconds']:.0f} s; traversal {o['traversal_seconds']:.0f} -> {i['traversal_seconds']:.0f} s")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
