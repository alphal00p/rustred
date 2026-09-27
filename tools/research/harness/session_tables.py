#!/usr/bin/env python3
"""Markdown tables for harness measurement sessions (lean: one load per run).

  session_tables.py [--cache DIR] BASE_RUN RUN_OR_GROUP...

Each RUN_OR_GROUP is a run directory of run_harness.sh or a group directory
whose members (node*/, ccd*/ ...) each hold a receipt.json (parallel
processes; their natives are merged). Each is summarized with analyze.py and
compared with BASE_RUN (matched by fixture index, mean over passes). A run
named c4l-<family>-k<K> is compared with the sibling c4l-<family>-k1 instead.
Per-run results are cached as JSON in --cache (keyed by the run path), so a
table over a whole session can be rebuilt run by run without holding more
than one run's natives in memory.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import analyze  # noqa: E402


def members(path):
    if os.path.exists(os.path.join(path, "receipt.json")):
        return [path]
    return sorted(os.path.join(path, m) for m in os.listdir(path)
                  if os.path.exists(os.path.join(path, m, "receipt.json")))


def rows_of(paths):
    rows = []
    for n, p in enumerate(paths):
        for r in analyze.load_natives(p):
            # each member has its own clock origin: keep intervals per member
            r["start_ns"] += n * 10**15
            r["end_ns"] += n * 10**15
            rows.append(r)
    return rows


def ratio_rows(base, rows, conc, k):
    res = {}
    for label, keep in (("all", None), ("full", [c >= 0.9 * k for c in conc])):
        cur = analyze.mean_by_index(rows, keep)
        matched = [i for i in cur if i in base]
        entry = {}
        for phase in ("Apply", "Route", "all"):
            ids = [i for i in matched if phase == "all" or cur[i][2] == phase]
            cb = sum(base[i][0] for i in ids)
            cc = sum(cur[i][0] for i in ids)
            wb = sum(base[i][1] for i in ids)
            wc = sum(cur[i][1] for i in ids)
            entry[phase] = {"n": len(ids), "cpu": cc / cb if cb else None, "wall": wc / wb if wb else None,
                            "cpu_ms": 1e3 * cc / 1e9 / max(len(ids), 1),
                            "base_cpu_ms": 1e3 * cb / 1e9 / max(len(ids), 1)}
        res[label] = entry
    return res


def evaluate(path, base_path, cache):
    key = hashlib.sha256(f"{os.path.abspath(path)}|{os.path.abspath(base_path)}".encode()).hexdigest()[:16]
    cfile = os.path.join(cache, f"{os.path.basename(path)}-{key}.json") if cache else None
    if cfile and os.path.exists(cfile):
        return json.load(open(cfile))
    paths = members(path)
    base = analyze.mean_by_index(analyze.load_natives(base_path))
    summaries = []
    rows_all = []
    for p in paths:
        rows = analyze.load_natives(p)
        conc = analyze.concurrency(rows) if rows else []
        s = analyze.summarize(p, rows, conc)
        s.pop("by_phase", None)
        summaries.append((s, rows, conc))
    k_member = summaries[0][0]["threads"]
    if len(paths) == 1:
        rows, conc = summaries[0][1], summaries[0][2]
    else:
        rows = rows_of(paths)
        conc = [c for _, _, cs in summaries for c in cs]
    ratios = ratio_rows(base, rows, conc, k_member)
    out = {"run": path, "base": base_path, "members": len(paths),
           "threads_total": sum(s["threads"] for s, _, _ in summaries), "threads_per_member": k_member,
           "summaries": [s for s, _, _ in summaries], "ratios": ratios}
    if cfile:
        os.makedirs(cache, exist_ok=True)
        json.dump(out, open(cfile, "w"), default=float)
    return out


def fmt(x, d=3):
    return "-" if x is None else f"{x:.{d}f}"


def main():
    args = sys.argv[1:]
    cache = None
    if args and args[0] == "--cache":
        cache, args = args[1], args[2:]
    base_path, runs = args[0], args[1:]
    print("| run | K | procs | CPUs | NUMA | alloc | natives | CPU/native vs base (all) | Apply | Route | at >=90% K "
          "| wall/CPU | IPC | far % of DRAM fills | foreign % of set (max) | base Apply ms -> run | sha256[:12] |")
    print("|---|---:|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|")
    for run in runs:
        name = os.path.basename(os.path.normpath(run))
        base = base_path
        if name.startswith("c4l-"):
            fam = name.split("-")[1]
            base = os.path.join(os.path.dirname(os.path.normpath(run)), f"c4l-{fam}-k1")
        if not members(run):
            print(f"| {name} | (no finished run: no receipt.json) |")
            continue
        e = evaluate(run, base, cache)
        ss = e["summaries"]
        r = e["ratios"]
        cpu = sum(s["native_cpu_seconds"] for s in ss)
        wall = sum(s["native_wall_seconds"] for s in ss)
        ipc = [s["perf"]["ipc"] for s in ss if s.get("perf") and s["perf"].get("ipc")]
        far = [s["perf"]["far_fraction_of_dram_fills"] for s in ss
               if s.get("perf") and s["perf"].get("far_fraction_of_dram_fills") is not None]
        foreign = [s["foreign"]["foreign_fraction_of_set"] for s in ss if s.get("foreign")]
        cpus = ss[0]["cpus"] if len(ss) == 1 else f"{len(ss)} x {ss[0]['threads']}"
        numa = ss[0]["numa"] if len(ss) == 1 else "bind per member"
        print(f"| {name} | {e['threads_total']} | {e['members']} | {cpus} | {numa} | {ss[0]['alloc']} | "
              f"{sum(s['natives'] for s in ss)} | {fmt(r['all']['all']['cpu'])} | {fmt(r['all']['Apply']['cpu'])} | "
              f"{fmt(r['all']['Route']['cpu'])} | {fmt(r['full']['all']['cpu'])} | {fmt(wall / cpu if cpu else None)} | "
              f"{fmt(sum(ipc) / len(ipc) if ipc else None, 2)} | {fmt(100 * sum(far) / len(far) if far else None, 1)} | "
              f"{fmt(100 * max(foreign) if foreign else None, 1)} | "
              f"{fmt(r['all']['Apply']['base_cpu_ms'], 2)} -> {fmt(r['all']['Apply']['cpu_ms'], 2)} | "
              f"{(ss[0].get('bin_sha256') or '')[:12]} |")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
