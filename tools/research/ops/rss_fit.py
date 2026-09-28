#!/usr/bin/env python
"""Marginal RSS per discovered domain of a knob_run.py run (handoff 0.1 item 2).

Fits current RSS (VmRSS of the native, `rss_now`) against the discovered
domains (`scheduled_nodes` from the events journal) by least squares over the
second half of the run by wall time, as directive 0.1.2 prescribes for
pilots of >= 40 min, and reports slope (bytes per domain), intercept, R^2,
the domain range and the peak RSS. Also the whole-run average RSS/domain,
which the import or the owner programs dominate (never a gate).

--until-seconds S drops the samples after S (a cooperative stop request at S
is followed by the final save and teardown, whose RSS is not walk growth);
the second half is then taken of [0, S].

usage: rss_fit.py RUN_DIR [RUN_DIR ...] [--from-fraction 0.5] [--until-seconds S]
"""
import argparse
import json
from pathlib import Path


def fit(xs, ys):
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    syy = sum((y - my) ** 2 for y in ys)
    slope = sxy / sxx if sxx else None
    r2 = (sxy * sxy / (sxx * syy)) if sxx and syy else None
    return slope, (my - slope * mx) if slope is not None else None, r2


def main():
    p = argparse.ArgumentParser()
    p.add_argument("runs", nargs="+", type=Path)
    p.add_argument("--from-fraction", type=float, default=0.5)
    p.add_argument("--until-seconds", type=float)
    args = p.parse_args()
    for run in args.runs:
        rows = [json.loads(line) for line in (run / "cpu.jsonl").read_text().splitlines() if line.strip()]
        rows = [r for r in rows if r.get("rss_now") and r.get("scheduled_nodes")
                and (args.until_seconds is None or r["t"] <= args.until_seconds)]
        if len(rows) < 3:
            print(json.dumps({"run": str(run), "error": "too few samples with rss_now and scheduled_nodes"}))
            continue
        end = rows[-1]["t"]
        half = [r for r in rows if r["t"] >= args.from_fraction * end]
        slope, intercept, r2 = fit([r["scheduled_nodes"] for r in half], [r["rss_now"] for r in half])
        metrics = json.loads((run / "metrics.json").read_text()) if (run / "metrics.json").exists() else {}
        out = {"run": str(run), "binary": metrics.get("binary"), "wall_seconds": end,
               "until_seconds": args.until_seconds,
               "window_seconds": [half[0]["t"], half[-1]["t"]], "samples": len(half),
               "domains_window": [half[0]["scheduled_nodes"], half[-1]["scheduled_nodes"]],
               "marginal_rss_bytes_per_domain": slope, "intercept_bytes": intercept, "r2": r2,
               "rss_window_bytes": [half[0]["rss_now"], half[-1]["rss_now"]],
               "peak_rss_bytes": max(r["rss_now"] for r in rows),
               "average_rss_bytes_per_domain_at_end": rows[-1]["rss_now"] / rows[-1]["scheduled_nodes"],
               "foreign_load_fraction": metrics.get("foreign_load_fraction")}
        print(json.dumps(out))


if __name__ == "__main__":
    main()
