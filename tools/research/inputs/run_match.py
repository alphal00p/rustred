#!/usr/bin/env python3
"""Run the matching-only owner-domain diagnostic on one query document.

Reproduces the ladder-(b) command of TMP/qcd-feynman-d9d10-input.dcgP73/
match-v2/command.json (no --follow-successors, unlimited per-query work,
finite-axes refinement, guard degree 64, 1,000,000 total pieces by default),
with the executable, selection copy and CPU pinning as arguments, then writes
matching-summary.json with examples/python/summarize_owner_domain_match.py.

Usage (inside `nix develop`):
  run_match.py --queries Q.json --out DIR [--binary BIN] [--cpu 100]
               [--max-total-pieces N]
DIR must not exist. Writes command.json, result.json, events.jsonl, stdout,
stderr (GNU time -v), matching-summary.json and receipt.json.
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
U64_MAX = "18446744073709551615"
ENV_ONE = {k: "1" for k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                            "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")}
HERE = Path(__file__).resolve().parent
SUMMARIZER = HERE.parents[2] / "examples/python/summarize_owner_domain_match.py"


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--queries", type=Path, required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    p.add_argument("--selection", type=Path, default=INPUTS / "selection.json")
    p.add_argument("--owner-base", type=Path, default=INPUTS)
    p.add_argument("--cpu", default="100")
    p.add_argument("--max-total-pieces", default="1000000")
    p.add_argument("--nice", default="5")
    args = p.parse_args(argv)
    out = args.out
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    queries = json.loads(args.queries.read_bytes())
    count = len(queries["queries"])
    size = args.queries.stat().st_size
    command = [str(args.binary), "owner-domain-match", "--manifest", str(args.selection),
               "--owner-base", str(args.owner_base), "--queries", str(args.queries),
               "--max-queries", str(count), "--max-query-bytes", str(size),
               "--output", str(out / "result.json"), "--events", str(out / "events.jsonl"),
               "--stop-file", str(out / "stop-request.json"),
               "--max-total-pieces", str(args.max_total_pieces), "--max-guard-univariate-degree", "64",
               "--bounded-refinement-axes", "finite-axes", "--no-progress"]
    for name in ("rules", "terminal-checks", "predicates", "pieces", "cells", "split-operations",
                 "coordinate-cells", "bounded-refinement-cells"):
        command += [f"--max-{name}-per-query", U64_MAX]
    (out / "command.json").write_text(json.dumps(command, indent=1) + "\n")
    env = dict(os.environ)
    env.update(ENV_ONE)
    started = time.time()
    with open(out / "stdout", "wb") as so, open(out / "stderr", "wb") as se:
        code = subprocess.call(["nice", "-n", args.nice, "taskset", "-c", args.cpu,
                                "/run/current-system/sw/bin/time", "-v"] + command,
                               stdout=so, stderr=se, env=env)
    wall = time.time() - started
    summary_code = None
    if (out / "result.json").exists():
        summary_code = subprocess.call([sys.executable, str(SUMMARIZER), "--result", str(out / "result.json"),
                                        "--output", str(out / "matching-summary.json")])
    receipt = {"queries": str(args.queries), "queries_sha256": sha256(args.queries), "query_count": count,
               "binary": str(args.binary), "binary_sha256": sha256(args.binary),
               "selection_sha256": sha256(args.selection), "cpu": args.cpu, "exit_code": code,
               "wall_seconds": round(wall, 3), "summary_exit_code": summary_code}
    if (out / "matching-summary.json").exists():
        s = json.loads((out / "matching-summary.json").read_text())
        receipt["summary"] = {k: s.get(k) for k in ("classification_complete", "all_queries_locally_applicable",
                                                      "completed_queries", "piece_counts", "incomplete_queries",
                                                      "helper_positive_power_owners")}
        receipt["queries_with_unresolved_gap_or_invalid"] = [
            {"id": q["id"], "owner": q["owner"], "counts": q["counts"]} for q in s.get("queries", [])
            if q["counts"]["unresolved"] or q["counts"]["exact_gap"] or q["counts"]["invalid_source_condition"]
            or not q["classification_complete"]]
    (out / "receipt.json").write_text(json.dumps(receipt, indent=1, sort_keys=True) + "\n")
    print(json.dumps({k: receipt[k] for k in ("exit_code", "wall_seconds")} |
                     {"piece_counts": receipt.get("summary", {}).get("piece_counts"),
                      "bad_queries": len(receipt.get("queries_with_unresolved_gap_or_invalid", []))}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
