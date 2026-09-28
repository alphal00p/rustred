#!/usr/bin/env python3
"""Domain-volume metrics of walks at matched natives (completed native inspections).

Input per run: NAME=DIR, where DIR is a probe.py directory (timeseries.jsonl) or
a run_four.py arm directory (events.jsonl heartbeats). At each matched native
count N (linear interpolation between samples; roots closed are a step
function, the value of the first sample at or after N):
  discovered D(N) (scheduled domains), pending P(N) (queued domains), dependency
  edges E(N), roots closed, peak pending up to N, discovered per native D/N, and
  over the interval since the previous matched point the pending growth per
  completion dP/dN and the new domains per completion dD/dN.
Whole run: final natives / discovered / pending / edges / roots, peak pending,
and from the run's side files when present: foreign busy CPUs on the run's CPU
set (loadmon summary: foreign seconds / wall), own CPU seconds, summed thread
run delay and instructions per native (run_four.py --perf-stat metrics.json).

Usage: volume.py NAME=DIR [NAME=DIR ...] [--at 250000,500000,...] [--output OUT.json]
"""
import argparse
import json
from pathlib import Path


def series_of(d):
    ts = d / "timeseries.jsonl"
    rows = []
    if ts.exists():
        for line in open(ts):
            if line.strip():
                r = json.loads(line)
                rows.append({"n": r.get("completed_nodes"), "d": r.get("scheduled_nodes"),
                             "p": r.get("queued_nodes"), "e": r.get("edges"), "roots": r.get("roots_closed"),
                             "t": r.get("t")})
    else:
        for line in open(d / "events.jsonl"):
            if '"heartbeat"' not in line:
                continue
            h = json.loads(line)
            p = h.get("progress") or {}
            if p.get("completed_nodes") is None or p.get("scheduled_nodes") is None:
                continue
            dc = p.get("descendant_closure") or {}
            rows.append({"n": p["completed_nodes"], "d": p["scheduled_nodes"], "p": p.get("queued_nodes"),
                         "e": dc.get("dependency_edges"), "roots": dc.get("initial_closed"),
                         "t": h.get("elapsed_seconds")})
    rows = [r for r in rows if r["n"] is not None]
    rows.sort(key=lambda r: (r["n"], r["t"] or 0))
    return rows


def interp(rows, n, key):
    prev = None
    for r in rows:
        if r["n"] >= n:
            if prev is None or r[key] is None or prev[key] is None or r["n"] == prev["n"]:
                return r[key]
            f = (n - prev["n"]) / (r["n"] - prev["n"])
            return prev[key] + f * (r[key] - prev[key])
        prev = r
    return None


def step(rows, n, key):
    for r in rows:
        if r["n"] >= n:
            return r[key]
    return None


def side_files(d):
    out = {}
    for load in (d.parent / f"{d.name}.load.jsonl", d.parent.parent / f"{d.parent.name}.load.jsonl"):
        if load.exists():
            rows = [json.loads(x) for x in open(load) if x.strip()]
            s = next((r for r in rows if r.get("summary")), None)
            if s and s.get("wall_s"):
                out["load_file"] = str(load)
                out["ncpus"] = s.get("ncpus")
                out["foreign_busy_cpus"] = round(s["foreign_s"] / s["wall_s"], 2)
                out["foreign_share"] = s.get("foreign_share")
                out["own_busy_cpus"] = round(s["own_s"] / s["wall_s"], 2)
                out["own_cpu_seconds"] = s["own_s"]
            break
    m = d / "metrics.json"
    if m.exists():
        mj = json.loads(m.read_text())
        for k in ("own_cpu_seconds", "run_delay_seconds", "instructions_per_native", "perf", "exit_code"):
            if k in mj:
                out[k] = mj[k]
    return out


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("runs", nargs="+")
    p.add_argument("--at", default="500000,1000000,1500000,2000000,2500000,3000000")
    p.add_argument("--output", type=Path)
    args = p.parse_args(argv)
    points = [int(x) for x in args.at.split(",")]
    result = {"points": points, "runs": {}}
    for spec in args.runs:
        name, _, d = spec.partition("=")
        d = Path(d)
        rows = series_of(d)
        last = rows[-1]
        at = []
        prev_n, prev_p, prev_d = 0, 0, 0
        for n in points:
            if n > last["n"]:
                at.append(None)
                continue
            dv, pv, ev = interp(rows, n, "d"), interp(rows, n, "p"), interp(rows, n, "e")
            peak = max([r["p"] for r in rows if r["n"] <= n and r["p"] is not None] + [pv or 0])
            at.append({"natives": n, "discovered": round(dv), "pending": round(pv), "edges": round(ev) if ev else None,
                       "roots_closed": step(rows, n, "roots"), "peak_pending": round(peak),
                       "discovered_per_native": round(dv / n, 3),
                       "pending_growth_per_completion": round((pv - prev_p) / (n - prev_n), 3),
                       "new_domains_per_completion": round((dv - prev_d) / (n - prev_n), 3),
                       "t": round(interp(rows, n, "t") or 0, 1)})
            prev_n, prev_p, prev_d = n, pv, dv
        final = {"natives": last["n"], "discovered": last["d"], "pending": last["p"], "edges": last["e"],
                 "roots_closed": last["roots"], "peak_pending": max(r["p"] or 0 for r in rows),
                 "discovered_per_native": round(last["d"] / max(1, last["n"]), 3), "t": last["t"]}
        result["runs"][name] = {"dir": str(d), "at": at, "final": final, **side_files(d)}
    if args.output:
        args.output.write_text(json.dumps(result, indent=1) + "\n")
    names = list(result["runs"])
    print("natives | " + " | ".join(f"{k}: D / P / peakP / D/n / dP/dn / dD/dn / E / roots" for k in names))
    for i, n in enumerate(points):
        cells = []
        for k in names:
            a = result["runs"][k]["at"][i]
            cells.append("-" if a is None else
                         f"{a['discovered']/1e6:.2f}M / {a['pending']/1e6:.2f}M / {a['peak_pending']/1e6:.2f}M / "
                         f"{a['discovered_per_native']:.2f} / {a['pending_growth_per_completion']:+.2f} / "
                         f"{a['new_domains_per_completion']:.2f} / "
                         f"{(a['edges'] or 0)/1e6:.1f}M / {a['roots_closed']}")
        print(f"{n/1e6:.2f}M | " + " | ".join(cells))
    for k in names:
        r = result["runs"][k]
        f = r["final"]
        print(f"{k}: final natives {f['natives']}, discovered {f['discovered']}, pending {f['pending']}, "
              f"peak pending {f['peak_pending']}, D/n {f['discovered_per_native']}, edges {f['edges']}, "
              f"roots {f['roots_closed']}; foreign busy CPUs {r.get('foreign_busy_cpus')} of {r.get('ncpus')}, "
              f"own CPU s {r.get('own_cpu_seconds')}, run delay s {r.get('run_delay_seconds')}, "
              f"instructions/native {r.get('instructions_per_native')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
