#!/usr/bin/env python3
"""One native hop from explicit query boxes, with the production walk engine.

Runs `rustred owner-domain-match --follow-successors` (Ordered publication,
route over-cover, unbounded work, checkpointed) on a query document and writes
the stop file as soon as every initial domain is published, so the checkpoint
holds the complete committed edge set of each initial inspection (plus some
partial later work, which is ignored). Then `cp5hop` lists every edge whose
source is an initial ID with the target (phase, mask).

The one-hop target sectors of a box B are a superset of those of every
sub-box of B: the first applicable rule of a point does not depend on the box,
and a coefficient that is identically zero on a larger sign cell is zero on
each of its sub-cells. Frontier pieces emit no successors, so the result is
only a complete one-hop over-approximation when the initial records carry no
frontier and no error (checked from the records sidecar).

Usage (inside `nix develop`):
  one_hop.py --queries Q.json --out DIR --cpus 100-117 --workers 18
             [--binary BIN] [--cp5hop PATH] [--timeout 3000]
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path("/common/dev/rustred")
DEFAULT_BINARY = ROOT / "TMP/fable51-controls/bin/rustred-4a17f9c7"
INPUTS = ROOT / "TMP/retired-campaigns-20260925.UtI4ay/five-loop-saved/inputs"
ENV_ONE = {k: "1" for k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                            "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")}


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def last_progress(events):
    """Most recent heartbeat progress dict (reads only the file tail)."""
    try:
        with open(events, "rb") as f:
            f.seek(0, 2)
            size = f.tell()
            f.seek(max(0, size - 400_000))
            lines = f.read().decode("utf-8", "replace").splitlines()
    except OSError:
        return None
    for line in reversed(lines):
        if '"initial_entry_domains_published"' not in line:
            continue
        try:
            d = json.loads(line)
        except ValueError:
            continue
        p = d.get("progress") or {}
        if "initial_entry_domains_published" in p:
            return p
    return None


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--queries", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    p.add_argument("--selection", type=Path, default=INPUTS / "selection.json")
    p.add_argument("--owner-base", type=Path, default=INPUTS)
    p.add_argument("--owners-txt", type=Path, default=ROOT / "TMP/w0/inputs/owners.txt")
    p.add_argument("--cp5hop", type=Path, required=True)
    p.add_argument("--cpus", default="100-117")
    p.add_argument("--workers", default="18")
    p.add_argument("--timeout", type=float, default=3000.0)
    args = p.parse_args(argv)
    out = args.out
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    count = len(json.loads(args.queries.read_bytes())["queries"])
    command = [str(args.binary), "owner-domain-match", "--manifest", str(args.selection),
               "--owner-base", str(args.owner_base), "--queries", str(args.queries),
               "--max-queries", str(count), "--max-query-bytes", str(args.queries.stat().st_size),
               "--output", str(out / "result.json"), "--events", str(out / "events.jsonl"),
               "--stop-file", str(out / "stop-request.json"), "--workers", args.workers,
               "--follow-successors", "--max-guard-univariate-degree", "64",
               "--bounded-refinement-axes", "finite-axes", "--publication-policy", "ordered",
               "--route-domain-overcover", "--checkpoint", str(out / "checkpoint"),
               "--checkpoint-interval-seconds", "14400", "--unbounded-work", "--no-progress"]
    (out / "command.json").write_text(json.dumps(command, indent=1) + "\n")
    env = dict(os.environ)
    env.update(ENV_ONE)
    started = time.time()
    stop_at = None
    with open(out / "stdout", "wb") as so, open(out / "stderr", "wb") as se:
        proc = subprocess.Popen(["nice", "-n", "5", "taskset", "-c", args.cpus,
                                 "/run/current-system/sw/bin/time", "-v"] + command, stdout=so, stderr=se, env=env)
        while proc.poll() is None:
            time.sleep(2)
            prog = last_progress(out / "events.jsonl")
            if stop_at is None and prog and prog["initial_entry_domains_published"] >= count:
                (out / "stop-request.json").write_text('{"reason":"one_hop_initial_published"}\n')
                stop_at = time.time() - started
            if stop_at is None and time.time() - started > args.timeout:
                (out / "stop-request.json").write_text('{"reason":"timeout"}\n')
                stop_at = -(time.time() - started)
        code = proc.returncode
    wall = time.time() - started
    latest = json.loads((out / "checkpoint/latest.json").read_text())
    gen = latest["generation"]
    subprocess.check_call([str(args.cp5hop), str(out / "checkpoint"), str(gen), str(args.owners_txt),
                           str(out / "hop"), str(count)])
    # initial records: frontiers / errors
    initial = {}
    for rec_path in sorted((out / "checkpoint").glob("records-*.jsonl")):
        for line in open(rec_path):
            r = json.loads(line)
            if isinstance(r.get("id"), int) and r["id"] < count:
                initial[r["id"]] = {"owner": r.get("owner"), "frontiers": len(r.get("frontiers") or []),
                                    "error": r.get("error"), "seconds": r.get("seconds"),
                                    "finished": r.get("local_inspection_finished")}
    receipt = {"queries": str(args.queries), "queries_sha256": sha256(args.queries), "query_count": count,
               "binary": str(args.binary), "binary_sha256": sha256(args.binary), "cpus": args.cpus,
               "workers": args.workers, "exit_code": code, "wall_seconds": round(wall, 1),
               "stop_requested_at_seconds": stop_at, "checkpoint_generation": gen,
               "initial_records": len(initial),
               "initial_with_frontiers_or_errors": {k: v for k, v in initial.items() if v["frontiers"] or v["error"]},
               "initial_record_seconds": {k: v["seconds"] for k, v in sorted(initial.items())}}
    (out / "receipt.json").write_text(json.dumps(receipt, indent=1, sort_keys=True) + "\n")
    print(json.dumps({k: receipt[k] for k in ("exit_code", "wall_seconds", "stop_requested_at_seconds",
                                              "initial_records")} |
                     {"bad_initial": len(receipt["initial_with_frontiers_or_errors"])}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
