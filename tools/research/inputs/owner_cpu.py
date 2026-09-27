#!/usr/bin/env python3
"""Native inspection seconds and counts per owner and owner class from CP5 record sidecars.

Streams every records-*.jsonl of the given checkpoint directories (or explicit
.jsonl files) in parallel byte ranges; for each record of kind
native_inspection it adds `seconds` and one native to (owner, phase). Owner
classes: L (in --lstar), G (in --guards), U (other owners of --owners); Route
records are keyed by the owner field of the record. Optionally
--envelope ENVELOPE.tsv (cp5hop census) adds the per-owner Apply domain count.

Output JSON: per owner {phase: {natives, seconds}}, class totals and shares of
Apply seconds / Apply natives / Apply domains, and the top owners by Apply
seconds. `seconds` is the per-inspection wall time the engine records for the
native inspection (inspector wall, including any contention).

Usage: owner_cpu.py PATH [PATH ...] --lstar L.json --guards MASKS --owners OWNERS.txt
                    [--envelope ENV.tsv] [--jobs 8] [--output OUT.json]
"""
import argparse
import csv
import json
import os
from collections import defaultdict
from multiprocessing import Pool
from pathlib import Path


def chunk(args):
    path, start, end = args
    acc = defaultdict(lambda: [0, 0.0])
    other = defaultdict(int)
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
                other[b"non_native"] += 1
                continue
            r = json.loads(line)
            a = acc[(r["owner"], r["phase"])]
            a[0] += 1
            a[1] += float(r.get("seconds") or 0.0)
    return {f"{o}|{p}": v for (o, p), v in acc.items()}, sum(other.values())


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
    per = defaultdict(lambda: [0, 0.0])
    non_native = 0
    with Pool(args.jobs) as pool:
        for acc, nn in pool.imap_unordered(chunk, tasks):
            non_native += nn
            for k, (n, s) in acc.items():
                per[k][0] += n
                per[k][1] += s
    lstar = set(json.loads(args.lstar.read_text()))
    guards = set(args.guards.split(","))
    owners = [line.strip() for line in open(args.owners) if line.strip()]
    klass = {o: ("L" if o in lstar else "G" if o in guards else "U") for o in owners}
    env = {}
    if args.envelope:
        env = {r["mask"]: int(r["apply_domains"]) for r in csv.DictReader(open(args.envelope), delimiter="\t")}
    by_owner = defaultdict(dict)
    for k, (n, s) in per.items():
        o, ph = k.split("|")
        by_owner[o][ph] = {"natives": n, "seconds": round(s, 3)}
    classes = defaultdict(lambda: defaultdict(float))
    for o, d in by_owner.items():
        c = klass.get(o, "non-owner")
        for ph, v in d.items():
            classes[c][ph + "_natives"] += v["natives"]
            classes[c][ph + "_seconds"] += v["seconds"]
    for o, n in env.items():
        classes[klass.get(o, "non-owner")]["Apply_domains"] += n
    tot = defaultdict(float)
    for c in classes.values():
        for k, v in c.items():
            tot[k] += v
    shares = {c: {k: (v / tot[k] if tot[k] else None) for k, v in d.items()} for c, d in classes.items()}
    top = sorted(((d.get("Apply", {}).get("seconds", 0.0), o) for o, d in by_owner.items()), reverse=True)[:10]
    out = {"files": [str(f) for f in files], "non_native_records": non_native,
           "classes": {c: dict(d) for c, d in classes.items()}, "totals": dict(tot), "shares": shares,
           "top_apply_seconds": [{"owner": o, "class": klass.get(o), "apply_seconds": s,
                                  "share": s / tot["Apply_seconds"] if tot["Apply_seconds"] else None,
                                  "apply_natives": by_owner[o].get("Apply", {}).get("natives"),
                                  "apply_domains": env.get(o)} for s, o in top],
           "by_owner": by_owner}
    text = json.dumps(out, indent=1, sort_keys=True)
    if args.output:
        args.output.write_text(text + "\n")
    brief = {k: out[k] for k in ("non_native_records", "shares", "top_apply_seconds")}
    brief["totals"] = out["totals"]
    print(json.dumps(brief, indent=1, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
