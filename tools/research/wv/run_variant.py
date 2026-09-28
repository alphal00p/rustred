#!/usr/bin/env python
"""Run one four-loop control with a substituted queries file (W0.11 D1(b)).

Reuses a historical command line verbatim (as TMP/fable51-controls/run_control.py
does), substituting only the executable, the queries (+ --max-queries /
--max-query-bytes), the output paths and optionally the policy / workers.
Writes OUT/{command.json,result.json,events.jsonl,checkpoint/,stderr,stdout,
metrics.json}. metrics.json records exit code, whole-command wall, peak RSS,
child CPU (rusage), and the foreign load on the run CPU set
(/proc/stat busy jiffies of those CPUs minus the child's own CPU time).

Usage: run_variant.py --family fg|bmw|h|x|four-all --queries Q.json --out DIR
                      [--binary BIN] [--cpus 72-79] [--policy ordered] [--workers 6]
                      [--timeout-seconds S]
"""
import argparse
import json
import os
import re
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path("/common/dev/rustred")
BASE = ROOT / "TMP/four-loop-saved-descendants.VaNmUN"
COMMANDS = {
    "fg": BASE / "fg/command-rank12orthant.json",
    "bmw": BASE / "bmw/command-upstream-a19.json",
    "h": BASE / "h/command-rank12orthant.json",
    "x": BASE / "x/command-rank12orthant.json",
    "four-all": ROOT / "TMP/c4l-build.wBU9zC/commands/command-four-all.json",
}
BINARY = ROOT / "TMP/fable51-controls/bin/rustred-4a17f9c7"
ENV_ONE = {"RAYON_NUM_THREADS": "1", "OMP_NUM_THREADS": "1", "OMP_THREAD_LIMIT": "1",
           "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1", "BLIS_NUM_THREADS": "1",
           "SYMBOLICA_HIDE_BANNER": "1"}


def cpu_list(spec):
    out = []
    for part in spec.split(","):
        if "-" in part:
            a, b = part.split("-")
            out.extend(range(int(a), int(b) + 1))
        else:
            out.append(int(part))
    return out


def busy_jiffies(cpus):
    busy = 0
    with open("/proc/stat") as f:
        for line in f:
            m = re.match(r"cpu(\d+) (.*)", line)
            if m and int(m.group(1)) in cpus:
                v = [int(x) for x in m.group(2).split()]
                busy += sum(v) - v[3] - v[4]  # minus idle, iowait
    return busy


def peak_rss(pid, stop, holder):
    peak = 0
    while not stop.is_set():
        try:
            # the native is a grandchild (nix develop); scan the session
            total = 0
            for p in Path("/proc").iterdir():
                if not p.name.isdigit():
                    continue
                try:
                    stat = (p / "stat").read_text()
                    fields = stat[stat.rindex(")") + 2:].split()
                    if int(fields[3]) != pid:  # session id
                        continue
                    for line in (p / "status").read_text().splitlines():
                        if line.startswith("VmRSS:"):
                            total = max(total, int(line.split()[1]) * 1024)
                except (OSError, ValueError):
                    pass
            peak = max(peak, total)
        except OSError:
            pass
        time.sleep(0.25)
    holder["peak_rss_bytes"] = peak


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--family", required=True, choices=sorted(COMMANDS))
    p.add_argument("--queries", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--binary", default=str(BINARY))
    p.add_argument("--cpus", default="72-79")
    p.add_argument("--policy")
    p.add_argument("--workers", type=int)
    p.add_argument("--timeout-seconds", type=int)
    a = p.parse_args()
    out = Path(a.out)
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    argv = json.load(open(COMMANDS[a.family]))
    argv[0] = a.binary
    qtext = Path(a.queries).read_bytes()
    nq = len(json.loads(qtext)["queries"])

    def set_opt(name, value):
        if name in argv:
            argv[argv.index(name) + 1] = str(value)
        else:
            argv.extend([name, str(value)])
    set_opt("--queries", Path(a.queries).resolve())
    set_opt("--max-queries", nq)
    set_opt("--max-query-bytes", max(len(qtext), 1))
    set_opt("--output", out / "result.json")
    set_opt("--events", out / "events.jsonl")
    set_opt("--stop-file", out / "stop-request.json")
    set_opt("--checkpoint", out / "checkpoint")
    if a.policy:
        set_opt("--publication-policy", a.policy)
    if a.workers:
        set_opt("--workers", a.workers)
    json.dump(argv, open(out / "command.json", "w"), indent=1)
    env = dict(os.environ)
    env.update(ENV_ONE)
    env["TMPDIR"] = str(ROOT / "TMP")
    cpus = cpu_list(a.cpus)
    b0, t0 = busy_jiffies(cpus), time.time()
    proc = subprocess.Popen(["nice", "-n", "5", "nix", "develop", str(ROOT), "--command"] + argv,
                            cwd=ROOT, env=env, stdout=open(out / "stdout", "w"),
                            stderr=open(out / "stderr", "w"),
                            preexec_fn=lambda: os.sched_setaffinity(0, cpus), start_new_session=True)
    stop, holder = threading.Event(), {}
    watcher = threading.Thread(target=peak_rss, args=(proc.pid, stop, holder))
    watcher.start()
    timed_out = False
    deadline = t0 + a.timeout_seconds if a.timeout_seconds else None
    while True:
        pid, status, ru = os.wait4(proc.pid, os.WNOHANG)
        if pid:
            break
        if deadline and time.time() > deadline and not timed_out:
            timed_out = True
            with open(out / "stop-request.json", "x") as f:
                json.dump({"reason": "wv_timeout", "unix_time": time.time(),
                           "family_closure_claim": False}, f)
        time.sleep(0.2)
    wall = time.time() - t0
    b1 = busy_jiffies(cpus)
    stop.set()
    watcher.join()
    code = os.waitstatus_to_exitcode(status)
    hz = os.sysconf("SC_CLK_TCK")
    own = ru.ru_utime + ru.ru_stime
    busy = (b1 - b0) / hz
    foreign = max(0.0, busy - own)
    metrics = {"family": a.family, "queries": str(Path(a.queries).resolve()), "query_count": nq,
               "binary": a.binary, "exit_code": code, "whole_command_seconds": round(wall, 3),
               "cpus": a.cpus, "child_cpu_seconds": round(own, 3),
               "cpu_set_busy_seconds": round(busy, 3),
               "foreign_load_share": round(foreign / (wall * len(cpus)), 4),
               "cooperative_stop_requested": timed_out}
    metrics.update(holder)
    res = out / "result.json"
    if res.exists():
        r = json.load(open(res))
        for k in ("traversal_seconds", "prepared_seconds", "native_processed_nodes", "scheduled_nodes",
                  "completed_nodes", "queued_nodes", "frontiers", "max_scheduled_finite_rank",
                  "containment_checks", "pre_admitted_orthant_hits", "full_orthant_hits",
                  "successors", "status", "recursive_worklist_exhausted"):
            if k in r:
                metrics[k] = r[k] if not isinstance(r[k], list) else len(r[k])
        dc = r.get("descendant_closure") or {}
        for k in ("initial_closed", "initial_total", "total_closed", "total_domains", "unresolved_domains",
                  "dependency_edges"):
            if k in dc:
                metrics["closure_" + k] = dc[k]
    json.dump(metrics, open(out / "metrics.json", "w"), indent=1)
    print(json.dumps(metrics))


if __name__ == "__main__":
    main()
