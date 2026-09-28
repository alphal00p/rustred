#!/usr/bin/env python3
"""Per-native cache-fill counters of harness runs (perf stat -x, output of run_harness.sh).

  fills_table.py [--prepare EVENTSET[:SHA12]=PREP_RUN ...] RUN_OR_GROUP...

(a reference keyed EVENTSET:SHA12 is used for runs of that binary, else the one keyed EVENTSET; for an
`any` event set without its own DRAM events the reference's demand DRAM fills are used for the bound)

perf stat counts the whole process, including the owner import (prepare). The
table therefore reports RAW per-native values (prepare included) for every
counter, and never subtracts a prepare reference from cycles or fills:

* A prepare-only reference (LIMIT=0) is only valid for runs of the same binary,
  on the same socket, with the same number of CONCURRENT prepares. The first
  version of this tool subtracted `prepare-only/B-ev7` (binary B, one prepare on
  socket-0 CPU 65) from socket-1 binary-C runs with 3 or 12 concurrent prepares,
  which produced negative far-DRAM fills and IPC ~6 (withdrawn 2026-09-28).
* Instructions of a prepare are deterministic, so `ins (native)` subtracts
  prepares x the reference's instructions (checked: 2.91e8 per native for 1, 3
  and 12 prepares).
* Cycles of the natives are estimated from the receipt's rusage split,
  `cyc (native est.)` = cycles x section user CPU / total user CPU [E: assumes
  the same clock during prepare and natives; poor when the prepare share is
  large, e.g. ~50% of user CPU with 12 copies].
* `prep DRAM bound` = prepares x the reference's (near + far) DRAM fills /
  natives: an estimate of the prepare's share of the raw DRAM columns [E: the
  reference ran elsewhere; its total, not its near/far split, is used]. The
  raw far-DRAM column is always an UPPER bound on the natives' far-DRAM fills.
* The cross-CCX columns (near + far cache) are robust: a prepare contributes
  about 1.2e3 such fills in total (reference), i.e. < 1 per native.
* The fill counts mix clean read-shared transfers and contended (written-line)
  transfers; they do not measure cost.

A group directory (node*/, ccd*/ members) is summed. Events understood (AMD Zen 4, user mode):
  ls_dmnd_fills_from_sys.{local_ccx,near_cache,far_cache,dram_io_near,dram_io_far}
  ls_any_fills_from_sys.{dram_io_near,dram_io_far,far_cache}, l2_cache_req_stat.ic_dc_miss_in_l2
near_cache = line supplied by another CCX's cache in the same NUMA node,
far_cache = by a CCX's cache in another node.
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


def fmt(x, spec=".3e"):
    return "-" if x is None else format(x, spec)


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
    rows = []
    for run in runs:
        ms = members(run)
        if not ms:
            continue
        tot, natives, prepares, sha, user_total, user_section = {}, 0, 0, "", 0.0, 0.0
        for m in ms:
            rec = json.load(open(os.path.join(m, "receipt.json")))
            sec = rec["section"]
            natives += sec["natives"]
            prepares += (rec.get("replicas") or {}).get("count", 1)
            user_total += sec["rusage_after"]["user_seconds"]
            user_section += sec["rusage_after"]["user_seconds"] - sec["rusage_before"]["user_seconds"]
            for k, v in perfstat(m).items():
                tot[k] = tot.get(k, 0.0) + v
            env = dict(line.rstrip("\n").split("=", 1) for line in open(os.path.join(m, "run.env")) if "=" in line)
            sha = env.get("BIN_SHA256", "")[:12]
        evset = "dmnd" if "ls_dmnd_fills_from_sys.near_cache" in tot else "any"
        prep = preps.get(f"{evset}:{sha}") or preps.get(evset) or {}
        raw = {k: v / natives for k, v in tot.items()}
        g = lambda k: raw.get(f"ls_{evset}_fills_from_sys.{k}")  # noqa: E731
        row = {"run": run.rstrip("/"), "natives": natives, "prepares": prepares, "sha": sha, "evset": evset,
               "raw_per_native": raw, "prepare_user_share": 1 - user_section / user_total if user_total else None}
        if "cycles" in tot and user_total:
            row["cycles_native_est"] = tot["cycles"] * user_section / user_total / natives
            row["ghz_user"] = tot["cycles"] / user_total / 1e9
        if "instructions" in tot and "instructions" in prep:
            row["instructions_native"] = (tot["instructions"] - prepares * prep["instructions"]) / natives
        if row.get("cycles_native_est") and row.get("instructions_native"):
            row["ipc_native_est"] = row["instructions_native"] / row["cycles_native_est"]
        pd = sum(prep.get(f"ls_{evset}_fills_from_sys.{k}", 0.0) for k in ("dram_io_near", "dram_io_far")) \
            or sum(prep.get(f"ls_dmnd_fills_from_sys.{k}", 0.0) for k in ("dram_io_near", "dram_io_far"))
        row["prep_dram_bound_per_native"] = prepares * pd / natives if pd else None
        row["xccx_per_native"] = (g("near_cache") or 0.0) + (g("far_cache") or 0.0) if evset == "dmnd" else g("far_cache")
        rows.append(row)
    print("| run | natives | prepares | GHz (cycles per user-s) | prepare share of user CPU | cyc/native raw "
          "| cyc/native (natives, est.) | ins/native (natives) | IPC (natives, est.) | local CCX | near cache "
          "| far cache | x-CCX (near+far cache) | near DRAM raw | far DRAM raw | prep DRAM bound |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for r in rows:
        d, ev = r["raw_per_native"], r["evset"]
        k = lambda n: d.get(f"ls_{ev}_fills_from_sys.{n}")  # noqa: E731
        print(f"| {os.path.basename(r['run'])} | {r['natives']} | {r['prepares']} | {fmt(r.get('ghz_user'), '.2f')} "
              f"| {fmt(r['prepare_user_share'], '.3f')} | {fmt(d.get('cycles'))} | {fmt(r.get('cycles_native_est'))} "
              f"| {fmt(r.get('instructions_native'))} | {fmt(r.get('ipc_native_est'), '.2f')} | {fmt(k('local_ccx'))} "
              f"| {fmt(k('near_cache'))} | {fmt(k('far_cache'))} | {fmt(r['xccx_per_native'])} "
              f"| {fmt(k('dram_io_near'))} | {fmt(k('dram_io_far'))} | {fmt(r['prep_dram_bound_per_native'])} |")
    print()
    print("Raw = prepare included. Cross-CCX columns are robust (prepare < 1 fill/native). The raw far-DRAM column "
          "is an upper bound on the natives' far-DRAM fills; `prep DRAM bound` estimates the prepare share of the "
          "DRAM columns from a reference run elsewhere [E]. Cycle/IPC columns for the natives are estimates [E]. "
          "Fill counts mix clean and contended transfers.")
    if os.environ.get("FILLS_JSON"):
        json.dump(rows, open(os.environ["FILLS_JSON"], "w"), indent=1)


if __name__ == "__main__":
    main()
