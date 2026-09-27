#!/usr/bin/env python3
"""Four-loop saved-owner control with a replaced query document.

Same historical command lines as TMP/fable51-controls/run_control.py (FG, H,
X rank-12 orthant anchors; BMW upstream-A19 anchors: A19/R12/D>=7 saved-cover
envelope roots), with the executable, output paths, CPU set and, optionally,
the query document substituted (--max-queries/--max-query-bytes follow the
new document). After the run, cp5hop reads the final checkpoint: per-owner
Apply envelope and owner-level transitions.

Usage (inside `nix develop`):
  run_four.py --family bmw --label NAME --cpus 100-105 [--queries Q.json]
              [--binary BIN] [--cp5hop BIN] [--owners-txt OWNERS]
Writes TMP/w0/inputs/four/runs/<label>/<family>/.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time

ROOT = Path("/common/dev/rustred")
COMMANDS = {
    "fg": ROOT / "TMP/four-loop-saved-descendants.VaNmUN/fg/command-rank12orthant.json",
    "bmw": ROOT / "TMP/four-loop-saved-descendants.VaNmUN/bmw/command-upstream-a19.json",
    "h": ROOT / "TMP/four-loop-saved-descendants.VaNmUN/h/command-rank12orthant.json",
    "x": ROOT / "TMP/four-loop-saved-descendants.VaNmUN/x/command-rank12orthant.json",
}
ENV_ONE = {k: "1" for k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                            "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")}
KEYS = ("traversal_seconds", "prepared_seconds", "elapsed_seconds", "completed_nodes", "scheduled_nodes",
        "queued_nodes", "containment_checks", "successors", "events", "max_scheduled_finite_rank",
        "native_processed_nodes", "frontiers")


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def cpu_list(spec):
    out = []
    for part in spec.split(","):
        a, _, b = part.partition("-")
        out.extend(range(int(a), int(b or a) + 1))
    return out


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--family", required=True, choices=sorted(COMMANDS))
    p.add_argument("--label", required=True)
    p.add_argument("--cpus", default="100-105")
    p.add_argument("--queries", type=Path)
    p.add_argument("--binary", type=Path, default=ROOT / "TMP/fable51-controls/bin/rustred-4a17f9c7")
    p.add_argument("--cp5hop", type=Path)
    p.add_argument("--owners-txt", type=Path)
    args = p.parse_args(argv)
    out = ROOT / "TMP/w0/inputs/four/runs" / args.label / args.family
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    argv_ = json.load(open(COMMANDS[args.family]))
    argv_[0] = str(args.binary)

    def set_opt(name, value):
        if name in argv_:
            argv_[argv_.index(name) + 1] = str(value)
        else:
            argv_.extend([name, str(value)])
    set_opt("--output", out / "result.json")
    set_opt("--events", out / "events.jsonl")
    set_opt("--stop-file", out / "stop-request.json")
    set_opt("--checkpoint", out / "checkpoint")
    queries = Path(argv_[argv_.index("--queries") + 1])
    if args.queries:
        queries = args.queries
        set_opt("--queries", queries)
        set_opt("--max-queries", len(json.load(open(queries))["queries"]))
        set_opt("--max-query-bytes", queries.stat().st_size)
    (out / "command.json").write_text(json.dumps(argv_, indent=1) + "\n")
    env = dict(os.environ)
    env.update(ENV_ONE)
    env["TMPDIR"] = str(ROOT / "TMP")
    cpus = cpu_list(args.cpus)
    peak = {"rss": 0}
    start = time.time()
    with open(out / "stdout", "wb") as so, open(out / "stderr", "wb") as se:
        proc = subprocess.Popen(["nice", "-n", "5"] + argv_, cwd=ROOT, env=env, stdout=so, stderr=se,
                                preexec_fn=lambda: os.sched_setaffinity(0, cpus))
        stop = threading.Event()

        def poll():
            while not stop.is_set():
                try:
                    for line in open(f"/proc/{proc.pid}/status"):
                        if line.startswith("VmRSS:"):
                            peak["rss"] = max(peak["rss"], int(line.split()[1]) * 1024)
                except OSError:
                    break
                time.sleep(0.5)
        watcher = threading.Thread(target=poll)
        watcher.start()
        code = proc.wait()
        stop.set()
        watcher.join()
    wall = time.time() - start
    metrics = {"family": args.family, "label": args.label, "binary": str(args.binary),
               "binary_sha256": sha256(args.binary), "queries": str(queries), "queries_sha256": sha256(queries),
               "exit_code": code, "whole_command_seconds": round(wall, 3), "cpus": args.cpus,
               "peak_rss_bytes": peak["rss"]}
    if (out / "result.json").exists():
        with open(out / "result.json", "rb") as f:
            head = f.read(4_000_000).decode("utf-8", "replace")
        for key in KEYS:
            m = re.search(r'"%s": ([0-9.]+|null)' % key, head)
            if m:
                metrics[key] = m.group(1)
        m = re.search(r'"initial_closed": ([0-9]+)', head)
        metrics["initial_closed"] = m.group(1) if m else None
        m = re.search(r'"initial_total": ([0-9]+)', head)
        metrics["initial_total"] = m.group(1) if m else None
    latest = out / "checkpoint/latest.json"
    if args.cp5hop and args.owners_txt and latest.exists():
        gen = json.loads(latest.read_text())["generation"]
        subprocess.check_call([str(args.cp5hop), str(out / "checkpoint"), str(gen), str(args.owners_txt),
                               str(out / "census"), "0"])
    (out / "metrics.json").write_text(json.dumps(metrics, indent=1) + "\n")
    print(json.dumps(metrics))
    return 0


if __name__ == "__main__":
    sys.exit(main())
