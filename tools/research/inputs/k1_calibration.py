#!/usr/bin/env python3
"""Per-class K=1 native cost calibration and K=1-equivalent Apply work shares.

HANDOFF 0.1 item 2: "native work" = natives x per-class K=1 cost, never raw
inspector-thread time. The CP5 record field `seconds` is inspector wall time at
the run's thread count (contended). This tool calibrates per-class K=1 costs on a
W0.3 harness re-inspection of a stratified sample of inspected natives
(natives.jsonl: per-native thread `cpu_ns` at K=1, or K=8 on one CCX, which the
harness measured at 1.018x K=1) and applies them to the class totals of
owner_cpu.py (all files = cumulative, and each generation append).

Groups: L (--lstar owners), Ghot (--hot owner), G (other --guards owners), U.
Each sample native carries its fixture stratum weight (population / sampled);
per group and ID tercile ("old", "mid", "young" from the fixture's
tercile_cut_ids, or "all") the tool forms weighted ratio estimators
  k1_per_op  = sum w cpu / sum w native_operations
  contention = sum w recorded_seconds / sum w cpu   (recorded = CP5 record `seconds`)
  k1_per_native = sum w cpu / sum w
and three K=1 work estimates per group and window:
  via_ops     = window ops x k1_per_op
  via_seconds = window recorded seconds / contention
  via_natives = window natives x k1_per_native
L* share of K=1 Apply work = L / (L + Ghot + G + U) for each estimator, with a
stratified bootstrap (resampling within fixture strata) for the calibration
uncertainty. Windows that lie entirely in one tercile use that tercile's
calibration ("matched") as well as the all-sample one.

Usage: k1_calibration.py --natives NATIVES.jsonl --fixture FIXTURE.json --records CP5DIR
         --owner-cpu OWNER_CPU.json --lstar L.json --guards MASKS --hot MASK
         [--boot 1000] [--jobs 8] [--output OUT.json]
"""
import argparse
import json
import random
import re
from collections import defaultdict
from multiprocessing import Pool
from pathlib import Path

ID_RE = re.compile(rb'"id":(\d+)')


def scan(args):
    path, start, end, wanted = args
    out = {}
    with open(path, "rb") as f:
        if start:
            f.seek(start - 1)
            if f.read(1) != b"\n":
                f.readline()
        pos = f.tell()
        while pos < end:
            line = f.readline()
            if not line:
                break
            pos += len(line)
            if b'"record_kind":"native_inspection"' not in line:
                continue
            m = ID_RE.search(line)
            if not m or int(m.group(1)) not in wanted:
                continue
            r = json.loads(line)
            if r.get("id") in wanted:
                s = r.get("stats") or {}
                out[r["id"]] = {"seconds": float(r.get("seconds") or 0.0),
                                "ops": int(s.get("native_operations") or 0),
                                "owner": r["owner"], "phase": r["phase"]}
    return out


def recorded(records_dir, ids, jobs, chunk_bytes=256 << 20):
    wanted = frozenset(ids)
    tasks = []
    for f in sorted(Path(records_dir).glob("records-*.jsonl")):
        size = f.stat().st_size
        tasks.extend((str(f), s, min(size, s + chunk_bytes), wanted) for s in range(0, size, chunk_bytes))
    out = {}
    with Pool(jobs) as pool:
        for part in pool.imap_unordered(scan, tasks):
            out.update(part)
    return out


def group_of(owner, lstar, guards, hot):
    if owner == hot:
        return "Ghot"
    if owner in lstar:
        return "L"
    if owner in guards:
        return "G"
    return "U"


def tercile(i, cuts):
    return "old" if i < cuts[0] else "mid" if i < cuts[1] else "young"


def ratios(rows):
    """rows: list of (w, cpu_s, ops, rec_s). Weighted ratio estimators."""
    sw = sum(r[0] for r in rows)
    cpu = sum(r[0] * r[1] for r in rows)
    ops = sum(r[0] * r[2] for r in rows)
    rec = sum(r[0] * r[3] for r in rows)
    return {"n": len(rows), "weight": sw,
            "k1_per_op_us": 1e6 * cpu / ops if ops else None,
            "contention": rec / cpu if cpu else None,
            "k1_per_native_ms": 1e3 * cpu / sw if sw else None,
            "recorded_per_native_ms": 1e3 * rec / sw if sw else None}


def shares(cal, win):
    """cal: {group: ratios}; win: {group: {natives, seconds, ops}} -> K=1 work per group and L share."""
    est = {}
    for how in ("via_ops", "via_seconds", "via_natives"):
        work = {}
        for g, w in win.items():
            c = cal.get(g)
            if not c or not c["n"]:
                work[g] = None
                continue
            if how == "via_ops":
                work[g] = w["ops"] * c["k1_per_op_us"] * 1e-6
            elif how == "via_seconds":
                work[g] = w["seconds"] / c["contention"]
            else:
                work[g] = w["natives"] * c["k1_per_native_ms"] * 1e-3
        if any(v is None for v in work.values()):
            est[how] = None
            continue
        tot = sum(work.values())
        est[how] = {"k1_seconds": work, "total_k1_seconds": tot,
                    "shares": {g: v / tot for g, v in work.items()}}
    return est


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--natives", type=Path, required=True, action="append",
                   help="harness natives.jsonl (repeatable: replicate runs are averaged per native)")
    p.add_argument("--fixture", type=Path, required=True)
    p.add_argument("--records", type=Path, required=True)
    p.add_argument("--owner-cpu", type=Path, required=True)
    p.add_argument("--lstar", type=Path, required=True)
    p.add_argument("--guards", required=True)
    p.add_argument("--hot", required=True)
    p.add_argument("--boot", type=int, default=1000)
    p.add_argument("--seed", type=int, default=20260928)
    p.add_argument("--jobs", type=int, default=8)
    p.add_argument("--output", type=Path)
    args = p.parse_args(argv)
    lstar = set(json.loads(args.lstar.read_text()))
    guards = set(args.guards.split(","))
    fx = json.loads(args.fixture.read_text())
    strata = fx["sampling"]["strata"]
    cuts = fx["sampling"]["tercile_cut_ids"]
    cpu = defaultdict(list)
    meta = {}
    for path in args.natives:
        for line in open(path):
            r = json.loads(line)
            if r.get("error") or r["phase"] != "Apply":
                continue
            cpu[r["id"]].append(r["cpu_ns"] * 1e-9)
            meta[r["id"]] = r
    rec = recorded(args.records, list(meta), args.jobs)
    rows = []  # (stratum, group, tercile, w, cpu_s, ops, rec_s)
    mismatched = 0
    for i, r in meta.items():
        c = rec.get(i)
        if c is None or c["owner"] != r["owner"] or c["ops"] != r["stats"]["native_operations"]:
            mismatched += 1
            continue
        st = r["stratum"]
        w = strata[st]["weight"]
        rows.append((st, group_of(r["owner"], lstar, guards, args.hot), tercile(i, cuts), w,
                     sum(cpu[i]) / len(cpu[i]), c["ops"], c["seconds"]))

    def calibrate(rs):
        out = {}
        for scope in ("all", "old", "mid", "young"):
            sel = [x for x in rs if scope == "all" or x[2] == scope]
            out[scope] = {g: ratios([(x[3], x[4], x[5], x[6]) for x in sel if x[1] == g])
                          for g in ("L", "Ghot", "G", "U")}
        return out

    cal = calibrate(rows)
    oc = json.loads(args.owner_cpu.read_text())

    def window(by_owner):
        win = defaultdict(lambda: {"natives": 0, "seconds": 0.0, "ops": 0})
        for o, d in by_owner.items():
            a = d.get("Apply")
            if not a:
                continue
            g = group_of(o, lstar, guards, args.hot)
            win[g]["natives"] += a["natives"]
            win[g]["seconds"] += a["seconds"]
            win[g]["ops"] += a["ops"]
        return dict(win)

    windows = {"all_files": (window(oc["by_owner"]), None)}
    # per-file by_owner is not stored; rebuild per-file group totals from the class split is not enough
    # (Ghot vs G), so owner_cpu.py per_file top lists are insufficient: use per_file_by_owner if present.
    for f, d in oc.get("per_file", {}).items():
        bo = d.get("by_owner")
        if bo is None:
            continue
        lo, hi = d["id_range"]
        scope = tercile(lo, cuts) if tercile(lo, cuts) == tercile(hi, cuts) else None
        windows[Path(f).name] = (window(bo), scope)

    rng = random.Random(args.seed)
    by_stratum = defaultdict(list)
    for x in rows:
        by_stratum[x[0]].append(x)
    boots = []
    for _ in range(args.boot):
        rs = []
        for st, xs in by_stratum.items():
            rs.extend(xs[rng.randrange(len(xs))] for _ in xs)
        boots.append(calibrate(rs))

    def ci(vals):
        v = sorted(x for x in vals if x is not None)
        if not v:
            return None
        return [v[int(0.025 * (len(v) - 1))], v[int(0.5 * (len(v) - 1))], v[int(0.975 * (len(v) - 1))]]

    result = {}
    for name, (win, scope) in windows.items():
        entry = {"groups": win, "calibration_scope": {}}
        for sc in ["all"] + ([scope] if scope else []):
            est = shares(cal[sc], win)
            bs = [shares(b[sc], win) for b in boots]
            entry["calibration_scope"][sc] = {
                how: (None if est[how] is None else {
                    **est[how],
                    "L_share_ci95": ci([b[how]["shares"]["L"] if b[how] else None for b in bs]),
                    "Ghot_share_ci95": ci([b[how]["shares"]["Ghot"] if b[how] else None for b in bs])})
                for how in est}
        tot_s = sum(v["seconds"] for v in win.values())
        tot_o = sum(v["ops"] for v in win.values())
        tot_n = sum(v["natives"] for v in win.values())
        entry["raw_shares"] = {g: {"seconds": v["seconds"] / tot_s, "ops": v["ops"] / tot_o,
                                   "natives": v["natives"] / tot_n} for g, v in win.items()}
        result[name] = entry
    out = {"schema": "rustred.inputs.k1-calibration.v1",
           "natives_files": [str(x) for x in args.natives], "fixture": str(args.fixture),
           "records": str(args.records), "owner_cpu": str(args.owner_cpu),
           "sample_apply_natives": len(meta), "joined": len(rows), "mismatched": mismatched,
           "tercile_cut_ids": cuts, "calibration": cal,
           "calibration_ci95": {sc: {g: {k: ci([b[sc][g][k] for b in boots])
                                         for k in ("k1_per_op_us", "contention", "k1_per_native_ms")}
                                     for g in ("L", "Ghot", "G", "U")}
                                for sc in ("all", "old", "mid", "young")},
           "windows": result, "boot": args.boot, "seed": args.seed}
    text = json.dumps(out, indent=1, sort_keys=True)
    if args.output:
        args.output.write_text(text + "\n")
    brief = {"joined": len(rows), "mismatched": mismatched,
             "calibration_all": cal["all"],
             "L_share": {n: {sc: {how: (None if v is None else
                                        [round(v["shares"]["L"], 4), [round(x, 4) for x in v["L_share_ci95"]]])
                                  for how, v in e.items()}
                             for sc, e in w["calibration_scope"].items()}
                         for n, w in result.items()}}
    print(json.dumps(brief, indent=1, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
