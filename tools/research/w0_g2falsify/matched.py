#!/usr/bin/env python
"""Work-volume dynamics at matched discovered domains (read-only).

usage: matched.py RUN_DIR... [--grid 200000,400000,...]
Reads each RUN_DIR/pending.json series (t, scheduled, committed, natives,
pending, pending natives; written by pending.py) and prints, at each grid
value of scheduled (discovered) domains, the linearly interpolated pending
domains and native records (completions), plus pending per completion; runs
that never reach a grid value show '-'. Launch criterion (H) of the audit
addendum compares these at matched discovered domains.
"""
import json
import sys
from pathlib import Path


def interp(rows, s, col):
    prev = None
    for row in rows:
        if row[1] is None or row[col] is None:
            continue
        if row[1] >= s:
            if prev is None or row[1] == prev[1]:
                return row[col]
            f = (s - prev[1]) / (row[1] - prev[1])
            return prev[col] + f * (row[col] - prev[col])
        prev = row
    return None


def main():
    args = sys.argv[1:]
    grid = [200_000, 400_000, 600_000, 800_000, 1_000_000, 1_200_000]
    if "--grid" in args:
        i = args.index("--grid")
        grid = [int(x) for x in args[i + 1].split(",")]
        args = args[:i] + args[i + 2:]
    print("| Run | " + " | ".join(f"@{g:,}: pending / completions (ratio)" for g in grid) + " |")
    print("|---|" + "---|" * len(grid))
    for run in args:
        p = json.load(open(Path(run) / "pending.json"))
        rows = p["series_t_sched_committed_natives_pending_pnative"]
        cells = []
        for g in grid:
            pend, nat = interp(rows, g, 4), interp(rows, g, 3)
            cells.append("-" if pend is None or not nat else f"{pend:,.0f} / {nat:,.0f} ({pend / nat:.3f})")
        print(f"| {run} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main()
