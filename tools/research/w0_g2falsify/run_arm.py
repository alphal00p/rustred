#!/usr/bin/env python
"""W0 G2' falsifier arm runner (throwaway; copied from the W0.9 runner on branch
fable_5_1-v3-widen and extended with the load recorder). One walk per call.

Reuses the historical control command lines of TMP/fable51-controls/run_control.py
(four-loop FG/BMW/H/X at the A<=19, R<=12, D>=7 saved-cover envelope and the
five-loop 1,324-tuple finite control) plus the hot-owner family (C-HOT inputs
with a replaceable queries file for C-HOT-sub). Only the executable, output
paths, policy, workers, CPU set and environment are substituted.

Usage:
  run_arm.py --binary BIN --family fg|bmw|h|x|five-finite|hot --label NAME
             --cpus 264-287 [--workers N] [--policy ordered|ready]
             [--queries FILE (hot only)] [--env KEY=VALUE ...]
             [--time-limit SECONDS] [--grace SECONDS] [--out-root DIR]
Writes <out-root>/<label>/<family>/{command.json,result.json,events.jsonl,
stderr,stdout,metrics.json}. A time limit writes the stop file (cooperative
pause) and kills the process tree after the grace period.
Recorder (plan section 7 / audit directive 0.1.7): every 10 s the busy time of
the run CPUs (/proc/stat) minus the run's own process-tree CPU time gives the
foreign busy CPUs; per-thread schedstat run delay of the walk process is
summed at the end (max seen per thread). With --perf the walk binary runs
under `perf stat -e instructions:u,cycles:u,task-clock` (user-space counts of
the whole process, every thread; perf_event_paranoid 2 allows :u events), and
metrics.json gets a "perf" block (instructions per native: see gate.py).
"""
import argparse
import json
import os
import re
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path("/common/dev/rustred")
sys.path.insert(0, str(ROOT / "TMP/fable51-controls"))
import run_control  # noqa: E402  (historical command lines and rewrite())

PERF = "/nix/store/7ccpnz8xkn5qsyw8nkz998vjmb6jpl49-perf-linux-6.19.6/bin/perf"
HOT_COMMAND = ROOT / "TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run/request.json"


def hot_argv(queries):
    argv = list(json.load(open(HOT_COMMAND))["command"])
    q = json.load(open(queries))
    size = os.path.getsize(queries)
    argv[argv.index("--queries") + 1] = str(queries)
    argv[argv.index("--max-queries") + 1] = str(len(q["queries"]))
    argv[argv.index("--max-query-bytes") + 1] = str(size)
    return argv


def descendants(pid):
    kids = {}
    for d in os.listdir("/proc"):
        if not d.isdigit():
            continue
        try:
            with open(f"/proc/{d}/stat") as f:
                ppid = int(f.read().rsplit(")", 1)[1].split()[1])
            kids.setdefault(ppid, []).append(int(d))
        except (OSError, ValueError, IndexError):
            pass
    out, stack = [], [pid]
    while stack:
        p = stack.pop()
        out.append(p)
        stack.extend(kids.get(p, []))
    return out


def rss_of(pids):
    total = 0
    for p in pids:
        try:
            with open(f"/proc/{p}/status") as f:
                for line in f:
                    if line.startswith("VmRSS:"):
                        total += int(line.split()[1]) * 1024
                        break
        except OSError:
            pass
    return total


def watch(pid, stop, holder):
    peak = 0
    while not stop.is_set():
        peak = max(peak, rss_of(descendants(pid)))
        time.sleep(1.0)
    holder["peak_tree_rss_bytes"] = peak


def extract(path):
    m = run_control.extract(path)
    if not path.exists():
        return m
    size = path.stat().st_size
    with open(path, "rb") as f:
        head = f.read(min(size, 400_000)).decode("utf-8", "replace")
        f.seek(max(0, size - 400_000))
        tail = f.read().decode("utf-8", "replace")
    for key in ("initial_closed", "initial_total", "total_closed", "total_domains", "unresolved_domains",
                "dependency_edges", "status", "all_scheduled_domains_resolved", "pending_descendant_domains",
                "transferred_obligations", "native_discharged", "delegated_publications", "job_local_reuse_hits",
                "pre_admitted_orthant_hits", "exact_domain_hits"):
        for blob in ((tail, head) if key == "status" else (head, tail)):
            mm = re.search((r'\n  "%s": ' if key == "status" else r'"%s": ') % key + r'("[^"]*"|[0-9.]+|null|true|false)', blob)
            if mm:
                m[key] = json.loads(mm.group(1))
                break
    mm = re.search(r'"slot_busy_seconds": \[([^\]]*)\]', tail)
    if mm:
        vals = [float(x) for x in mm.group(1).split(",") if x.strip()]
        m["slot_busy_seconds_sum"] = sum(vals)
        m["inspection_slots"] = len(vals)
    mm = re.search(r'"slot_backpressure_seconds": \[([^\]]*)\]', tail)
    if mm:
        m["slot_backpressure_seconds_sum"] = sum(float(x) for x in mm.group(1).split(",") if x.strip())
    mm = re.search(r'\n  "w0_g2_donly": (\{.*?\n  \})', tail, re.S)
    if mm:
        try:
            m["w0_g2_donly"] = json.loads(mm.group(1))
        except json.JSONDecodeError:
            m["w0_g2_donly"] = mm.group(1)
    mm = re.search(r'\n  "descendant_closure": (\{.*?\n  \})', head, re.S)
    if mm:
        try:
            dc = json.loads(mm.group(1))
            m["closure"] = {k: dc.get(k) for k in ("available", "initial_total", "initial_closed",
                                                   "total_domains", "total_closed", "unresolved_domains")}
        except json.JSONDecodeError:
            pass
    return m


def cpu_times(cpus):
    busy = total = 0
    want = {f"cpu{c}" for c in cpus}
    with open("/proc/stat") as f:
        for line in f:
            parts = line.split()
            if parts and parts[0] in want:
                vals = [int(x) for x in parts[1:]]
                idle = vals[3] + vals[4]
                total += sum(vals[:8])
                busy += sum(vals[:8]) - idle
    return busy, total


def tree_cpu_ticks(pids):
    ticks = 0
    for p in pids:
        try:
            with open(f"/proc/{p}/stat") as f:
                rest = f.read().rsplit(")", 1)[1].split()
            ticks += int(rest[11]) + int(rest[12])
        except (OSError, ValueError, IndexError):
            pass
    return ticks


def run_delays(pids, seen):
    for p in pids:
        try:
            tids = os.listdir(f"/proc/{p}/task")
        except OSError:
            continue
        for t in tids:
            try:
                with open(f"/proc/{p}/task/{t}/schedstat") as f:
                    run_ns, wait_ns, _ = (int(x) for x in f.read().split())
                key = (p, t)
                old = seen.get(key, (0, 0))
                seen[key] = (max(old[0], run_ns), max(old[1], wait_ns))
            except (OSError, ValueError):
                pass


def recorder(pid, cpus, stop, holder):
    """Foreign load on the run CPUs and schedstat run delay of the walk."""
    hz = os.sysconf("SC_CLK_TCK")
    samples = []
    seen = {}
    b0, t0 = cpu_times(cpus)
    own0 = tree_cpu_ticks(descendants(pid))
    w0 = time.time()
    while not stop.wait(10.0):
        pids = descendants(pid)
        b1, t1 = cpu_times(cpus)
        own1 = tree_cpu_ticks(pids)
        w1 = time.time()
        busy_cpus = (b1 - b0) / hz / (w1 - w0)
        own_cpus = max(0, own1 - own0) / hz / (w1 - w0)
        samples.append((round(w1 - holder["start"], 1), round(busy_cpus, 3), round(own_cpus, 3)))
        b0, t0, own0, w0 = b1, t1, own1, w1
        run_delays(pids, seen)
    fb = [max(0.0, b - o) for _, b, o in samples]
    holder["recorder"] = {
        "run_cpus": len(cpus), "samples": len(samples), "interval_seconds": 10,
        "foreign_busy_cpus_mean": round(sum(fb) / len(fb), 3) if fb else None,
        "foreign_busy_cpus_max": round(max(fb), 3) if fb else None,
        "foreign_busy_fraction_mean": round(sum(fb) / len(fb) / len(cpus), 4) if fb else None,
        "own_busy_cpus_mean": round(sum(o for _, _, o in samples) / len(samples), 3) if samples else None,
        "schedstat_run_seconds": round(sum(v[0] for v in seen.values()) * 1e-9, 1),
        "schedstat_run_delay_seconds": round(sum(v[1] for v in seen.values()) * 1e-9, 1),
        "schedstat_threads": len(seen),
        "series_t_busy_own": samples,
        "method": "foreign = busy(run CPUs, /proc/stat) - own process-tree utime+stime; run delay = sum over walk threads of /proc/<pid>/task/<tid>/schedstat field 2 (max seen)",
    }


def perf_block(path):
    """Parse `perf stat -x,` output: value,unit,event,run-time,percent,..."""
    out = {}
    if not path.exists():
        return None
    for line in path.read_text().splitlines():
        parts = line.split(",")
        if len(parts) < 3 or line.startswith("#"):
            continue
        try:
            value = float(parts[0])
        except ValueError:
            out[parts[2]] = parts[0]
            continue
        out[parts[2]] = value
        if len(parts) > 4 and parts[4]:
            out[parts[2] + "_enabled_percent"] = float(parts[4])
    ins, cyc = out.get("instructions:u"), out.get("cycles:u")
    if isinstance(ins, float) and isinstance(cyc, float) and cyc > 0:
        out["ipc_u"] = round(ins / cyc, 4)
    out["tool"] = PERF + " stat -x, -e instructions:u,cycles:u,task-clock (whole process, all threads, user space)"
    return out


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--binary", required=True)
    p.add_argument("--family", required=True, choices=sorted(run_control.COMMANDS) + ["hot"])
    p.add_argument("--label", required=True)
    p.add_argument("--cpus", default="264-287")
    p.add_argument("--policy")
    p.add_argument("--workers", type=int)
    p.add_argument("--queries")
    p.add_argument("--env", nargs="*", default=[])
    p.add_argument("--time-limit", type=float)
    p.add_argument("--grace", type=float, default=300.0)
    p.add_argument("--nice", type=int, default=5)
    p.add_argument("--perf", action="store_true")
    p.add_argument("--out-root", default=str(ROOT / "TMP/w0/g2falsify/runs"))
    args = p.parse_args()
    out = Path(args.out_root) / args.label / args.family
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    if args.family == "hot":
        argv = hot_argv(args.queries or str(ROOT / "TMP/qcd-feynman-d9d10-pilot-hot-owner/queries.json"))
    else:
        argv = json.load(open(run_control.COMMANDS[args.family]))
    argv = run_control.rewrite(argv, args.binary, out, args.policy, args.workers, None, [],
                               args.family == "five-finite")
    json.dump(argv, open(out / "command.json", "w"), indent=1)
    if args.perf:
        argv = [PERF, "stat", "-x", ",", "-o", str(out / "perf-stat.csv"),
                "-e", "instructions:u,cycles:u,task-clock", "--"] + argv
    env = dict(os.environ)
    env.update(run_control.ENV_ONE)
    env["TMPDIR"] = str(ROOT / "TMP")
    extra_env = dict(kv.split("=", 1) for kv in args.env)
    env.update(extra_env)
    cpus = run_control.cpu_list(args.cpus)
    stderr = open(out / "stderr", "w")
    stdout = open(out / "stdout", "w")
    start = time.time()
    proc = subprocess.Popen(
        ["nice", "-n", str(args.nice), "nix", "develop", str(ROOT), "--command"] + argv,
        cwd=ROOT, env=env, stdout=stdout, stderr=stderr, start_new_session=True,
        preexec_fn=lambda: os.sched_setaffinity(0, cpus))
    stop = threading.Event()
    holder = {"start": start}
    watcher = threading.Thread(target=watch, args=(proc.pid, stop, holder))
    watcher.start()
    rec = threading.Thread(target=recorder, args=(proc.pid, cpus, stop, holder))
    rec.start()
    stopped_by_limit = None
    killed = False
    stop_file = out / "stop-request.json"
    while True:
        try:
            code = proc.wait(timeout=2.0)
            break
        except subprocess.TimeoutExpired:
            pass
        elapsed = time.time() - start
        if args.time_limit and elapsed > args.time_limit and stopped_by_limit is None:
            stop_file.write_text(json.dumps({"reason": "w0 falsify time limit", "elapsed": elapsed}))
            stopped_by_limit = elapsed
        if stopped_by_limit is not None and elapsed > stopped_by_limit + args.grace and not killed:
            os.killpg(proc.pid, signal.SIGKILL)
            killed = True
    stop.set()
    watcher.join()
    rec.join()
    holder.pop("start", None)
    wall = time.time() - start
    metrics = {"family": args.family, "label": args.label, "binary": args.binary,
               "exit_code": code, "whole_command_seconds": round(wall, 3), "cpus": args.cpus,
               "policy": args.policy, "workers": args.workers, "env": extra_env,
               "queries": args.queries, "time_limit": args.time_limit,
               "stopped_by_time_limit_at": stopped_by_limit, "killed_after_grace": killed,
               "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(start))}
    metrics.update(holder)
    if args.perf:
        metrics["perf"] = perf_block(out / "perf-stat.csv")
    metrics.update(extract(out / "result.json"))
    json.dump(metrics, open(out / "metrics.json", "w"), indent=1)
    print(json.dumps(metrics, indent=1))


if __name__ == "__main__":
    main()
