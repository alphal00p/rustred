#!/usr/bin/env python3
"""Per-native cache-fill counters of harness runs (perf stat -x, output of run_harness.sh).

  fills_table.py [--prepare EVENTSET=PREP_RUN ...] RUN_OR_GROUP...

perf stat counts the whole process, including the owner import (prepare). A
prepare-only run (LIMIT=0, same binary family, same events) is subtracted once
per prepare the run performed (members x replicas), then counts are divided by
the natives inspected. A group directory (node*/, ccd*/ members) is summed.
Events understood (AMD Zen 4, user mode):
  ls_dmnd_fills_from_sys.{local_ccx,near_cache,far_cache,dram_io_near,dram_io_far}
  ls_any_fills_from_sys.{dram_io_near,dram_io_far,far_cache}, l2_cache_req_stat.ic_dc_miss_in_l2
near_cache = line supplied by another CCX's cache in the same NUMA node,
far_cache = by a CCX's cache in another node (both: cache-to-cache transfers,
i.e. lines written or held by a thread on another L3).
"""
import json
import os
import sys


def members(path):
    if os.path.exists(os.path.join(path, "receipt.json")):
        return [path]
    return sorted(os.path.join(path, m) for m in os.listdir(path)
                  if os.path.exists(os.path.join(path, m, "receipt.json")))


def perfstat(run):
    out = {}
    p = os.path.join(run, "perfstat.csv")
    if not os.path.exists(p):
        return out
    for line in open(p):
        parts = line.strip().split(",")
        if len(parts) < 3 or not parts[0] or parts[0].startswith("#"):
            continue
        try:
            out[parts[2].replace(":u", "")] = float(parts[0])
        except ValueError:
            pass
    return out


def main():
    args = sys.argv[1:]
    preps = {}
    runs = []
    while args:
        if args[0] == "--prepare":
            key, path = args[1].split("=", 1)
            preps[key] = perfstat(path)
            args = args[2:]
        else:
            runs.append(args[0])
            args = args[1:]
    cols = ["cycles", "instructions", "ls_dmnd_fills_from_sys.local_ccx", "ls_dmnd_fills_from_sys.near_cache",
            "ls_dmnd_fills_from_sys.far_cache", "ls_dmnd_fills_from_sys.dram_io_near",
            "ls_dmnd_fills_from_sys.dram_io_far", "ls_any_fills_from_sys.far_cache",
            "ls_any_fills_from_sys.dram_io_near", "ls_any_fills_from_sys.dram_io_far",
            "l2_cache_req_stat.ic_dc_miss_in_l2"]
    short = {"cycles": "cyc", "instructions": "ins", "ls_dmnd_fills_from_sys.local_ccx": "d.lccx",
             "ls_dmnd_fills_from_sys.near_cache": "d.ncache", "ls_dmnd_fills_from_sys.far_cache": "d.fcache",
             "ls_dmnd_fills_from_sys.dram_io_near": "d.ndram", "ls_dmnd_fills_from_sys.dram_io_far": "d.fdram",
             "ls_any_fills_from_sys.far_cache": "a.fcache", "ls_any_fills_from_sys.dram_io_near": "a.ndram",
             "ls_any_fills_from_sys.dram_io_far": "a.fdram", "l2_cache_req_stat.ic_dc_miss_in_l2": "l2miss"}
    rows = []
    for run in runs:
        ms = members(run)
        if not ms:
            continue
        tot, natives, prepares, sha = {}, 0, 0, ""
        for m in ms:
            rec = json.load(open(os.path.join(m, "receipt.json")))
            natives += rec["section"]["natives"]
            prepares += (rec.get("replicas") or {}).get("count", 1)
            for k, v in perfstat(m).items():
                tot[k] = tot.get(k, 0.0) + v
            env = dict(line.rstrip("\n").split("=", 1) for line in open(os.path.join(m, "run.env")) if "=" in line)
            sha = env.get("BIN_SHA256", "")[:12]
        evset = "dmnd" if "ls_dmnd_fills_from_sys.near_cache" in tot else "any"
        prep = preps.get(f"{evset}:{sha}") or preps.get(evset) or {}
        per = {}
        for c in cols:
            if c in tot:
                per[c] = (tot[c] - prepares * prep.get(c, 0.0)) / natives
        row = {"run": run.rstrip("/"), "natives": natives, "prepares": prepares, "sha": sha,
               "prepare_subtracted": bool(prep), "per_native": per}
        d = per
        if evset == "dmnd":
            xccx = d.get("ls_dmnd_fills_from_sys.near_cache", 0) + d.get("ls_dmnd_fills_from_sys.far_cache", 0)
            beyond_l2 = xccx + sum(d.get(f"ls_dmnd_fills_from_sys.{k}", 0) for k in ("local_ccx", "dram_io_near", "dram_io_far"))
            remote = d.get("ls_dmnd_fills_from_sys.far_cache", 0) + d.get("ls_dmnd_fills_from_sys.dram_io_far", 0)
            row["xccx_per_native"] = xccx
            row["xccx_share_of_fills_beyond_l2"] = xccx / beyond_l2 if beyond_l2 else None
            row["remote_share_of_fills_beyond_l3"] = remote / (beyond_l2 - d.get("ls_dmnd_fills_from_sys.local_ccx", 0)) \
                if beyond_l2 else None
        rows.append(row)
    present = [c for c in cols if any(c in r["per_native"] for r in rows)]
    print("| run | natives | prepares subtracted | " + " | ".join(short[c] for c in present)
          + " | x-CCX fills | x-CCX share | remote share (beyond L3) | IPC |")
    print("|---|---:|---:|" + "---:|" * (len(present) + 4))
    for r in rows:
        d = r["per_native"]
        ipc = d["instructions"] / d["cycles"] if d.get("cycles") else None
        cells = [f"{d[c]:.3e}" if c in d else "-" for c in present]
        x = r.get("xccx_per_native")
        xs = r.get("xccx_share_of_fills_beyond_l2")
        rs = r.get("remote_share_of_fills_beyond_l3")
        print(f"| {os.path.basename(r['run'])} | {r['natives']} | {r['prepares'] if r['prepare_subtracted'] else 0} | "
              + " | ".join(cells)
              + f" | {'-' if x is None else f'{x:.3e}'} | {'-' if xs is None else f'{100 * xs:.1f}%'} | "
              + f"{'-' if rs is None else f'{100 * rs:.1f}%'} | {'-' if ipc is None else f'{ipc:.2f}'} |")
    if os.environ.get("FILLS_JSON"):
        json.dump(rows, open(os.environ["FILLS_JSON"], "w"), indent=1)


if __name__ == "__main__":
    main()
