#!/usr/bin/env python3
"""Time-boxed fresh walk probe on a candidate query document (legacy engine).

Command shape of the v2 campaign (Ready publication, lookahead 256, route
over-cover, initial D-band reuse, unbounded work, checkpointed), with the
executable, workers and CPU set as arguments. A cooperative stop is requested
at --stop-after seconds so the final checkpoint save fits the one-hour budget.
Every --sample seconds the last heartbeat is appended to timeseries.jsonl
(elapsed, completed/scheduled/queued nodes, frontiers, max rank, closed
roots, RSS). After exit, cp5hop reads the checkpoint: per-owner Apply
envelope and, with --bounds, the Apply domains outside each owner's helper
box (escapes.tsv).

Usage (inside `nix develop`):
  probe.py --queries Q.json --out DIR --cpus 100-117 --workers 24
           --stop-after 3240 --cp5hop BIN [--bounds BOUNDS.tsv]
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
KEYS = ("completed_nodes", "scheduled_nodes", "queued_nodes", "committed_domains", "frontiers",
        "max_scheduled_finite_rank", "successors", "events", "routed_domains", "unbounded_rank_domains",
        "initial_entry_domains_published")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def last_heartbeat(events):
    try:
        with open(events, "rb") as f:
            f.seek(0, 2)
            size = f.tell()
            f.seek(max(0, size - 2_000_000))
            lines = f.read().decode("utf-8", "replace").splitlines()
    except OSError:
        return None
    for line in reversed(lines):
        if '"domain_progress"' not in line:
            continue
        try:
            d = json.loads(line)
        except ValueError:
            continue
        p = d.get("progress") or {}
        if p.get("event") != "domain_progress":
            continue
        dc = p.get("descendant_closure") or {}
        row = {"t": d.get("elapsed_seconds"), "rss": d.get("process_rss_bytes"),
               "roots_closed": dc.get("initial_closed"), "total_closed": dc.get("total_closed"),
               "edges": dc.get("dependency_edges")}
        for k in KEYS:
            row[k] = p.get(k)
        return row
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
    p.add_argument("--bounds", type=Path)
    p.add_argument("--cpus", default="100-117")
    p.add_argument("--workers", default="24")
    p.add_argument("--policy", default="ready")
    p.add_argument("--stop-after", type=float, default=3240.0)
    p.add_argument("--sample", type=float, default=30.0)
    args = p.parse_args(argv)
    out = args.out
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    count = len(json.loads(args.queries.read_bytes())["queries"])
    command = [str(args.binary), "owner-domain-match", "--manifest", str(args.selection),
               "--owner-base", str(args.owner_base), "--output", str(out / "result.json"),
               "--events", str(out / "events.jsonl"), "--stop-file", str(out / "stop-request.json"),
               "--workers", args.workers, "--queries", str(args.queries), "--follow-successors",
               "--max-queries", str(count), "--max-query-bytes", str(args.queries.stat().st_size),
               "--max-guard-univariate-degree", "64", "--bounded-refinement-axes", "finite-axes",
               "--transfer-unreserved-lookahead", "256", "--publication-policy", args.policy,
               "--route-domain-overcover", "--reuse-initial-d-bands", "--checkpoint", str(out / "checkpoint"),
               "--checkpoint-interval-seconds", "14400", "--unbounded-work", "--no-progress"]
    (out / "command.json").write_text(json.dumps(command, indent=1) + "\n")
    env = dict(os.environ)
    env.update(ENV_ONE)
    started = time.time()
    first_frontier = None
    stop_at = None
    series = open(out / "timeseries.jsonl", "w")
    with open(out / "stdout", "wb") as so, open(out / "stderr", "wb") as se:
        proc = subprocess.Popen(["nice", "-n", "5", "taskset", "-c", args.cpus,
                                 "/run/current-system/sw/bin/time", "-v"] + command, stdout=so, stderr=se, env=env)
        next_sample = started + args.sample
        while proc.poll() is None:
            time.sleep(1)
            now = time.time()
            if stop_at is None and now - started >= args.stop_after:
                (out / "stop-request.json").write_text('{"reason":"probe_time_box"}\n')
                stop_at = now - started
            if now >= next_sample:
                next_sample += args.sample
                row = last_heartbeat(out / "events.jsonl")
                if row:
                    row["wall"] = round(now - started, 1)
                    series.write(json.dumps(row) + "\n")
                    series.flush()
                    if first_frontier is None and (row.get("frontiers") or 0) > 0:
                        first_frontier = row["t"]
        code = proc.returncode
    wall = time.time() - started
    row = last_heartbeat(out / "events.jsonl")
    if row:
        row["wall"] = round(wall, 1)
        series.write(json.dumps(row) + "\n")
    series.close()
    receipt = {"queries": str(args.queries), "queries_sha256": sha256(args.queries), "query_count": count,
               "binary": str(args.binary), "binary_sha256": sha256(args.binary), "cpus": args.cpus,
               "workers": args.workers, "policy": args.policy, "exit_code": code, "wall_seconds": round(wall, 1),
               "stop_requested_at_seconds": stop_at, "first_frontier_heartbeat_seconds": first_frontier,
               "last_heartbeat": row}
    latest = out / "checkpoint/latest.json"
    if latest.exists():
        gen = json.loads(latest.read_text())["generation"]
        cmd = [str(args.cp5hop), str(out / "checkpoint"), str(gen), str(args.owners_txt), str(out / "census"), "0"]
        if args.bounds:
            cmd.append(str(args.bounds))
        t0 = time.time()
        subprocess.check_call(cmd)
        receipt["census_seconds"] = round(time.time() - t0, 1)
        receipt["checkpoint_generation"] = gen
    if (out / "result.json").exists():
        with open(out / "result.json", "rb") as f:
            head = f.read(4_000_000).decode("utf-8", "replace")
        for key in ("frontiers", "completed_nodes", "scheduled_nodes", "queued_nodes", "max_scheduled_finite_rank"):
            import re
            m = re.search(r'"%s": ([0-9]+|null)' % key, head)
            receipt["result_" + key] = m.group(1) if m else None
    (out / "receipt.json").write_text(json.dumps(receipt, indent=1, sort_keys=True) + "\n")
    print(json.dumps({k: receipt.get(k) for k in ("exit_code", "wall_seconds", "stop_requested_at_seconds",
                                                  "first_frontier_heartbeat_seconds", "result_frontiers",
                                                  "result_completed_nodes", "result_scheduled_nodes")}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
