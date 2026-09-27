#!/usr/bin/env python
"""Interleaved A/B runner for the saved-owner walk controls (Symbolica lane, W0).

Reuses the historical command lines and rewrite rules of
TMP/fable51-controls/run_control.py (imported, not copied), but writes to a
caller-chosen output root and records the child's rusage (user + system CPU
seconds of the whole process tree reaped through wait4).

Each family runs on its own CPU set; within a family the binaries run
sequentially and interleaved (A, B, A, B, ...), so an A/B pair always shares
the same CPUs and similar foreign load. Families run concurrently.

Usage:
  ab_controls.py --out-root DIR --bin A=/path/a --bin B=/path/b \
      --family fg:288-293 --family bmw:294-299 [--repeats 2] [--policy ordered]
      [--workers N] [--socket1-lock]

Writes DIR/<family>/<label>-r<k>/{command.json,result.json,events.jsonl,
stdout,stderr,metrics.json} and DIR/<family>/schedule.json.
"""
import argparse
import importlib.util
import json
import os
import subprocess
import threading
import time
from pathlib import Path

RUN_CONTROL = Path("/common/dev/rustred/TMP/fable51-controls/run_control.py")
_SPEC = importlib.util.spec_from_file_location("run_control", RUN_CONTROL)
RC = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(RC)


def foreign_load(cpus):
    """Return per-CPU (busy, total) jiffies for the given CPUs from /proc/stat."""
    out = {}
    with open("/proc/stat") as f:
        for line in f:
            if not line.startswith("cpu") or line.startswith("cpu "):
                continue
            parts = line.split()
            cpu = int(parts[0][3:])
            if cpu in cpus:
                vals = list(map(int, parts[1:]))
                idle = vals[3] + vals[4]
                out[cpu] = (sum(vals) - idle, sum(vals))
    return out


def run_one(binary, family, out, cpus_spec, policy, workers, nice, extra):
    out.mkdir(parents=True, exist_ok=False)
    argv = json.load(open(RC.COMMANDS[family]))
    argv = RC.rewrite(argv, binary, out, policy, workers, None, extra, family == "five-finite")
    json.dump(argv, open(out / "command.json", "w"), indent=1)
    env = dict(os.environ)
    env.update(RC.ENV_ONE)
    env["TMPDIR"] = str(RC.ROOT / "TMP")
    cpus = RC.cpu_list(cpus_spec)
    before = foreign_load(set(cpus))
    stderr = open(out / "stderr", "w")
    stdout = open(out / "stdout", "w")
    start = time.time()
    proc = subprocess.Popen(
        ["nice", "-n", str(nice), "nix", "develop", "--command"] + argv,
        cwd=RC.ROOT, env=env, stdout=stdout, stderr=stderr,
        preexec_fn=lambda: os.sched_setaffinity(0, cpus))
    stop = threading.Event()
    holder = {}
    watcher = threading.Thread(target=RC.peak_rss, args=(proc.pid, stop, holder))
    watcher.start()
    _, status, usage = os.wait4(proc.pid, 0)
    proc.returncode = os.waitstatus_to_exitcode(status)
    stop.set()
    watcher.join()
    wall = time.time() - start
    after = foreign_load(set(cpus))
    busy = sum(after[c][0] - before[c][0] for c in after)
    total = sum(after[c][1] - before[c][1] for c in after)
    metrics = {
        "family": family, "binary": str(binary), "exit_code": proc.returncode,
        "whole_command_seconds": round(wall, 3), "cpus": cpus_spec, "policy": policy,
        "workers": workers, "extra": extra,
        "rusage_user_seconds": round(usage.ru_utime, 3),
        "rusage_system_seconds": round(usage.ru_stime, 3),
        "rusage_max_rss_kib": usage.ru_maxrss,
        "cpu_set_busy_share": round(busy / total, 4) if total else None,
    }
    metrics.update(holder)
    metrics.update(RC.extract(out / "result.json"))
    json.dump(metrics, open(out / "metrics.json", "w"), indent=1)
    return metrics


def family_worker(args, family, cpus_spec, bins, results):
    schedule = []
    fam_root = Path(args.out_root) / family
    fam_root.mkdir(parents=True, exist_ok=True)
    for k in range(1, args.repeats + 1):
        order = bins if k % 2 == 1 or not args.alternate else list(reversed(bins))
        for label, binary in order:
            out = fam_root / f"{label}-r{k}"
            m = run_one(binary, family, out, cpus_spec, args.policy, args.workers, args.nice,
                        args.extra)
            m["label"] = label
            m["repeat"] = k
            schedule.append({"label": label, "repeat": k, "dir": str(out),
                             "exit_code": m["exit_code"],
                             "whole_command_seconds": m["whole_command_seconds"]})
            print(json.dumps({"family": family, "label": label, "repeat": k,
                              "exit": m["exit_code"], "wall": m["whole_command_seconds"],
                              "user": m["rusage_user_seconds"],
                              "traversal": m.get("traversal_seconds")}), flush=True)
            json.dump(schedule, open(fam_root / "schedule.json", "w"), indent=1)
    results[family] = schedule


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--out-root", required=True)
    p.add_argument("--bin", action="append", required=True, help="LABEL=/path/to/binary")
    p.add_argument("--family", action="append", required=True, help="FAMILY:CPUSPEC")
    p.add_argument("--repeats", type=int, default=2)
    p.add_argument("--alternate", action="store_true",
                   help="reverse the binary order on even repeats (A,B,B,A)")
    p.add_argument("--policy", default="ordered")
    p.add_argument("--workers", type=int)
    p.add_argument("--nice", type=int, default=5)
    p.add_argument("--extra", nargs="*", default=[])
    args = p.parse_args()
    bins = []
    for spec in args.bin:
        label, path = spec.split("=", 1)
        bins.append((label, Path(path)))
    fams = []
    for spec in args.family:
        fam, cpus = spec.split(":", 1)
        if fam not in RC.COMMANDS:
            raise SystemExit(f"unknown family {fam}")
        fams.append((fam, cpus))
    results = {}
    threads = [threading.Thread(target=family_worker, args=(args, fam, cpus, bins, results))
               for fam, cpus in fams]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    print(json.dumps(results, indent=1))


if __name__ == "__main__":
    main()
