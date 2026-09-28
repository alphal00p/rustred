#!/usr/bin/env python
"""Work summary of one walk result.json (lane g2prod; read-only, streamed).

usage: g2prod_stats.py RUN_DIR [--out RUN_DIR/g2stats.json]

Native calls and record seconds per phase and per Apply owner (a G2' full
cover makes no native call; a G2' residual record and an initial-D-band
partial each make one), G2' record kinds, anchor links by anchor kind, query
and covered points, residual D levels, and the native operation / term visit
sums of Apply records. Record seconds of G2' records include their plan time.
"""
import importlib.util
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("audit", ROOT / "examples/python/audit_owner_domain_walk.py")
AUDIT = importlib.util.module_from_spec(spec)
spec.loader.exec_module(AUDIT)


def summarize(result):
    kinds = Counter()
    phase = defaultdict(lambda: [0, 0.0])
    owner = defaultdict(lambda: [0, 0.0])
    g2 = Counter()
    anchor_kinds = Counter()
    apply_ops = Counter()
    levels = Counter()
    for item in AUDIT.stream_walk(result):
        if item[0] != "domain":
            continue
        row = item[1]
        kind = row.get("record_kind") or "native_inspection"
        kinds[kind] += 1
        if kind == "delegated_not_inspected":
            continue
        seconds = row.get("seconds") or 0.0
        ph = row.get("phase")
        call = True
        block = row.get("g2_residual_anchors")
        if kind == "g2_residual_anchor_inspection" and isinstance(block, dict):
            anchors = block.get("anchors") or []
            g2["records"] += 1
            g2["anchor_links"] += len(anchors)
            g2["query_points"] += block.get("query_points") or 0
            g2["covered_points"] += block.get("covered_points") or 0
            g2["candidates"] += block.get("candidates") or 0
            for anchor in anchors:
                anchor_kinds[anchor.get("kind")] += 1
            if block.get("residual_power_bounds") is None:
                g2["full_cover"] += 1
                g2["full_cover_seconds"] += seconds
                call = False
            else:
                g2["residual"] += 1
                g2["residual_seconds"] += seconds
                band = block.get("residual_d_band") or [0, -1]
                levels["residual_d_levels"] += band[1] - band[0] + 1
        if call:
            phase[ph][0] += 1
        phase[ph][1] += seconds
        if ph == "Apply":
            if call:
                owner[row.get("owner")][0] += 1
            owner[row.get("owner")][1] += seconds
            stats = row.get("stats") or {}
            for key in ("native_operations", "term_visits", "events", "successors"):
                apply_ops[key] += stats.get(key) or 0
    return {
        "record_kinds": dict(kinds),
        "native_calls_by_phase": {k: v[0] for k, v in phase.items()},
        "record_seconds_by_phase": {k: v[1] for k, v in phase.items()},
        "apply_by_owner": {k: {"native_calls": v[0], "record_seconds": v[1]} for k, v in sorted(owner.items())},
        "g2": dict(g2), "g2_anchor_kinds": dict(anchor_kinds), "g2_levels": dict(levels),
        "apply_stat_sums": dict(apply_ops),
        "scope": "record seconds are native wall seconds per record (G2' plan included); full covers make no native call",
    }


def main():
    run = Path(sys.argv[1])
    out = Path(sys.argv[sys.argv.index("--out") + 1]) if "--out" in sys.argv else run / "g2stats.json"
    summary = summarize(run / "result.json")
    out.write_text(json.dumps(summary, indent=1) + "\n")
    print(json.dumps({k: v for k, v in summary.items() if k != "apply_by_owner"}, indent=1))


if __name__ == "__main__":
    main()
