#!/usr/bin/env python
"""W1 G2' production arm runner (lane g2prod): the W0 G2' falsifier runner
(tools/research/w0_g2falsify/run_arm.py) with `--g2 off|union` passed as the
production request flag `--g2-residual-anchors` and `--extra` native arguments
(e.g. `--frontier-policy stop`), default output TMP/w1/g2prod/runs. One walk per call.

Reuses the historical control command lines of TMP/fable51-controls/run_control.py
(four-loop FG/BMW/H/X at the A<=19, R<=12, D>=7 saved-cover envelope and the
five-loop 1,324-tuple finite control) plus the hot-owner family (C-HOT inputs
with a replaceable queries file for C-HOT-sub). Only the executable, output
paths, policy, workers, CPU set and environment are substituted.
For LC2, pass --command /common/dev/rustred/TMP/lc2/commands/FAMILY.json
to use the converted format-6 input paths rather than the historical inputs.

Usage:
  run_arm.py --binary BIN --family fg|bmw|h|x|five-finite|hot --label NAME
             --cpus 264-287 [--workers N] [--policy ordered|ready]
             [--queries FILE (hot only)] [--env KEY=VALUE ...]
             [--time-limit SECONDS] [--grace SECONDS] [--out-root DIR]
             [--heavy-lock PATH] [--lock PATH ...]
             [--minimum-start-headroom-gib 250 --minimum-headroom-gib 150]
Writes <out-root>/<label>/<family>/{command.json,result.json,events.jsonl,
stderr,stdout,metrics.json}. A time limit writes the stop file (cooperative
pause) and kills the owned new-session process group after the grace period.
Guarded LC2 pilots must pass the shared heavy lock and both headroom options;
locks remain held until the complete group drains, even if its launcher exits.
Any operator/resource/time stop is censored, including a cooperative exit 0.
Recorder (plan section 7 / audit directive 0.1.7): every 10 s the busy time of
the run CPUs (/proc/stat) minus the run's own process-tree CPU time gives the
foreign busy CPUs; per-thread schedstat run delay of the walk process is
summed at the end (max seen per thread). With --perf the walk binary runs
under `perf stat -e instructions:u,cycles:u,task-clock` (user-space counts of
the whole process, every thread; perf_event_paranoid 2 allows :u events), and
metrics.json gets a "perf" block (instructions per native: see gate.py).
"""
import argparse
import fcntl
import json
import math
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
sys.path.insert(0, str(ROOT / "TMP/fable51-controls"))
import run_control  # noqa: E402  (historical command lines and rewrite())

PERF = "/nix/store/7ccpnz8xkn5qsyw8nkz998vjmb6jpl49-perf-linux-6.19.6/bin/perf"
HOT_COMMAND = ROOT / "TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ordered/run/request.json"
GIB = 1024 ** 3


def mem_available():
    for line in Path("/proc/meminfo").read_text().splitlines():
        if line.startswith("MemAvailable:"):
            return int(line.split()[1]) * 1024
    raise RuntimeError("MemAvailable unavailable")


def group_exists(group):
    try:
        os.killpg(group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def group_running(group):
    """The owned new-session group, including children of an exited launcher.

    Native/Nix children inherit this PG. Zombies cannot work; no unrelated
    PID, ancestry guess, or production process is signalled.
    """
    try:
        paths = list(Path("/proc").iterdir())
    except OSError:
        # Without the zombie-aware view, do not pretend a still-existing
        # group has drained. ESRCH remains sufficient evidence of absence.
        return group_exists(group)
    uncertain = False
    for path in paths:
        if not path.name.isdigit():
            continue
        try:
            fields = (path / "stat").read_text().rsplit(")", 1)[1].split()
            if int(fields[2]) == group and int(fields[3]) == group and fields[0] != "Z":
                return True
        except (FileNotFoundError, ProcessLookupError):
            pass  # A process that disappeared during the scan cannot work.
        except (OSError, ValueError, IndexError):
            uncertain = True
    return group_exists(group) if uncertain else False


class ArmGuard:
    """Own the solver session and its resource locks, not an outer wrapper.

    Every stop first writes the native cooperative stop file. Grace expires
    into SIGKILL of this new-session PG only. Locks outlive the launcher if
    any of its non-zombie group members remain.
    """
    def __init__(self, stop_file, locks=(), minimum_start=None, minimum_run=None,
                 time_limit=None, grace=300.0):
        self.stop_file, self.paths = Path(stop_file), list(dict.fromkeys(map(Path, locks)))
        self.minimum_start, self.minimum_run = minimum_start, minimum_run
        self.time_limit, self.grace = time_limit, grace
        self.locks, self.handlers = [], {}
        self.proc = self.start = self.launcher_exit = self.drained = None
        self.reason = self.stopped = self.lowest = self.initial_memory = None
        self.killed = False
        self.lock_wait_seconds = 0.0

    def __enter__(self):
        for sig in (signal.SIGINT, signal.SIGTERM):
            self.handlers[sig] = signal.signal(sig, lambda signum, frame: self.stop(f"operator_signal_{signum}"))
        return self

    def stop(self, reason):
        if self.reason is None:
            self.reason, self.stopped = reason, time.monotonic()
            try:
                with self.stop_file.open("x") as output:
                    json.dump({"reason": reason, "requested_unix": time.time()}, output)
            except FileExistsError:
                pass
            except OSError:
                # Cooperative publication failed: do not wait the normal
                # grace with a solver that cannot observe its stop request.
                self.reason += ":stop_file_unavailable"
                self.stopped -= self.grace

    def memory_ok(self, starting=False):
        threshold = self.minimum_start if starting else self.minimum_run
        if threshold is None:
            return True
        try:
            current = mem_available()
            self.lowest = current if self.lowest is None else min(self.lowest, current)
            if starting:
                self.initial_memory = current
        except (OSError, RuntimeError, ValueError, IndexError):
            self.stop("memory_monitor_unavailable")
            return False
        if current < threshold:
            self.stop("insufficient_start_headroom" if starting else "host_headroom_below_minimum")
            return False
        return True

    def acquire(self):
        waiting = time.monotonic()
        for path in self.paths:
            lock = path.open("a")
            self.locks.append(lock)
            while self.reason is None:
                try:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    time.sleep(0.1)
            if self.reason is not None:
                break
        self.lock_wait_seconds = time.monotonic() - waiting
        return self.reason is None and self.memory_ok(starting=True)

    def launch(self, command, **kwargs):
        if self.reason is not None:
            return None
        self.start = time.monotonic()
        self.proc = subprocess.Popen(command, start_new_session=True, **kwargs)
        return self.proc

    def wait(self):
        if self.proc is None:
            return 125
        while True:
            code = self.proc.poll()
            now = time.monotonic()
            if code is not None and self.launcher_exit is None:
                self.launcher_exit = now
            # Do not let an exited launcher release the locks around its
            # still-running native children (including during kill grace).
            if code is not None and not group_running(self.proc.pid):
                self.drained = time.monotonic()
                return code
            self.memory_ok()
            if self.time_limit is not None and now - self.start >= self.time_limit:
                self.stop("time_limit")
            if self.stopped is not None and now - self.stopped >= self.grace:
                try:
                    os.killpg(self.proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                self.killed = True
            time.sleep(0.2)

    def receipt(self):
        return {"locks": [str(path) for path in self.paths],
                "lock_wait_seconds": self.lock_wait_seconds,
                "minimum_start_headroom_bytes": self.minimum_start,
                "minimum_headroom_bytes": self.minimum_run,
                "start_mem_available_bytes": self.initial_memory,
                "minimum_mem_available_bytes": self.lowest,
                "stop_reason": self.reason, "child_started": self.proc is not None,
                "process_group": self.proc.pid if self.proc else None,
                "launcher_wait_seconds": self.launcher_exit - self.start if self.launcher_exit else None,
                "owned_group_drain_seconds": self.drained - self.launcher_exit if self.drained else None}

    def __exit__(self, kind, error, traceback):
        try:
            if self.proc is not None and (self.proc.poll() is None or group_running(self.proc.pid)):
                self.stop("runner_error" if kind else "runner_early_exit")
                self.wait()
        finally:
            for lock in reversed(self.locks):
                lock.close()
            for sig, handler in self.handlers.items():
                signal.signal(sig, handler)


def wait_with_recorders(guard, stop, threads):
    """Join every successfully started sampler even if a later start fails."""
    started = []
    try:
        for thread in threads:
            thread.start()
            started.append(thread)
        return_code = guard.wait()
    finally:
        exited = time.monotonic()
        stop.set()
        for thread in started:
            thread.join()
        shutdown = time.monotonic() - exited
    return return_code, shutdown


def waited_child_usage(before, after):
    return {"child_user_seconds": after.ru_utime - before.ru_utime,
            "child_system_seconds": after.ru_stime - before.ru_stime,
            "maximum_single_waited_child_rss_bytes": after.ru_maxrss * 1024,
            "child_cpu_scope": "RUSAGE_CHILDREN delta around launch through owned-group drain; waited launcher/native/Nix descendants",
            "single_child_rss_scope": "RUSAGE_CHILDREN cumulative maximum since runner startup; not concurrent aggregate RSS"}


def hot_argv(queries):
    argv = list(json.load(open(HOT_COMMAND))["command"])
    q = json.load(open(queries))
    size = os.path.getsize(queries)
    argv[argv.index("--queries") + 1] = str(queries)
    argv[argv.index("--max-queries") + 1] = str(len(q["queries"]))
    argv[argv.index("--max-query-bytes") + 1] = str(size)
    return argv


def command_template(path):
    """Read an explicit immutable argv template (e.g. the LC2 v6 controls)."""
    with open(path) as source:
        argv = json.load(source)
    if not isinstance(argv, list) or not argv or any(not isinstance(arg, str) for arg in argv):
        raise ValueError("command template must be a nonempty JSON array of strings")
    return argv


def configured_workers(argv):
    """Actual explicit budget after overrides, not the optional runner argument."""
    try:
        workers = int(argv[argv.index("--workers") + 1])
    except (ValueError, IndexError):
        return None
    return workers if workers > 0 else None


def descendants(pid):
    kids = {}
    owned_group = set()
    for d in os.listdir("/proc"):
        if not d.isdigit():
            continue
        try:
            with open(f"/proc/{d}/stat") as f:
                fields = f.read().rsplit(")", 1)[1].split()
                ppid = int(fields[1])
            # A launcher can exit before its children, which are then
            # reparented but still belong to our owned session/group.
            if int(fields[2]) == pid and int(fields[3]) == pid and fields[0] != "Z":
                owned_group.add(int(d))
            kids.setdefault(ppid, []).append(int(d))
        except (OSError, ValueError, IndexError):
            pass
    out, stack = [], [pid]
    while stack:
        p = stack.pop()
        out.append(p)
        stack.extend(kids.get(p, []))
    return sorted(set(out) | owned_group)


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


EXTRACT_HEAD_BYTES = 4_000_000
EXTRACT_TAIL_BYTES = 400_000
ROOT_METRICS = (
    "traversal_seconds", "prepared_seconds", "elapsed_seconds", "completed_nodes",
    "scheduled_nodes", "queued_nodes", "containment_checks", "containment_maintenance_checks",
    "containment_retired_candidates", "containment_semantic_hits", "containment_semantic_retirements",
    "successors", "events", "deduplication_hits", "route_masks", "routed_domains",
    "max_scheduled_finite_rank", "native_processed_nodes", "frontiers", "status",
    "all_scheduled_domains_resolved", "pending_descendant_domains", "job_local_reuse_hits",
    "pre_admitted_orthant_hits", "exact_domain_hits", "g2_residual_anchors", "g2_index_telemetry", "delegation",
)


def bounded_root_values(path, wanted):
    """Known native pretty-JSON root fields only, with bounded teardown I/O.

    Native reports indent root members by exactly two spaces. Never accept a
    nested same-name field or an incomplete token/object at a buffer edge.
    Missing, differently formatted or truncated fields remain unknown. This
    is telemetry, not a JSON validity or closure oracle; full audits are separate.
    """
    if not path.exists():
        return {}
    size = path.stat().st_size
    with path.open("rb") as source:
        blobs = [source.read(min(size, EXTRACT_HEAD_BYTES)).decode("utf-8", "replace")]
        if size > EXTRACT_HEAD_BYTES:
            offset = max(0, size - EXTRACT_TAIL_BYTES)
            source.seek(offset)
            tail = source.read(EXTRACT_TAIL_BYTES).decode("utf-8", "replace")
            # Clipped indentation cannot make a nested key look like a root.
            blobs.append((tail.split("\n", 1)[1] if "\n" in tail else "") if offset else tail)
    values, ambiguous, decoder = {}, set(), json.JSONDecoder()
    member_end = re.compile(r",?\r?\n")
    for blob in blobs:
        for match in re.finditer(r'^  "([^"\n]+)": ', blob, re.M):
            key = match.group(1)
            if key not in wanted or key in ambiguous:
                continue
            try:
                value, end = decoder.raw_decode(blob, match.end())
            except (ValueError, RecursionError):
                continue
            # raw_decode accepts truncated numeric prefixes: require the full
            # pretty-JSON member delimiter and newline inside this buffer.
            if not member_end.match(blob, end):
                continue
            if key in values and values[key] != value:
                values.pop(key)
                ambiguous.add(key)
            else:
                values[key] = value
    return values


def extract(path):
    top = bounded_root_values(path, {*ROOT_METRICS, "descendant_closure", "parallel"})
    metrics = {key: top[key] for key in ROOT_METRICS if key in top}
    closure = top.get("descendant_closure")
    if isinstance(closure, dict):
        for key in ("initial_closed", "initial_total", "total_closed", "total_domains",
                    "unresolved_domains", "dependency_edges"):
            if key in closure:
                metrics[key] = closure[key]
        metrics["closure"] = {key: closure[key] for key in
                              ("available", "initial_total", "initial_closed", "total_domains",
                               "total_closed", "unresolved_domains") if key in closure}
    ledger = top.get("delegation")
    if isinstance(ledger, dict):
        for key in ("transferred_obligations", "native_discharged", "delegated_publications"):
            if key in ledger:
                metrics[key] = ledger[key]
    parallel = top.get("parallel")
    if isinstance(parallel, dict):
        for key, target in (("slot_busy_seconds", "slot_busy_seconds_sum"),
                            ("slot_backpressure_seconds", "slot_backpressure_seconds_sum")):
            values = parallel.get(key)
            if isinstance(values, list) and all(type(value) in (int, float) and math.isfinite(value) for value in values):
                metrics[target] = sum(values)
                if key == "slot_busy_seconds":
                    metrics["inspection_slots"] = len(values)
    return metrics


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
    p.add_argument("--family", required=True, choices=sorted(set(run_control.COMMANDS) | {"hot", "four-all-p5"}))
    p.add_argument("--label", required=True)
    p.add_argument("--cpus", default="264-287")
    p.add_argument("--policy")
    p.add_argument("--workers", type=int)
    p.add_argument("--queries")
    p.add_argument("--command", help="explicit command JSON array; use LC2 templates for format-6 inputs")
    p.add_argument("--env", nargs="*", default=[])
    p.add_argument("--time-limit", type=float)
    p.add_argument("--grace", type=float, default=300.0)
    p.add_argument("--heavy-lock", type=Path, help="shared resource lock held until the owned solver group drains")
    p.add_argument("--lock", type=Path, action="append", default=[], help="additional resource lock, after heavy-lock")
    p.add_argument("--minimum-start-headroom-gib", type=float)
    p.add_argument("--minimum-headroom-gib", type=float)
    p.add_argument("--nice", type=int, default=5)
    p.add_argument("--perf", action="store_true")
    p.add_argument("--g2", choices=("off", "union"), default="off")
    p.add_argument("--extra", nargs="*", default=[])
    p.add_argument("--out-root", default=str(ROOT / "TMP/w1/g2prod/runs"))
    args = p.parse_args()
    for name in ("time_limit", "grace", "minimum_start_headroom_gib", "minimum_headroom_gib"):
        value = getattr(args, name)
        if value is not None and (not math.isfinite(value) or value <= 0):
            p.error(f"--{name.replace('_', '-')} must be finite and positive")
    if (args.minimum_start_headroom_gib is None) != (args.minimum_headroom_gib is None):
        p.error("supply both start and runtime headroom thresholds")
    if args.minimum_start_headroom_gib is not None and args.minimum_start_headroom_gib < args.minimum_headroom_gib:
        p.error("start headroom must be at least runtime headroom")
    cpus = run_control.cpu_list(args.cpus)
    if not cpus or not set(cpus) <= os.sched_getaffinity(0):
        p.error("CPU affinity must be nonempty and available to this runner")
    guarded = args.heavy_lock or args.lock or args.minimum_start_headroom_gib is not None
    if guarded and set(cpus) & set(range(128, 228)):
        p.error("guarded pilot affinity overlaps protected production CPUs 128–227")
    if args.command and args.queries:
        p.error("--command and --queries are mutually exclusive; bind queries in the command template")
    if args.command:
        argv = command_template(args.command)
    elif args.family == "hot":
        argv = hot_argv(args.queries or str(ROOT / "TMP/qcd-feynman-d9d10-pilot-hot-owner/queries.json"))
    else:
        if args.family not in run_control.COMMANDS:
            p.error(f"{args.family} requires an explicit --command template")
        argv = command_template(run_control.COMMANDS[args.family])
    if "--g2-residual-anchors" in argv:
        p.error("command templates must omit --g2-residual-anchors; choose the arm with --g2")
    if guarded:
        # Keep the runner/samplers, not only the native child, off protected
        # production CPUs. The per-child affinity remains explicit below.
        os.sched_setaffinity(0, cpus)
    out = Path(args.out_root) / args.label / args.family
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    extra = list(args.extra)
    if args.g2 != "off":
        extra += ["--g2-residual-anchors", args.g2]
    argv = run_control.rewrite(argv, args.binary, out, args.policy, args.workers, None, extra,
                               args.family == "five-finite" and args.command is None)
    workers = configured_workers(argv)
    with (out / "command.json").open("w") as output:
        json.dump(argv, output, indent=1)
    if args.perf:
        argv = [PERF, "stat", "-x", ",", "-o", str(out / "perf-stat.csv"),
                "-e", "instructions:u,cycles:u,task-clock", "--"] + argv
    env = dict(os.environ)
    env.update(run_control.ENV_ONE)
    env["TMPDIR"] = str(ROOT / "TMP")
    extra_env = dict(kv.split("=", 1) for kv in args.env)
    env.update(extra_env)
    locks = ([args.heavy_lock] if args.heavy_lock else []) + args.lock
    guard = ArmGuard(out / "stop-request.json", locks,
                     args.minimum_start_headroom_gib * GIB if args.minimum_start_headroom_gib else None,
                     args.minimum_headroom_gib * GIB if args.minimum_headroom_gib else None,
                     args.time_limit, args.grace)
    start, wall, recorder_shutdown, code = None, None, 0.0, 125
    holder, failure, usage = {}, None, {}
    try:
        with guard, (out / "stderr").open("w") as stderr, (out / "stdout").open("w") as stdout:
            if guard.acquire():
                start = time.time()
                usage_before = resource.getrusage(resource.RUSAGE_CHILDREN)
                proc = guard.launch(
                    ["nice", "-n", str(args.nice), "nix", "develop", str(ROOT), "--command"] + argv,
                    cwd=ROOT, env=env, stdout=stdout, stderr=stderr,
                    preexec_fn=lambda: os.sched_setaffinity(0, cpus))
                if proc is not None:
                    stop = threading.Event()
                    holder["start"] = start
                    threads = [threading.Thread(target=watch, args=(proc.pid, stop, holder)),
                               threading.Thread(target=recorder, args=(proc.pid, cpus, stop, holder))]
                    try:
                        code, recorder_shutdown = wait_with_recorders(guard, stop, threads)
                    finally:
                        holder.pop("start", None)
                    wall = guard.drained - guard.start
                    usage = waited_child_usage(usage_before, resource.getrusage(resource.RUSAGE_CHILDREN))
    except Exception as error:
        guard.stop("runner_error")
        failure = f"{type(error).__name__}: {error}"
    stopped_by_limit = (guard.stopped - guard.start
                        if guard.reason == "time_limit" and guard.start is not None else None)
    metrics = {"family": args.family, "label": args.label, "binary": args.binary,
               "exit_code": code, "whole_command_seconds": round(wall, 3) if wall is not None else None, "cpus": args.cpus,
               "whole_command_timing_scope": "launcher-inclusive nice+nix+native launch through owned process-group drain; lock admission and recorder shutdown excluded",
               "recorder_shutdown_seconds": round(recorder_shutdown, 6),
               "policy": args.policy, "workers": workers, "requested_workers": args.workers,
               "env": {key: "<redacted>" if any(word in key.upper() for word in ("LICENSE", "TOKEN", "SECRET", "PASSWORD", "KEY")) else value
                       for key, value in extra_env.items()}, "g2": args.g2,
               "extra": args.extra,
               "queries": args.queries, "command_template": args.command, "time_limit": args.time_limit,
               "stopped_by_time_limit_at": stopped_by_limit, "killed_after_grace": guard.killed,
               "stop_reason": guard.reason, "censored": guard.reason is not None,
               "resource_guard": guard.receipt(), "runner_error": failure,
               "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(start)) if start is not None else None}
    metrics.update(holder)
    metrics.update(usage)
    if args.perf:
        metrics["perf"] = perf_block(out / "perf-stat.csv")
    metrics.update(extract(out / "result.json"))
    with (out / "metrics.json").open("w") as output:
        json.dump(metrics, output, indent=1)
    print(json.dumps(metrics, indent=1))
    return 0 if code == 0 and guard.reason is None and failure is None else 1


if __name__ == "__main__":
    sys.exit(main())
