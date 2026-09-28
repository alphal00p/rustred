#!/usr/bin/env python
"""Work-volume series of one walk from its events.jsonl heartbeats (read-only).

usage: pending.py RUN_DIR [--out RUN_DIR/pending.json]

Per heartbeat: elapsed seconds, scheduled (discovered) domains, committed
domains, native inspections (completed_nodes), pending domains (queued_nodes =
scheduled - committed) and pending native publications. Summary: peak pending
(and the natives at the peak), final scheduled domains and natives, discovered
domains per native, and the pending growth per completion: the least-squares
slope of pending vs natives up to the peak, plus the plain ratio
peak_pending / natives_at_peak. Series are downsampled to <= 400 points.
"""
import json
import sys
from pathlib import Path


def series(run):
    rows = []
    with open(Path(run) / "events.jsonl") as f:
        for line in f:
            if '"heartbeat"' not in line:
                continue
            e = json.loads(line)
            if e.get("event") != "heartbeat":
                continue
            p = e.get("progress") or {}
            d = p.get("delegation") or {}
            if p.get("scheduled_nodes") is None:
                continue
            rows.append((e.get("elapsed_seconds"), p.get("scheduled_nodes"), p.get("committed_domains"),
                         p.get("completed_nodes"), p.get("queued_nodes"), d.get("pending_native_publications")))
    return rows


def slope(xs, ys):
    n = len(xs)
    if n < 2:
        return None
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    if sxx == 0:
        return None
    return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx


def summarize(rows):
    if not rows:
        return {"heartbeats": 0}
    peak = max(range(len(rows)), key=lambda i: rows[i][4])
    t, sched, comm, nat, pend, pnat = rows[peak]
    up = rows[: peak + 1]
    final = rows[-1]
    step = max(1, len(rows) // 400)
    return {
        "heartbeats": len(rows),
        "peak_pending_domains": pend, "peak_at_seconds": t, "natives_at_peak": nat, "scheduled_at_peak": sched,
        "peak_pending_native_publications": max((r[5] or 0) for r in rows),
        "final_scheduled_domains": final[1], "final_committed_domains": final[2], "final_natives": final[3],
        "final_pending_domains": final[4], "final_seconds": final[0],
        "discovered_domains_per_native": final[1] / final[3] if final[3] else None,
        "pending_growth_per_completion_slope_to_peak": slope([r[3] for r in up], [r[4] for r in up]),
        "peak_pending_per_native_at_peak": pend / nat if nat else None,
        "series_t_sched_committed_natives_pending_pnative": rows[::step] + ([rows[-1]] if len(rows) % step else []),
    }


def main():
    run = Path(sys.argv[1])
    out = Path(sys.argv[sys.argv.index("--out") + 1]) if "--out" in sys.argv else run / "pending.json"
    summary = summarize(series(run))
    out.write_text(json.dumps(summary, indent=1) + "\n")
    print(json.dumps({k: v for k, v in summary.items() if not k.startswith("series")}, indent=1))


if __name__ == "__main__":
    main()
