#!/usr/bin/env python
"""Stream one or more owner-domain walk result.json files and report inspector
time per native inspection (Symbolica lane, W0).

Per file: native inspection count and the sum of per-record `seconds` by
phase (the inspector-thread wall seconds of each native inspection), the
slot busy/backpressure/idle sums, coordinator elapsed, and, when a sibling
metrics.json from ab_controls.py exists, the process-tree user+system CPU.

Usage: native_seconds.py RESULT.json [RESULT.json ...] [--json OUT]
"""
import argparse
import importlib.util
import json
from collections import Counter
from pathlib import Path

AUDIT_PATH = Path(__file__).resolve().parents[3] / "examples/python/audit_owner_domain_walk.py"
_SPEC = importlib.util.spec_from_file_location("audit_owner_domain_walk", AUDIT_PATH)
AUDIT = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(AUDIT)


def scan(path):
    natives = Counter()
    seconds = Counter()
    records = 0
    top = {}
    for item in AUDIT.stream_walk(path):
        if item[0] == "domain":
            records += 1
            rec = item[1]
            if rec.get("record_kind") == "native_inspection":
                phase = rec.get("phase")
                natives[phase] += 1
                seconds[phase] += float(rec.get("seconds") or 0.0)
        elif item[0] == "top" and item[1] in ("parallel", "elapsed_seconds", "traversal_seconds",
                                               "prepared_seconds", "native_processed_nodes"):
            top[item[1]] = item[2]
    par = top.get("parallel") or {}
    duty = par.get("coordinator_duty") or {}
    total_native = sum(natives.values())
    total_seconds = sum(seconds.values())
    out = {
        "path": str(path),
        "records": records,
        "native_inspections": dict(natives),
        "native_record_seconds": {k: round(v, 3) for k, v in seconds.items()},
        "native_record_seconds_total": round(total_seconds, 3),
        "ms_per_native": round(1000 * total_seconds / total_native, 4) if total_native else None,
        "slot_busy_seconds": round(sum(par.get("slot_busy_seconds") or []), 3),
        "slot_backpressure_seconds": round(sum(par.get("slot_backpressure_seconds") or []), 3),
        "slot_idle_seconds": round(sum(par.get("slot_idle_seconds") or []), 3),
        "coordinator_elapsed_seconds": duty.get("coordinator_elapsed_seconds"),
        "traversal_seconds": top.get("traversal_seconds"),
        "elapsed_seconds": top.get("elapsed_seconds"),
        "native_processed_nodes": top.get("native_processed_nodes"),
    }
    metrics = Path(path).with_name("metrics.json")
    if metrics.exists():
        m = json.load(open(metrics))
        cpu = (m.get("rusage_user_seconds") or 0) + (m.get("rusage_system_seconds") or 0)
        out["process_cpu_seconds"] = round(cpu, 3)
        out["whole_command_seconds"] = m.get("whole_command_seconds")
        out["peak_rss_bytes"] = m.get("peak_rss_bytes")
        out["cpu_set_busy_share"] = m.get("cpu_set_busy_share")
        if total_native:
            out["process_cpu_ms_per_native"] = round(1000 * cpu / total_native, 4)
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("results", nargs="+", type=Path)
    p.add_argument("--json", type=Path)
    args = p.parse_args()
    rows = [scan(path) for path in args.results]
    text = json.dumps(rows, indent=1)
    if args.json:
        args.json.write_text(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
