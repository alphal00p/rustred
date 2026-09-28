#!/usr/bin/env python3
"""W0.3 harness analysis: per-run summaries and CPU-per-native ratios.

Usage:
  analyze.py summary RUN_DIR...                  one JSON summary per run
  analyze.py ratio BASE_RUN RUN_DIR...           matched CPU-per-native ratios vs BASE_RUN
  analyze.py cancel RUN_DIR                      cancellation-latency histogram

A run directory is the output of run_harness.sh (receipt.json, natives.jsonl,
run.env, procstat.before/after, perfstat.csv). Matching is by fixture index
`i`; a native's cost is the mean over its passes. "full" natives are those
whose interval saw on average >= 90% of the run's K threads busy (computed
exactly from the start/end stamps by a sweep).
"""
import json
import math
import os
import sys
from collections import defaultdict


# Fields the analyses read; everything else (counts, stats JSON) is dropped on
# load so that a 72,000-native K=96 run stays small in memory.
KEEP = ("i", "id", "pass", "thread", "phase", "stratum", "cpu_ns", "wall_ns", "start_ns", "end_ns",
        "active_start", "active_end", "voluntary_switches", "involuntary_switches", "minor_faults",
        "error_kind", "cancel")


def load_natives(run, keep=KEEP):
    rows = []
    with open(os.path.join(run, "natives.jsonl")) as f:
        for line in f:
            r = json.loads(line)
            rows.append({k: r[k] for k in keep if k in r} if keep else r)
    return rows


def read_env(run):
    env = {}
    path = os.path.join(run, "run.env")
    if os.path.exists(path):
        for line in open(path):
            if "=" in line:
                k, v = line.rstrip("\n").split("=", 1)
                env[k] = v
    return env


def cpu_set(spec):
    out = set()
    for part in spec.split(","):
        if "-" in part:
            a, b = part.split("-")
            out.update(range(int(a), int(b) + 1))
        elif part:
            out.add(int(part))
    return out


def procstat(path):
    table = {}
    for line in open(path):
        parts = line.split()
        if parts[0] == "cpu":
            continue
        cpu = int(parts[0][3:])
        vals = [int(x) for x in parts[1:]]
        idle = vals[3] + vals[4]
        table[cpu] = (sum(vals[:8]), idle)
    return table


def foreign_load(run, receipt):
    env = read_env(run)
    before, after = os.path.join(run, "procstat.before"), os.path.join(run, "procstat.after")
    if not (os.path.exists(before) and os.path.exists(after) and "CPUS" in env):
        return None
    cpus = cpu_set(env["CPUS"])
    b, a = procstat(before), procstat(after)
    tck = os.sysconf("SC_CLK_TCK")
    busy = sum((a[c][0] - a[c][1]) - (b[c][0] - b[c][1]) for c in cpus if c in a) / tck
    wall = float(open(os.path.join(run, "end.unix")).read()) - float(open(os.path.join(run, "start.unix")).read())
    ru = receipt.get("section", {}).get("rusage_after", {})
    own = (ru.get("user_seconds") or 0) + (ru.get("system_seconds") or 0)
    foreign = max(0.0, busy - own)
    return {"cpus": len(cpus), "wall_seconds": wall, "busy_cpu_seconds_on_set": busy,
            "own_process_cpu_seconds": own, "foreign_cpu_seconds": foreign,
            "foreign_fraction_of_set": foreign / (len(cpus) * wall) if wall > 0 else None}


def perfstat(run):
    path = os.path.join(run, "perfstat.csv")
    if not os.path.exists(path):
        return None
    out = {}
    for line in open(path):
        parts = line.strip().split(",")
        if len(parts) < 3 or not parts[0] or parts[0].startswith("#"):
            continue
        try:
            value = float(parts[0])
        except ValueError:
            continue
        out[parts[2]] = value
    return out


def concurrency(rows):
    """Mean number of busy natives over each native's own interval."""
    events = []
    for n, r in enumerate(rows):
        events.append((r["start_ns"], 1, n))
        events.append((r["end_ns"], -1, n))
    events.sort(key=lambda e: (e[0], e[1]))
    # integral of active(t) dt accumulated globally; per native = F(end)-F(start)
    integral_at = {}
    active, last_t, acc = 0, None, 0.0
    for t, delta, n in events:
        if last_t is not None:
            acc += active * (t - last_t)
        last_t = t
        key = (n, "s" if delta == 1 else "e")
        integral_at[key] = acc
        active += delta
    out = []
    for n, r in enumerate(rows):
        span = r["end_ns"] - r["start_ns"]
        if span <= 0:
            out.append(float(r.get("active_start", 1)))
        else:
            out.append((integral_at[(n, "e")] - integral_at[(n, "s")]) / span)
    return out


def summarize(run, rows=None, conc=None):
    receipt = json.load(open(os.path.join(run, "receipt.json")))
    rows = load_natives(run) if rows is None else rows
    k = receipt.get("run", {}).get("threads", 1)
    conc = (concurrency(rows) if rows else []) if conc is None else conc
    by_phase = defaultdict(lambda: {"natives": 0, "cpu_s": 0.0, "wall_s": 0.0, "vcsw": 0, "ivcsw": 0,
                                    "minflt": 0, "errors": defaultdict(int)})
    for r in rows:
        p = by_phase[r["phase"]]
        p["natives"] += 1
        p["cpu_s"] += r["cpu_ns"] / 1e9
        p["wall_s"] += r["wall_ns"] / 1e9
        p["vcsw"] += r["voluntary_switches"]
        p["ivcsw"] += r["involuntary_switches"]
        p["minflt"] += r["minor_faults"]
        p["errors"][r["error_kind"]] += 1
    for p in by_phase.values():
        p["cpu_ms_per_native"] = 1e3 * p["cpu_s"] / max(p["natives"], 1)
        p["off_cpu_fraction"] = 1 - p["cpu_s"] / p["wall_s"] if p["wall_s"] else None
        p["errors"] = dict(p["errors"])
    threads = receipt.get("section", {}).get("threads", [])
    run_delay = sum(t.get("schedstat_run_delay_ns", 0) for t in threads) / 1e9
    on_cpu = sum(t.get("schedstat_on_cpu_ns", 0) for t in threads) / 1e9
    section_wall = receipt.get("section", {}).get("wall_seconds")
    full = [c >= 0.9 * k for c in conc]
    pstat = perfstat(run)
    perf = None
    if pstat:
        cyc = pstat.get("cycles:u")
        ins = pstat.get("instructions:u")
        near = pstat.get("ls_any_fills_from_sys.dram_io_near:u", 0)
        far = pstat.get("ls_any_fills_from_sys.dram_io_far:u", 0)
        wall = float(open(os.path.join(run, "end.unix")).read()) - float(open(os.path.join(run, "start.unix")).read())
        perf = {"ipc": ins / cyc if cyc else None, "cycles": cyc, "instructions": ins,
                "l1d_fills_from_dram_near": near, "l1d_fills_from_dram_far": far,
                "far_fraction_of_dram_fills": far / (near + far) if near + far else None,
                "dram_fill_GBps_whole_process": 64 * (near + far) / wall / 1e9 if wall else None,
                "l2_misses": pstat.get("l2_cache_req_stat.ic_dc_miss_in_l2:u"),
                "events": pstat,
                "scope": "user-mode core events for the whole process (prepare included); L1D fills from DRAM count 64-byte lines (demand + L1 prefetch), not L2 prefetches, so the GB/s is a lower bound on DRAM traffic"}
    env = read_env(run)
    return {"run": run, "bin_sha256": env.get("BIN_SHA256"), "cpus": env.get("CPUS"), "numa": env.get("NUMA"),
            "alloc": env.get("ALLOC"), "threads": k, "natives": len(rows),
            "passed": receipt.get("passed"), "error": receipt.get("error"),
            "prepare_seconds": receipt.get("timings", {}).get("prepare_seconds"),
            "section_wall_seconds": section_wall,
            "native_cpu_seconds": sum(r["cpu_ns"] for r in rows) / 1e9,
            "native_wall_seconds": sum(r["wall_ns"] for r in rows) / 1e9,
            "threads_on_cpu_seconds": on_cpu, "threads_run_delay_seconds": run_delay,
            "natives_at_full_concurrency": sum(full),
            "by_phase": by_phase, "perf": perf, "foreign": foreign_load(run, receipt),
            "vm_hwm_kib": receipt.get("memory", {}).get("vm_hwm_kib"),
            "differential": receipt.get("differential", {}) and {
                k2: receipt["differential"].get(k2) for k2 in
                ("passed", "identical", "mismatched", "not_in_tap", "tap_keys_not_reinspected",
                 "harness_natives", "tap_completed_keys", "tap_incomplete_attempts", "by_phase")}}


def mean_by_index(rows, keep=None):
    acc = defaultdict(lambda: [0, 0, 0, None, None])
    for n, r in enumerate(rows):
        if keep is not None and not keep[n]:
            continue
        a = acc[r["i"]]
        a[0] += r["cpu_ns"]
        a[1] += r["wall_ns"]
        a[2] += 1
        a[3] = r["phase"]
        a[4] = r["stratum"]
    return {i: (a[0] / a[2], a[1] / a[2], a[3], a[4]) for i, a in acc.items()}


def ratio(base_run, runs):
    base = mean_by_index(load_natives(base_run))
    out = []
    for run in runs:
        rows = load_natives(run)
        receipt = json.load(open(os.path.join(run, "receipt.json")))
        k = receipt.get("run", {}).get("threads", 1)
        conc = concurrency(rows)
        for label, keep in (("all", None), ("full_concurrency", [c >= 0.9 * k for c in conc])):
            cur = mean_by_index(rows, keep)
            matched = [i for i in cur if i in base]
            res = {"run": run, "threads": k, "subset": label, "matched": len(matched)}
            for phase in ("Apply", "Route", "all"):
                ids = [i for i in matched if phase == "all" or cur[i][2] == phase]
                cb = sum(base[i][0] for i in ids)
                cc = sum(cur[i][0] for i in ids)
                wb = sum(base[i][1] for i in ids)
                wc = sum(cur[i][1] for i in ids)
                per = sorted(cur[i][0] / base[i][0] for i in ids if base[i][0] > 0)
                res[phase] = {"natives": len(ids), "cpu_ratio": cc / cb if cb else None,
                              "wall_ratio": wc / wb if wb else None,
                              "median_per_native_cpu_ratio": per[len(per) // 2] if per else None,
                              "base_cpu_ms_per_native": 1e3 * cb / 1e9 / max(len(ids), 1),
                              "cpu_ms_per_native": 1e3 * cc / 1e9 / max(len(ids), 1)}
            out.append(res)
    return out


def cancel(run):
    rows = load_natives(run)
    hist = defaultdict(int)
    lat = []
    outcomes = defaultdict(int)
    by_class = defaultdict(list)
    for r in rows:
        c = r.get("cancel")
        if not c:
            outcomes["no_prior_cost"] += 1
            continue
        if not c.get("flag_set"):
            outcomes["finished_before_flag"] += 1
            continue
        kind = r["error_kind"]
        outcomes["flag_set_" + kind] += 1
        if kind != "cancelled":
            continue
        ns = c["latency_ns"]
        wall_class = "<1ms" if r["wall_ns"] < 1e6 else "1-100ms" if r["wall_ns"] < 1e8 else ">=100ms"
        by_class[wall_class].append(ns)
        lat.append((ns, r["phase"], r["i"], kind, r["wall_ns"]))
        bucket = 0 if ns <= 0 else int(math.floor(math.log2(ns)))
        hist[bucket] += 1
    lat.sort()

    def q(p):
        return lat[min(len(lat) - 1, int(p * len(lat)))][0] if lat else None
    return {"run": run, "outcomes": dict(outcomes), "latency_ns_quantiles":
            {"p50": q(0.5), "p90": q(0.9), "p99": q(0.99), "max": lat[-1][0] if lat else None},
            "log2_ns_histogram": {f"[2^{b},2^{b+1}) ns": hist[b] for b in sorted(hist)},
            "by_time_to_cancel_return": {k: {"n": len(v), "p50": sorted(v)[len(v) // 2],
                                             "p99": sorted(v)[min(len(v) - 1, int(0.99 * len(v)))],
                                             "max": max(v)} for k, v in by_class.items()},
            "scope": "flag set by a 20-us polling timer at CANCEL_FRACTION of each native's prior wall time; latency = flag store to inspect() return, cancelled natives only",
            "worst": [{"latency_ns": x[0], "phase": x[1], "i": x[2], "error_kind": x[3], "wall_ns": x[4]}
                      for x in lat[-10:]]}


def main():
    mode = sys.argv[1]
    if mode == "summary":
        for run in sys.argv[2:]:
            print(json.dumps(summarize(run), default=float))
    elif mode == "ratio":
        for res in ratio(sys.argv[2], sys.argv[3:]):
            print(json.dumps(res))
    elif mode == "cancel":
        print(json.dumps(cancel(sys.argv[2]), indent=1))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
