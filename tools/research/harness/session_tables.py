#!/usr/bin/env python3
"""Markdown tables for one harness measurement session.

  session_tables.py SESSION_ROOT BASE_RUN [BASE_RUN_FOR_C4L_PREFIX...]

Every run directory under SESSION_ROOT (and every parallel4 group, whose
node*/ members are merged) is summarized with analyze.py and compared with
BASE_RUN (matched by fixture index, mean over passes). Four-loop runs named
c4l-<family>-k96 are compared with c4l-<family>-k1.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import analyze  # noqa: E402


def runs(root):
    out = []
    for name in sorted(os.listdir(root)):
        path = os.path.join(root, name)
        if not os.path.isdir(path) or name.startswith("."):
            continue
        if os.path.exists(os.path.join(path, "receipt.json")):
            out.append((name, [path]))
        else:
            members = sorted(os.path.join(path, m) for m in os.listdir(path)
                             if os.path.exists(os.path.join(path, m, "receipt.json")))
            if members:
                out.append((name, members))
    return out


def merged_rows(paths):
    rows = []
    offset = 0
    for p in paths:
        rs = analyze.load_natives(p)
        # each member has its own clock origin: keep intervals per member
        for r in rs:
            r = dict(r)
            r["start_ns"] += offset
            r["end_ns"] += offset
            rows.append(r)
        offset += 10**15
    return rows


def ratio_rows(base_rows, rows, k):
    base = analyze.mean_by_index(base_rows)
    conc = analyze.concurrency(rows) if rows else []
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
                            "cpu_ms": 1e3 * cc / 1e9 / max(len(ids), 1), "base_cpu_ms": 1e3 * cb / 1e9 / max(len(ids), 1)}
        res[label] = entry
    return res


def fmt(x, d=3):
    return "-" if x is None else f"{x:.{d}f}"


def main():
    root, base_name = sys.argv[1], sys.argv[2]
    all_runs = runs(root)
    byname = dict(all_runs)
    base_rows = analyze.load_natives(byname[base_name][0])
    print(f"Session `{root}`; base `{base_name}` (matched by fixture index, mean over passes).\n")
    print("| run | K | CPUs | NUMA | alloc | natives | CPU/native vs base (all) | Apply | Route | at >=90% K | wall/CPU | IPC | DRAM fills far % | foreign % of set | sha256[:12] |")
    print("|---|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|")
    for name, paths in all_runs:
        summaries = [analyze.summarize(p) for p in paths]
        s0 = summaries[0]
        k = sum(s["threads"] for s in summaries)
        rows = merged_rows(paths) if len(paths) > 1 else analyze.load_natives(paths[0])
        if name.startswith("c4l-"):
            fam = name.split("-")[1]
            b = byname.get(f"c4l-{fam}-k1")
            brows = analyze.load_natives(b[0]) if b else None
        else:
            brows = base_rows
        if brows is None or not rows:
            continue
        r = ratio_rows(brows, rows, s0["threads"])
        cpu = sum(s["native_cpu_seconds"] for s in summaries)
        wall = sum(s["native_wall_seconds"] for s in summaries)
        ipc = [s["perf"]["ipc"] for s in summaries if s.get("perf") and s["perf"].get("ipc")]
        far = [s["perf"]["far_fraction_of_dram_fills"] for s in summaries if s.get("perf") and s["perf"].get("far_fraction_of_dram_fills") is not None]
        foreign = [s["foreign"]["foreign_fraction_of_set"] for s in summaries if s.get("foreign")]
        print(f"| {name} | {k} | {s0['cpus'] if len(paths) == 1 else '4 nodes'} | {s0['numa'] if len(paths) == 1 else 'bind per node'} | {s0['alloc']} | "
              f"{sum(s['natives'] for s in summaries)} | {fmt(r['all']['all']['cpu'])} | {fmt(r['all']['Apply']['cpu'])} | {fmt(r['all']['Route']['cpu'])} | "
              f"{fmt(r['full']['all']['cpu'])} | {fmt(wall / cpu if cpu else None)} | {fmt(sum(ipc) / len(ipc) if ipc else None, 2)} | "
              f"{fmt(100 * sum(far) / len(far) if far else None, 1)} | {fmt(100 * max(foreign) if foreign else None, 1)} | {(s0.get('bin_sha256') or '')[:12]} |")
    print()
    print("Per-run absolute CPU per native (ms, matched set, all passes averaged): base -> run")
    for name, paths in all_runs:
        rows = merged_rows(paths) if len(paths) > 1 else analyze.load_natives(paths[0])
        if not rows or name.startswith("c4l-"):
            continue
        r = ratio_rows(base_rows, rows, analyze.summarize(paths[0])["threads"])
        print(f"- {name}: Apply {fmt(r['all']['Apply']['base_cpu_ms'])} -> {fmt(r['all']['Apply']['cpu_ms'])}; Route {fmt(r['all']['Route']['base_cpu_ms'], 4)} -> {fmt(r['all']['Route']['cpu_ms'], 4)}")


if __name__ == "__main__":
    main()
