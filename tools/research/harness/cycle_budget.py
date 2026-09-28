#!/usr/bin/env python3
"""Per-native cycle budget of a K-thread harness run against K=1 (W0.3 fix round).

  cycle_budget.py [--near-cache-from RUN] [--dram-cycles C] [--xccx-cycles C] [--prep-dram FILLS]
                  K1_RUN[,K1_RUN...] RUN[,RUN...]

(--prep-dram: DRAM fills of one prepare, default 8.8e7 from the socket-0 prepare-only references A/B-ev7 [E];
used only for the informational all-DRAM change line.)

Comma-separated runs are averaged. For each side: native cycles per native are
estimated as cycles x section user CPU / total user CPU (rusage in receipt.json;
[E]: same clock during prepare and natives; the prepare is <= 1% of the user CPU
at K >= 48 and ~24% at K=1). Fill counts are raw per native (prepare included;
see fills_table.py). The budget charges

  * every extra far-DRAM fill (K minus K=1) the full latency bound --dram-cycles
    (default 775 = 250 ns at 3.1 GHz, an UPPER bound for a remote-node fill on the
    same socket; prefetch fills in the `any` event set do not stall at all), and
  * every cross-CCX fill (near + far cache) the uncontended-transfer bound
    --xccx-cycles (default 775),

and reports the residual. It then states what the residual implies if it is
attributed entirely to the cross-CCX fills (mean cycles per transfer), and a
Little's-law restatement: stalled threads L = K x (1 - c1/cK), transfer rate
lambda = x-CCX fills per native x natives / wall, W = L / lambda. That is the
same data seen as a queue, not an independent measurement. Fill counts mix clean
read-shared transfers and contended ones; latencies are bounds, not measured.

--near-cache-from RUN takes the near-cache count per native from another run
(for example a demand-event run of session D when the timed run only recorded
`ls_any_fills_from_sys.far_cache`); it is labelled [E] in the output.
"""
import json
import os
import sys


def perfstat(run):
    out = {}
    for line in open(os.path.join(run, "perfstat.csv")):
        p = line.strip().split(",")
        if len(p) < 3 or not p[0] or p[0].startswith("#"):
            continue
        try:
            out[p[2].replace(":u", "")] = float(p[0])
        except ValueError:
            pass
    return out


def side(runs):
    acc = {"cyc": 0.0, "fdram": 0.0, "ndram": 0.0, "fcache": 0.0, "ncache": None, "wall": 0.0, "ghz": 0.0,
           "threads": 0, "prep_per_native": 0.0}
    for run in runs:
        rec = json.load(open(os.path.join(run, "receipt.json")))
        sec = rec["section"]
        n = sec["natives"]
        ut = sec["rusage_after"]["user_seconds"]
        us = ut - sec["rusage_before"]["user_seconds"]
        ps = perfstat(run)
        ev = "dmnd" if "ls_dmnd_fills_from_sys.near_cache" in ps else "any"
        acc["cyc"] += ps["cycles"] * us / ut / n
        acc["fdram"] += ps.get(f"ls_{ev}_fills_from_sys.dram_io_far", 0.0) / n
        acc["ndram"] += ps.get(f"ls_{ev}_fills_from_sys.dram_io_near", 0.0) / n
        acc["fcache"] += ps.get(f"ls_{ev}_fills_from_sys.far_cache", 0.0) / n
        if ev == "dmnd":
            acc["ncache"] = (acc["ncache"] or 0.0) + ps["ls_dmnd_fills_from_sys.near_cache"] / n
        acc["wall"] += sec["wall_seconds"] * 1e9 / n   # ns of wall per native (whole run)
        acc["ghz"] += ps["cycles"] / ut / 1e9
        acc["threads"] = rec["run"]["threads"]
        acc["prep_per_native"] += (rec.get("replicas") or {}).get("count", 1) / n
    m = len(runs)
    return {k: (v / m if isinstance(v, float) else v) for k, v in acc.items()}


def main():
    a = sys.argv[1:]
    near_from, lat_dram, lat_x, prep_dram = None, 775.0, 775.0, 8.8e7
    while a and a[0].startswith("--"):
        if a[0] == "--near-cache-from":
            near_from = a[1]
        elif a[0] == "--dram-cycles":
            lat_dram = float(a[1])
        elif a[0] == "--xccx-cycles":
            lat_x = float(a[1])
        elif a[0] == "--prep-dram":
            prep_dram = float(a[1])
        a = a[2:]
    b = side(a[0].split(","))
    r = side(a[1].split(","))
    near_label = "[M]"
    if r["ncache"] is None and near_from:
        r["ncache"] = side([near_from])["ncache"]
        near_label = f"[E, from {os.path.basename(near_from)}]"
    ncache = r["ncache"] or 0.0
    extra = r["cyc"] - b["cyc"]
    d_fdram = r["fdram"] - b["fdram"]
    d_dram = (r["fdram"] + r["ndram"] - prep_dram * r["prep_per_native"]) \
        - (b["fdram"] + b["ndram"] - prep_dram * b["prep_per_native"])
    xccx = r["fcache"] + ncache
    c_dram = max(d_fdram, 0.0) * lat_dram
    c_x = xccx * lat_x
    resid = extra - c_dram - c_x
    k = r["threads"]
    stalled = k * (1 - b["cyc"] / r["cyc"])
    lam = xccx / r["wall"] * 1e9  # transfers per second, whole process
    w_ns = stalled / lam * 1e9 if lam else float("nan")
    print(f"| item (per native) | value | cycles | share of extra |")
    print("|---|---:|---:|---:|")
    print(f"| native cycles, K=1 ({a[0]}) | - | {b['cyc']:.3e} | - |")
    print(f"| native cycles, K={k} ({a[1]}) | - | {r['cyc']:.3e} | - |")
    print(f"| extra cycles | ratio {r['cyc'] / b['cyc']:.3f} | {extra:.3e} | 100% |")
    print(f"| extra far-DRAM fills x {lat_dram:.0f} cycles (upper bound) | {d_fdram:.3e} | {c_dram:.3e} "
          f"| {100 * c_dram / extra:.1f}% |")
    print(f"| (change of all DRAM fills, near + far, minus {prep_dram:.2e} per prepare [E]) | {d_dram:.3e} | - | - |")
    print(f"| cross-CCX fills: far cache [M] + near cache {near_label} x {lat_x:.0f} cycles (uncontended bound) "
          f"| {r['fcache']:.3e} + {ncache:.3e} | {c_x:.3e} | {100 * c_x / extra:.1f}% |")
    print(f"| residual (not covered by counts x uncontended latency bounds) | - | {resid:.3e} "
          f"| {100 * resid / extra:.1f}% |")
    print()
    print(f"If the residual plus the transfer term is carried by the cross-CCX fills: "
          f"{(resid + c_x) / xccx:.3e} cycles ({(resid + c_x) / xccx / r['ghz']:.0f} ns at {r['ghz']:.2f} GHz) per "
          f"transfer, {(resid + c_x) / xccx / lat_x:.1f}x the uncontended bound [E].")
    print(f"Queue view [E]: stalled threads L = {k} x (1 - K1/K) = {stalled:.1f}; cross-CCX transfer rate "
          f"lambda = {lam:.3e}/s; W = L/lambda = {w_ns:.0f} ns = {w_ns * r['ghz']:.3e} cycles per transfer "
          f"(same data as a queue, not independent). One line serviced every t ns carries at most 1e9/t "
          f"transfers/s: lambda needs >= {lam * 100e-9:.1f} line(s) at t = 100 ns, {lam * 200e-9:.1f} at 200 ns.")


if __name__ == "__main__":
    main()
