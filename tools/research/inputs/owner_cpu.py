#!/usr/bin/env python3
"""Native inspection work per owner and owner class from CP5 record sidecars.

Streams every records-*.jsonl of the given checkpoint directories (or explicit
.jsonl files) in parallel byte ranges; for each record of kind
native_inspection it adds, per (file, owner, phase): one native, `seconds`,
stats.native_operations, stats.term_visits and accepted_events, and tracks the
id range of the file. Owner classes: L (in --lstar), G (in --guards), U (other
owners of --owners); Route records are keyed by the owner field of the record.
Optionally --envelope ENVELOPE.tsv (cp5hop census) adds the per-owner Apply
domain count (cumulative, whole checkpoint).

Units. `seconds` is the per-inspection wall time the engine records (inspector
wall at the run's thread count, INCLUDING contention: HANDOFF 0.1 item 2 excludes
it as a work measure). native_operations, term_visits and accepted_events are
schedule- and contention-free counts. A K=1 work estimate needs a per-class K=1
cost calibration (see k1_calibration.py); this tool only reports the counts.

Windows. A CP5 checkpoint directory holds one records file per generation
append (records-<gen>.jsonl; the first file may hold several generations). The
"all" totals are cumulative over the files given; "per_file" gives each append
(generation window) separately.

Output JSON: per owner {phase: {natives, seconds, ops, term_visits, accepted}},
class totals and shares of every Apply metric (all files and per file), and the
top owners by Apply seconds and by Apply ops.

Usage: owner_cpu.py PATH [PATH ...] --lstar L.json --guards MASKS --owners OWNERS.txt
                    [--envelope ENV.tsv] [--jobs 8] [--output OUT.json]
"""
import argparse
import csv
import json
from collections import defaultdict
from multiprocessing import Pool
from pathlib import Path

METRICS = ("natives", "seconds", "ops", "term_visits", "accepted")


def chunk(args):
    path, start, end = args
    acc = defaultdict(lambda: [0, 0.0, 0, 0, 0])
    lo, hi = None, None
    non_native = 0
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
                non_native += 1
                continue
            r = json.loads(line)
            s = r.get("stats") or {}
            a = acc[(r["owner"], r["phase"])]
            a[0] += 1
            a[1] += float(r.get("seconds") or 0.0)
            a[2] += int(s.get("native_operations") or 0)
            a[3] += int(s.get("term_visits") or 0)
            a[4] += int(r.get("accepted_events") or 0)
            i = r.get("id")
            if i is not None:
                lo = i if lo is None or i < lo else lo
                hi = i if hi is None or i > hi else hi
    return path, {f"{o}|{p}": v for (o, p), v in acc.items()}, non_native, lo, hi


def summarize(per, klass, env):
    """per: {"owner|phase": [natives, seconds, ops, term_visits, accepted]} -> classes, totals, shares."""
    by_owner = defaultdict(dict)
    for k, v in per.items():
        o, ph = k.split("|")
        by_owner[o][ph] = dict(zip(METRICS, [v[0], round(v[1], 3), v[2], v[3], v[4]]))
    classes = defaultdict(lambda: defaultdict(float))
    for o, d in by_owner.items():
        c = klass.get(o, "non-owner")
        for ph, v in d.items():
            for m in METRICS:
                classes[c][f"{ph}_{m}"] += v[m]
    for o, n in env.items():
        classes[klass.get(o, "non-owner")]["Apply_domains"] += n
    tot = defaultdict(float)
    for c in classes.values():
        for k, v in c.items():
            tot[k] += v
    shares = {c: {k: (v / tot[k] if tot[k] else None) for k, v in d.items()} for c, d in classes.items()}
    return by_owner, {c: dict(d) for c, d in classes.items()}, dict(tot), shares


def top(by_owner, klass, env, key, tot):
    rows = sorted(((d.get("Apply", {}).get(key, 0), o) for o, d in by_owner.items()), reverse=True)[:10]
    return [{"owner": o, "class": klass.get(o), f"apply_{key}": v,
             "share": v / tot if tot else None,
             "apply_natives": by_owner[o].get("Apply", {}).get("natives"),
             "apply_seconds": by_owner[o].get("Apply", {}).get("seconds"),
             "apply_ops": by_owner[o].get("Apply", {}).get("ops"),
             "apply_domains": env.get(o)} for v, o in rows]


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("paths", nargs="+", type=Path)
    p.add_argument("--lstar", type=Path, required=True)
    p.add_argument("--guards", required=True)
    p.add_argument("--owners", type=Path, required=True)
    p.add_argument("--envelope", type=Path)
    p.add_argument("--jobs", type=int, default=8)
    p.add_argument("--chunk-bytes", type=int, default=256 << 20)
    p.add_argument("--output", type=Path)
    args = p.parse_args(argv)
    files = []
    for path in args.paths:
        files.extend(sorted(path.glob("records-*.jsonl")) if path.is_dir() else [path])
    tasks = []
    for f in files:
        size = f.stat().st_size
        tasks.extend((str(f), s, min(size, s + args.chunk_bytes)) for s in range(0, size, args.chunk_bytes))
    per_file = {str(f): defaultdict(lambda: [0, 0.0, 0, 0, 0]) for f in files}
    ids = {str(f): [None, None] for f in files}
    non_native = 0
    with Pool(args.jobs) as pool:
        for path, acc, nn, lo, hi in pool.imap_unordered(chunk, tasks):
            non_native += nn
            for k, v in acc.items():
                t = per_file[path][k]
                for j in range(5):
                    t[j] += v[j]
            r = ids[path]
            if lo is not None:
                r[0] = lo if r[0] is None or lo < r[0] else r[0]
                r[1] = hi if r[1] is None or hi > r[1] else r[1]
    per = defaultdict(lambda: [0, 0.0, 0, 0, 0])
    for acc in per_file.values():
        for k, v in acc.items():
            for j in range(5):
                per[k][j] += v[j]
    lstar = set(json.loads(args.lstar.read_text()))
    guards = set(args.guards.split(","))
    owners = [line.strip() for line in open(args.owners) if line.strip()]
    klass = {o: ("L" if o in lstar else "G" if o in guards else "U") for o in owners}
    env = {}
    if args.envelope:
        env = {r["mask"]: int(r["apply_domains"]) for r in csv.DictReader(open(args.envelope), delimiter="\t")}
    by_owner, classes, tot, shares = summarize(per, klass, env)
    files_out = {}
    for f, acc in per_file.items():
        bo, cl, tt, sh = summarize(acc, klass, {})
        files_out[f] = {"id_range": ids[f], "classes": cl, "totals": tt, "shares": sh, "by_owner": bo,
                        "top_apply_seconds": top(bo, klass, {}, "seconds", tt.get("Apply_seconds", 0)),
                        "top_apply_ops": top(bo, klass, {}, "ops", tt.get("Apply_ops", 0))}
    out = {"files": [str(f) for f in files], "non_native_records": non_native,
           "units": {"seconds": "per-inspection inspector wall incl. contention (not a work unit)",
                     "ops": "stats.native_operations", "term_visits": "stats.term_visits",
                     "accepted": "record accepted_events"},
           "classes": classes, "totals": tot, "shares": shares,
           "top_apply_seconds": top(by_owner, klass, env, "seconds", tot.get("Apply_seconds", 0)),
           "top_apply_ops": top(by_owner, klass, env, "ops", tot.get("Apply_ops", 0)),
           "per_file": files_out, "by_owner": by_owner}
    text = json.dumps(out, indent=1, sort_keys=True)
    if args.output:
        args.output.write_text(text + "\n")
    brief = {"non_native_records": non_native, "totals": tot,
             "shares_apply": {c: {k: v for k, v in d.items() if k.startswith("Apply_")} for c, d in shares.items()},
             "per_file_L_apply_shares": {Path(f).name: {"id_range": d["id_range"],
                                                        **{k: v for k, v in d["shares"].get("L", {}).items()
                                                           if k.startswith("Apply_")}}
                                         for f, d in files_out.items()}}
    print(json.dumps(brief, indent=1, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
