#!/usr/bin/env python
"""Control runner for the combined four-loop walks (four-all and variants), with the mandatory recorder.

Self-contained successor of the lane-local runner TMP/c4l-s2/run_c4l.py (sha256 42a30903..., used for
sessions B2 and C). The argv rewriting, the cooperative stop and the result extraction are copied verbatim
from the shared runner TMP/fable51-controls/run_control.py at sha256 8dace33c... (rewrite, peak_rss,
extract, cpu_list), so this file no longer imports anything from TMP.

usage:
  run_c4l.py --binary BIN --command ARGV.json --family NAME --label LABEL --cpus LIST
             [--policy ordered|ready] [--workers N] [--timeout-seconds S] [--stop-natives N]
             [--out-root DIR] [--nice 5]

Writes <out-root>/<label>/<family>/{command.json,events.jsonl,result.json,checkpoint/,stdout,stderr,
perfstat.csv,metrics.json}. ARGV.json is an owner-domain-match argv template (a JSON list); the runner
substitutes the executable, the output/events/stop-file/checkpoint paths, the publication policy and
the worker count.

Stops (both cooperative: the runner writes the native --stop-file, the native checkpoints and writes
result.json; SIGKILL only after a 600 s grace):
  --timeout-seconds S   after S seconds of wall time (the <= 1 h rule);
  --stop-natives N      once an events.jsonl heartbeat reports N completed natives (README section 5:
                        60,000 = twice the Ordered reference; no drained four-all variant exceeded 38,173).

Recorder, stamped into metrics.json (FABLE_5_1_CRITIQUE 2.6, HANDOFF_opus_5_5 0.1 item 7):
  foreign_load: busy jiffies on the run CPUs (/proc/stat user+nice+system+irq+softirq+steal) minus
                this run's own CPU (getrusage children delta), over the run wall;
  recorder:     perf stat user-mode instructions and cycles of the whole native process -> IPC and
                instructions per native; schedstat on-CPU time and run delay summed over the native's
                threads (sampled every 0.5 s, max per thread); user and system seconds, page faults and
                context switches of the run (getrusage children delta); the effective user-mode clock
                (user cycles / user seconds).
  The user/system split was added in the c4l fix round (2026-09-28) after the unexplained 10x slowdown of
  c4l-4a17f9c7-ready-w24s1-rep5 (README section 5), which the earlier recorder could not attribute.
"""
import argparse
import json
import os
import re
import resource
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path("/common/dev/rustred")
OUT_ROOT = ROOT / "TMP/fable51-controls"
ENV_ONE = {
    "RAYON_NUM_THREADS": "1", "OMP_NUM_THREADS": "1", "OMP_THREAD_LIMIT": "1",
    "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1", "BLIS_NUM_THREADS": "1",
}
HZ = os.sysconf("SC_CLK_TCK")
PERF = "/nix/store/gyp2si1k1w7jhw8z4xx1bwr2m0pr5445-perf-linux-7.2/bin/perf"
HEARTBEAT_NATIVES = re.compile(rb'"expanded_nodes":(\d+)')


# --- copied from TMP/fable51-controls/run_control.py (sha256 8dace33c...) -------------------------
def cpu_list(spec):
    cpus = []
    for part in spec.split(","):
        if "-" in part:
            a, b = part.split("-")
            cpus.extend(range(int(a), int(b) + 1))
        else:
            cpus.append(int(part))
    return cpus


def rewrite(argv, binary, out, policy, workers):
    argv = list(argv)
    argv[0] = str(binary)

    def set_opt(name, value):
        if name in argv:
            argv[argv.index(name) + 1] = str(value)
        else:
            argv.extend([name, str(value)])
    set_opt("--output", out / "result.json")
    set_opt("--events", out / "events.jsonl")
    set_opt("--stop-file", out / "stop-request.json")
    if "--checkpoint" in argv:
        set_opt("--checkpoint", out / "checkpoint")
    if policy:
        set_opt("--publication-policy", policy)
    if workers:
        set_opt("--workers", workers)
    return argv


def peak_rss(pid, stop, holder):
    peak = 0
    while not stop.is_set():
        try:
            with open(f"/proc/{pid}/status") as f:
                for line in f:
                    if line.startswith("VmRSS:"):
                        peak = max(peak, int(line.split()[1]) * 1024)
                        break
        except OSError:
            break
        time.sleep(0.5)
    holder["peak_rss_bytes"] = peak


def extract(result_path):
    metrics = {}
    if not result_path.exists():
        return metrics
    keys = ("traversal_seconds", "prepared_seconds", "elapsed_seconds", "completed_nodes",
            "scheduled_nodes", "queued_nodes", "containment_checks",
            "containment_maintenance_checks", "containment_retired_candidates",
            "containment_semantic_hits", "containment_semantic_retirements",
            "successors", "events", "deduplication_hits", "route_masks", "routed_domains",
            "max_scheduled_finite_rank", "native_processed_nodes", "frontiers")
    # result.json can be GBs: scan head and tail only.
    size = result_path.stat().st_size
    with open(result_path, "rb") as f:
        head = f.read(min(size, 4_000_000)).decode("utf-8", "replace")
        if size > 4_000_000:
            f.seek(max(0, size - 200_000))
            tail = f.read().decode("utf-8", "replace")
        else:
            tail = ""
    for key in keys:
        for blob in (head, tail):
            m = re.search(r'"%s": ([0-9.]+|null)' % key, blob)
            if m:
                metrics[key] = m.group(1)
                break
    return metrics
# --- end of the copied part ---------------------------------------------------------------------


def children_of(pid):
    try:
        with open(f"/proc/{pid}/task/{pid}/children") as f:
            return [int(x) for x in f.read().split()]
    except OSError:
        return []


def find_native(root, binary_name):
    stack = [root]
    while stack:
        pid = stack.pop()
        try:
            with open(f"/proc/{pid}/comm") as f:
                comm = f.read().strip()
        except OSError:
            continue
        if comm == binary_name[:15]:
            return pid
        stack.extend(children_of(pid))
    return None


def schedstat_sampler(root, binary_name, stop, holder):
    """Max per thread of (on-CPU ns, run-queue wait ns, timeslices) from /proc/<native>/task/*/schedstat."""
    per_tid = {}
    native = None
    while not stop.is_set():
        if native is None:
            native = find_native(root, binary_name)
        if native is not None:
            try:
                for tid in os.listdir(f"/proc/{native}/task"):
                    try:
                        with open(f"/proc/{native}/task/{tid}/schedstat") as f:
                            run_ns, wait_ns, slices = (int(x) for x in f.read().split()[:3])
                    except (OSError, ValueError):
                        continue
                    old = per_tid.get(tid, (0, 0, 0))
                    per_tid[tid] = (max(old[0], run_ns), max(old[1], wait_ns), max(old[2], slices))
            except OSError:
                pass
        stop.wait(0.5)
    holder["schedstat"] = {"threads_seen": len(per_tid),
                           "on_cpu_seconds": round(sum(v[0] for v in per_tid.values()) / 1e9, 3),
                           "run_delay_seconds": round(sum(v[1] for v in per_tid.values()) / 1e9, 3),
                           "timeslices": sum(v[2] for v in per_tid.values()),
                           "method": "max per thread of /proc/<native>/task/*/schedstat sampled every 0.5 s; "
                                     "threads that exit between samples are undercounted"}


def read_perf(path):
    out = {}
    try:
        for line in open(path):
            parts = line.strip().split(",")
            if len(parts) >= 3 and parts[0] and parts[0][0].isdigit():
                out[parts[2]] = int(parts[0])
    except OSError:
        pass
    return out


def cpu_busy(cpus):
    want = {f"cpu{c}" for c in cpus}
    busy = 0
    with open("/proc/stat") as f:
        for line in f:
            parts = line.split()
            if parts and parts[0] in want:
                v = [int(x) for x in parts[1:]]
                # user nice system idle iowait irq softirq steal
                busy += v[0] + v[1] + v[2] + v[5] + v[6] + (v[7] if len(v) > 7 else 0)
    return busy / HZ


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--binary", required=True)
    p.add_argument("--command", required=True, type=Path, help="owner-domain-match argv template (JSON list)")
    p.add_argument("--family", required=True, help="directory name of the walk")
    p.add_argument("--label", required=True)
    p.add_argument("--cpus", required=True)
    p.add_argument("--policy")
    p.add_argument("--workers", type=int)
    p.add_argument("--nice", type=int, default=5)
    p.add_argument("--timeout-seconds", type=int)
    p.add_argument("--stop-natives", type=int,
                   help="cooperative early stop once a heartbeat reports this many completed natives")
    p.add_argument("--out-root", type=Path, default=OUT_ROOT)
    args = p.parse_args()
    out = args.out_root / args.label / args.family
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    argv = json.load(open(args.command))
    argv = rewrite(argv, args.binary, out, args.policy, args.workers)
    json.dump(argv, open(out / "command.json", "w"), indent=1)
    stop_file = Path(argv[argv.index("--stop-file") + 1])
    env = dict(os.environ)
    env.update(ENV_ONE)
    env["TMPDIR"] = str(ROOT / "TMP")
    cpus = cpu_list(args.cpus)
    stderr = open(out / "stderr", "w")
    stdout = open(out / "stdout", "w")
    ru0 = resource.getrusage(resource.RUSAGE_CHILDREN)
    busy0 = cpu_busy(cpus)
    start = time.time()
    perf_out = out / "perfstat.csv"
    perf = PERF if os.path.exists(PERF) else "perf"
    proc = subprocess.Popen(
        ["nice", "-n", str(args.nice), "nix", "develop", "--command", perf, "stat", "-x,", "-o", str(perf_out),
         "-e", "instructions:u,cycles:u", "--"] + argv,
        cwd=ROOT, env=env, stdout=stdout, stderr=stderr,
        preexec_fn=lambda: os.sched_setaffinity(0, cpus), start_new_session=True)
    stop = threading.Event()
    holder = {}
    watcher = threading.Thread(target=peak_rss, args=(proc.pid, stop, holder))
    watcher.start()
    sched_holder = {}
    sampler = threading.Thread(target=schedstat_sampler,
                               args=(proc.pid, os.path.basename(argv[0]), stop, sched_holder))
    sampler.start()
    timed_out = False
    stop_reason = None
    stop_at = {}
    deadline = start + args.timeout_seconds if args.timeout_seconds else None
    events_path = out / "events.jsonl"
    events_pos = 0
    heartbeat_natives = 0
    while True:
        try:
            code = proc.wait(timeout=2)
            break
        except subprocess.TimeoutExpired:
            pass
        if args.stop_natives:
            # incremental tail of events.jsonl: last heartbeat expanded_nodes (natives completed so far)
            try:
                with open(events_path, "rb") as f:
                    f.seek(events_pos)
                    chunk = f.read()
                cut = chunk.rfind(b"\n")
                if cut >= 0:
                    events_pos += cut + 1
                    for line in chunk[:cut].split(b"\n"):
                        if b'"event":"heartbeat"' in line:
                            m = HEARTBEAT_NATIVES.search(line)
                            if m:
                                heartbeat_natives = int(m.group(1))
            except OSError:
                pass
            if heartbeat_natives >= args.stop_natives:
                stop_reason = "run_c4l_native_cap"
        if stop_reason is None and deadline is not None and time.time() >= deadline:
            stop_reason = "run_c4l_timeout"
        if stop_reason:
            timed_out = True
            stop_at = {"stop_reason": stop_reason, "stop_requested_after_seconds": round(time.time() - start, 1),
                       "heartbeat_natives_at_stop_request": heartbeat_natives}
            with open(stop_file, "x") as receipt:
                json.dump({"reason": stop_reason, "unix_time": time.time(),
                           "heartbeat_natives": heartbeat_natives, "family_closure_claim": False}, receipt)
            try:
                code = proc.wait(timeout=600)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, signal.SIGKILL)
                code = proc.wait()
            break
    wall = time.time() - start
    busy1 = cpu_busy(cpus)
    ru1 = resource.getrusage(resource.RUSAGE_CHILDREN)
    stop.set()
    watcher.join()
    sampler.join()
    user_s = ru1.ru_utime - ru0.ru_utime
    sys_s = ru1.ru_stime - ru0.ru_stime
    own = user_s + sys_s
    busy = busy1 - busy0
    foreign = max(0.0, busy - own)
    metrics = {"family": args.family, "label": args.label, "binary": args.binary,
               "command_template": str(args.command), "runner": "examples/input/four_loop_combined/tools/run_c4l.py",
               "exit_code": code, "whole_command_seconds": round(wall, 3), "cpus": args.cpus,
               "policy": args.policy, "workers": args.workers, "timeout_seconds": args.timeout_seconds,
               "stop_natives": args.stop_natives, "cooperative_stop_requested": timed_out, **stop_at,
               "foreign_load": {"run_cpu_busy_seconds": round(busy, 2), "own_cpu_seconds": round(own, 2),
                                "foreign_cpu_seconds": round(foreign, 2),
                                "foreign_fraction_of_run_cpus": round(foreign / (len(cpus) * wall), 4) if wall > 0 else None,
                                "method": "/proc/stat busy (user+nice+system+irq+softirq+steal) on run CPUs minus getrusage(children) delta"}}
    metrics.update(holder)
    metrics.update(extract(out / "result.json"))
    counters = read_perf(perf_out)
    natives = int(metrics.get("completed_nodes") or 0)
    ins, cyc = counters.get("instructions:u"), counters.get("cycles:u")
    metrics["recorder"] = {
        "instructions_u": ins, "cycles_u": cyc,
        "ipc": round(ins / cyc, 3) if ins and cyc else None,
        "instructions_per_native": round(ins / natives) if ins and natives else None,
        "scope_instructions": "whole native process (coordinator, admission helpers and inspectors), user mode",
        **sched_holder.get("schedstat", {}),
        "foreign_busy_cpus": round(foreign / wall, 2) if wall > 0 else None,
        "user_seconds": round(user_s, 3), "system_seconds": round(sys_s, 3),
        "system_fraction_of_own_cpu": round(sys_s / own, 4) if own > 0 else None,
        "effective_user_ghz": round(cyc / user_s / 1e9, 3) if cyc and user_s > 0 else None,
        "minor_faults": ru1.ru_minflt - ru0.ru_minflt, "major_faults": ru1.ru_majflt - ru0.ru_majflt,
        "voluntary_context_switches": ru1.ru_nvcsw - ru0.ru_nvcsw,
        "involuntary_context_switches": ru1.ru_nivcsw - ru0.ru_nivcsw,
        "scope_rusage": "getrusage(RUSAGE_CHILDREN) delta: the native plus its nix/perf wrappers"}
    json.dump(metrics, open(out / "metrics.json", "w"), indent=1)
    print(json.dumps(metrics, indent=1))


if __name__ == "__main__":
    main()
