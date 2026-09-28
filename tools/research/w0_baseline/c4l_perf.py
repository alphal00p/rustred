#!/usr/bin/env python
"""Whole-run fp call-graph profile of one C-4L control, for caller attribution at
four-loop scale.  Replays the native argv of a finished run_control.py run
(command.json) with fresh output paths, under
  perf record -F 499 -e cpu-clock:u --call-graph fp -- taskset -c CPUS <argv>
with the run_control inner-pool environment, then attributes the main thread
(TID == PID, the coordinator) and the worker threads.

Usage: c4l_perf.py --source RUN_DIR --out OUT_DIR --cpus 82-87 --perf PERF
"""
import argparse
import json
import os
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import perf_attribution  # noqa: E402

ENV_ONE = ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT", "OPENBLAS_NUM_THREADS",
           "MKL_NUM_THREADS", "BLIS_NUM_THREADS")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--source", required=True, type=Path)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--cpus", default="82-87")
    p.add_argument("--perf", required=True)
    p.add_argument("--binary", help="override argv[0]")
    p.add_argument("--analyze-only", action="store_true", help="reuse OUT/perf.data")
    args = p.parse_args()
    args.out.mkdir(parents=True, exist_ok=args.analyze_only)
    argv = json.load(open(args.source / "command.json"))
    if args.binary:
        argv[0] = args.binary
    for flag, name in (("--output", "result.json"), ("--events", "events.jsonl"),
                       ("--stop-file", "stop-request.json"), ("--checkpoint", "checkpoint")):
        if flag in argv:
            argv[argv.index(flag) + 1] = str(args.out / name)
    if not args.analyze_only:
        json.dump(argv, open(args.out / "command.json", "w"), indent=1)
    env = dict(os.environ)
    env.update({k: "1" for k in ENV_ONE})
    env["TMPDIR"] = "/common/dev/rustred/TMP"
    data = args.out / "perf.data"
    cmd = [args.perf, "record", "-F", "499", "-e", "cpu-clock:u", "--call-graph", "fp", "-o", str(data),
           "--", "taskset", "-c", args.cpus] + argv
    start = time.time()
    code = None
    if not args.analyze_only:
        code = subprocess.call(cmd, env=env, stdout=open(args.out / "stdout", "w"),
                               stderr=open(args.out / "stderr", "w"), cwd="/common/dev/rustred")
    wall = time.time() - start
    res = subprocess.run([args.perf, "script", "-i", str(data), "-F", "pid,tid"], capture_output=True, text=True)
    counts = Counter(tuple(line.replace("/", " ").split()[:2]) for line in res.stdout.splitlines() if line.strip())
    mains = [(c, pid) for (pid, tid), c in counts.items() if pid == tid]
    main_tid = max(mains)[1] if mains else None
    workers = sorted({tid for (pid, tid) in counts if main_tid and pid == main_tid and tid != main_tid})
    out = {"source": str(args.source), "exit_code": code, "wall_seconds": wall, "main_tid": main_tid,
           "samples_by_thread": {f"{pid}/{tid}": c for (pid, tid), c in counts.most_common()},
           "coordinator": perf_attribution.attribute(args.perf, data, "coordinator", tids=[main_tid]) if main_tid else None,
           "workers": perf_attribution.attribute(args.perf, data, "worker", tids=workers) if workers else None}
    json.dump(out, open(args.out / "attribution.json", "w"), indent=1)
    print(json.dumps({k: out[k] for k in ("exit_code", "wall_seconds", "main_tid")}))
    if out["coordinator"]:
        c = out["coordinator"]
        print(json.dumps({k: c.get(k) for k in ("samples", "named_share", "attributed_share", "bucket_shares")}))


if __name__ == "__main__":
    main()
