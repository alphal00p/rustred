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
              [--manifest SEL.json] [--workers N] [--policy ready|ordered]
              [--binary BIN] [--cp5hop BIN] [--owners-txt OWNERS]
              [--perf-stat EVENTS]
With --perf-stat the walk runs under `perf stat -x, -e EVENTS` (user-mode
counters of the walk process and its threads; perfstat.csv) and metrics.json
gets the counter values and instructions per native; the summed scheduler run
delay of the walk's threads (/proc/PID/task/*/schedstat, last value seen per
thread, polled every 0.5 s) is recorded in either case.
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
    # C-5F: five-loop 1,324-tuple finite control (hot owner), W50 historically
    "five-finite": ROOT / "TMP/ready-five-loop-finite-w50.a6ABXd/ready-first/command.json",
}
PERF = "/nix/store/gyp2si1k1w7jhw8z4xx1bwr2m0pr5445-perf-linux-7.2/bin/perf"
RETIRED = ROOT / "TMP/retired-campaigns-20260925.UtI4ay"
ENV_ONE = {k: "1" for k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                            "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")}
KEYS = ("traversal_seconds", "prepared_seconds", "elapsed_seconds", "completed_nodes", "scheduled_nodes",
        "queued_nodes", "containment_checks", "successors", "events", "max_scheduled_finite_rank",
        "native_processed_nodes", "frontiers")


def result_metrics(path):
    """Top-level counters of a (possibly GB-sized) result.json: head and tail only."""
    size = path.stat().st_size
    with open(path, "rb") as f:
        head = f.read(min(size, 4_000_000)).decode("utf-8", "replace")
        f.seek(max(0, size - 4_000_000))
        tail = f.read().decode("utf-8", "replace")
    out = {}
    for key in KEYS + ("initial_closed", "initial_total"):
        for blob in (tail, head):
            # top-level keys are indented by two spaces in the pretty-printed document
            m = re.search(r'\n  "%s": ([0-9.]+|null)' % key, blob) or re.search(r'"%s": ([0-9.]+|null)' % key, blob)
            if m:
                out[key] = m.group(1)
                break
    return out


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def cpu_busy(cpus):
    """Summed busy and total jiffies of the given CPUs from /proc/stat."""
    busy = total = 0
    for line in open("/proc/stat"):
        name, *fields = line.split()
        if name.startswith("cpu") and name[3:].isdigit() and int(name[3:]) in cpus:
            values = list(map(int, fields))
            total += sum(values[:8])
            busy += sum(values[:8]) - values[3] - values[4]
    return busy, total


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
    p.add_argument("--manifest", type=Path, help="replace the selection manifest (owner base unchanged)")
    p.add_argument("--workers", type=int, help="replace --workers")
    p.add_argument("--policy", choices=("ready", "ordered"), help="replace --publication-policy")
    p.add_argument("--out-root", type=Path, default=ROOT / "TMP/w0/inputs/four/runs")
    p.add_argument("--binary", type=Path, default=ROOT / "TMP/fable51-controls/bin/rustred-4a17f9c7")
    p.add_argument("--cp5hop", type=Path)
    p.add_argument("--owners-txt", type=Path)
    p.add_argument("--max-seconds", type=float, default=600.0,
                   help="write the stop file after this many seconds (cooperative stop, checkpoint saved)")
    p.add_argument("--perf-stat", metavar="EVENTS",
                   help="run the walk under perf stat with these events, e.g. instructions:u,cycles:u")
    args = p.parse_args(argv)
    out = args.out_root / args.label / args.family
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
    for i, a in enumerate(argv_):
        if "/common/dev/rustred/campaigns/five-loop-saved-coarse-cover/inputs" in a:
            argv_[i] = a.replace("/common/dev/rustred/campaigns/five-loop-saved-coarse-cover/inputs",
                                 str(RETIRED / "five-loop-saved-coarse-cover/inputs"))
    if args.manifest:
        set_opt("--manifest", args.manifest)
    if args.workers:
        set_opt("--workers", args.workers)
    if args.policy:
        set_opt("--publication-policy", args.policy)
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
    busy0, total0 = cpu_busy(set(cpus))
    start = time.time()
    with open(out / "stdout", "wb") as so, open(out / "stderr", "wb") as se:
        prefix = ([PERF, "stat", "-x,", "-o", str(out / "perfstat.csv"), "-e", args.perf_stat, "--"]
                  if args.perf_stat else [])
        proc = subprocess.Popen(["nice", "-n", "5"] + prefix + argv_, cwd=ROOT, env=env, stdout=so, stderr=se,
                                preexec_fn=lambda: os.sched_setaffinity(0, cpus))
        stop = threading.Event()

        capped = {"at": None}
        delay = {}

        def walk_pid():
            if not args.perf_stat:
                return proc.pid
            try:
                kids = open(f"/proc/{proc.pid}/task/{proc.pid}/children").read().split()
                return int(kids[0]) if kids else None
            except OSError:
                return None

        def poll():
            while not stop.is_set():
                if capped["at"] is None and time.time() - start > args.max_seconds:
                    (out / "stop-request.json").write_text('{"reason":"run_four time cap"}\n')
                    capped["at"] = round(time.time() - start, 1)
                pid = walk_pid()
                if pid is None:
                    time.sleep(0.5)
                    continue
                try:
                    for tid in os.listdir(f"/proc/{pid}/task"):
                        try:
                            delay[tid] = int(open(f"/proc/{pid}/task/{tid}/schedstat").read().split()[1])
                        except (OSError, IndexError, ValueError):
                            pass
                except OSError:
                    pass
                try:
                    for line in open(f"/proc/{pid}/status"):
                        if line.startswith("VmRSS:"):
                            peak["rss"] = max(peak["rss"], int(line.split()[1]) * 1024)
                except OSError:
                    break
                time.sleep(0.5)
        watcher = threading.Thread(target=poll)
        watcher.start()
        _, status, usage = os.wait4(proc.pid, 0)
        code = os.waitstatus_to_exitcode(status)
        proc.returncode = code
        stop.set()
        watcher.join()
    wall = time.time() - start
    busy1, total1 = cpu_busy(set(cpus))
    hz = os.sysconf("SC_CLK_TCK")
    own = usage.ru_utime + usage.ru_stime
    busy_seconds = (busy1 - busy0) / hz
    metrics_load = {"own_cpu_seconds": round(own, 2), "cpuset_busy_seconds": round(busy_seconds, 2),
                    "foreign_cpu_seconds": round(max(0.0, busy_seconds - own), 2),
                    "foreign_share_of_cpuset": round(max(0.0, busy_seconds - own) / (len(cpus) * wall), 4) if wall else None}
    metrics = {"family": args.family, "label": args.label, "binary": str(args.binary),
               "binary_sha256": sha256(args.binary), "queries": str(queries), "queries_sha256": sha256(queries),
               "manifest": argv_[argv_.index("--manifest") + 1],
               "manifest_sha256": sha256(argv_[argv_.index("--manifest") + 1]),
               "exit_code": code, "whole_command_seconds": round(wall, 3), "cpus": args.cpus,
               "peak_rss_bytes": peak["rss"], "time_cap_seconds": args.max_seconds,
               "stop_requested_at_seconds": capped["at"], **metrics_load,
               "run_delay_seconds": round(sum(delay.values()) / 1e9, 2), "run_delay_threads": len(delay)}
    if (out / "result.json").exists():
        metrics.update(result_metrics(out / "result.json"))
    if args.perf_stat and (out / "perfstat.csv").exists():
        counters = {}
        for line in open(out / "perfstat.csv"):
            f = line.strip().split(",")
            if len(f) > 2 and f[2] and not line.startswith("#"):
                try:
                    counters[f[2]] = int(f[0])
                except ValueError:
                    counters[f[2]] = None
        metrics["perf"] = counters
        natives = int(metrics.get("completed_nodes") or 0)
        ins = counters.get("instructions:u") or counters.get("instructions")
        if natives and ins:
            metrics["instructions_per_native"] = round(ins / natives)
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
