#!/usr/bin/env python
"""W0.5 production baseline M1: resume a block clone of a production walk
checkpoint with a frame-pointer build and profile it for a fixed wall budget.

The native argv is the campaign's exact argv (request.json "command"); only the
executable, the output/events/stop-file paths, the input paths (byte-identical
clones; the checkpoint binding hashes file *contents*, not paths) and
`--checkpoint DIR` -> `--resume CLONE` are rewritten. Every rewritten argv is
refused if any element still points into /common/dev/rustred/campaigns/.

Timeline (T = first heartbeat carrying coordinator_duty after
`checkpoint_restored`, i.e. traversal restarted):
  T + early_offset         60 s perf window "early" (fp call graphs + perf stat)
  T + dwarf_offset         short DWARF call-graph window on the coordinator
  T + late_offset          60 s perf window "late"
  T + numa_offset          one /proc/PID/numa_maps census
  T + run_seconds          create the stop file (cooperative stop and save)
  launch + budget          SIGKILL if still alive (the clone is disposable)
The stop file is also created early if RSS exceeds --rss-stop-gib or host
MemAvailable falls below --min-memavail-gib.

Every 5 s it records per-thread /proc sched + schedstat counters, VmRSS/VmHWM,
/proc/stat busy/idle for the pinned CPUs (foreign-load estimate), MemAvailable
and loadavg to samples.jsonl, and extracts heartbeats (elapsed, discovered,
expanded, committed, RSS, coordinator_duty, admission_preparation, slot busy
sums) to heartbeats.jsonl. It never signals the process except through the
stop file and the budget SIGKILL.

perf buffers are sized explicitly (-m 16 for each fp record, one -m 64 DWARF
record at a time, each retried with smaller buffers): the
per-user perf mlock budget (perf_event_mlock_kb = 516 KB here) is shared with every
other perf session of the same user on the host, and default-size buffers for
several concurrent records failed with "Permission error mapping pages" in
run1 (2026-09-27).
"""
import argparse
import glob
import hashlib
import json
import os
import shlex
import signal
import subprocess
import sys
import time
from pathlib import Path

CAMPAIGNS = "/common/dev/rustred/campaigns/"
SCHED_KEYS = ("se.sum_exec_runtime", "se.nr_migrations", "nr_switches",
              "nr_voluntary_switches", "nr_involuntary_switches")
HZ = os.sysconf("SC_CLK_TCK")
STAT_EVENTS = ("cycles:u,instructions:u,cache-misses:u,dTLB-load-misses:u,"
               "ls_dmnd_fills_from_sys.dram_io_near:u,ls_dmnd_fills_from_sys.dram_io_far:u")


def cpu_list(spec):
    out = []
    for part in spec.split(","):
        if "-" in part:
            a, b = part.split("-")
            out.extend(range(int(a), int(b) + 1))
        else:
            out.append(int(part))
    return out


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 22), b""):
            h.update(block)
    return h.hexdigest()


def meminfo():
    out = {}
    for line in open("/proc/meminfo"):
        key, _, value = line.partition(":")
        if key in ("MemAvailable", "MemFree", "MemTotal", "SwapFree", "SwapTotal"):
            out[key] = int(value.split()[0]) * 1024
    return out


def thread_class(comm):
    if comm.startswith("owner-domain-"):
        return "inspector"
    if comm.startswith("owner-admission"):  # comm is truncated to 15 bytes: "owner-admission"
        return "admission_helper"
    if comm.startswith("owner-batch-admit-"):
        return "batch_admit"
    return "other"


def parse_sched(text):
    values = {}
    for line in text.splitlines():
        key, sep, value = line.partition(":")
        if not sep:
            continue
        key = key.strip()
        if key in SCHED_KEYS:
            try:
                values[key] = float(value.strip())
            except ValueError:
                pass
    return values


def read_threads(pid):
    threads = {}
    for task in glob.glob(f"/proc/{pid}/task/*"):
        tid = int(task.rsplit("/", 1)[1])
        try:
            comm = open(f"{task}/comm").read().strip()
            sched = open(f"{task}/sched").read()
            stat = open(f"{task}/stat").read()
            schedstat = open(f"{task}/schedstat").read().split()
        except OSError:
            continue
        row = {"comm": comm, "class": "coordinator" if tid == pid else thread_class(comm)}
        row.update(parse_sched(sched))
        close = stat.rfind(")")
        fields = stat[close + 2:].split()
        try:
            row.update({"state": fields[0], "utime_s": int(fields[11]) / HZ,
                        "stime_s": int(fields[12]) / HZ, "processor": int(fields[36])})
        except (IndexError, ValueError):
            pass
        if len(schedstat) >= 3:
            row["run_ns"], row["run_delay_ns"], row["timeslices"] = map(int, schedstat[:3])
        threads[tid] = row
    return threads


def read_status(pid):
    out = {}
    try:
        for line in open(f"/proc/{pid}/status"):
            if line.startswith(("VmRSS:", "VmHWM:", "Threads:", "RssAnon:", "RssFile:", "VmSwap:")):
                key, value = line.split(":", 1)
                out[key] = int(value.split()[0])
    except OSError:
        pass
    return out


def read_cpu_stat(cpus):
    wanted = {f"cpu{c}" for c in cpus}
    busy = idle = 0
    for line in open("/proc/stat"):
        name = line.split(" ", 1)[0]
        if name in wanted:
            v = [int(x) for x in line.split()[1:]]
            i = v[3] + v[4]
            idle += i
            busy += sum(v[:8]) - i
    return {"busy_jiffies": busy, "idle_jiffies": idle}


class EventTail:
    """Incremental reader of events.jsonl; keeps compact heartbeat extracts."""

    def __init__(self, path, sink):
        self.path = path
        self.offset = 0
        self.partial = b""
        self.sink = sink
        self.restored = None
        self.first_duty = None
        self.last = None
        self.markers = []

    def poll(self):
        try:
            with open(self.path, "rb") as f:
                f.seek(self.offset)
                data = f.read()
        except OSError:
            return
        self.offset += len(data)
        data = self.partial + data
        lines = data.split(b"\n")
        self.partial = lines.pop()
        now = time.time()
        for line in lines:
            if not line.strip():
                continue
            try:
                record = json.loads(line)
            except ValueError:
                continue
            event = record.get("event")
            if event != "heartbeat":
                if event in ("checkpoint_restored", "checkpoint_executable_changed", "checkpoint_started",
                             "checkpoint_saved", "finished", "admitted", "initial_overlap_prepared"):
                    marker = {"event": event, "observed_unix_time": now}
                    if event == "checkpoint_restored":
                        marker["restore"] = record.get("restore")
                        self.restored = now
                    else:
                        marker["record"] = {k: v for k, v in record.items() if k != "event"}
                    self.markers.append(marker)
                    self.sink.write(json.dumps(marker) + "\n")
                continue
            progress = record.get("progress") or {}
            parallel = progress.get("parallel") or {}
            row = {"observed_unix_time": now, "elapsed_seconds": record.get("elapsed_seconds"),
                   "discovered": record.get("currently_discovered_nodes"),
                   "expanded": record.get("expanded_nodes"),
                   "committed_domains": progress.get("committed_domains"),
                   "committed_events": progress.get("committed_events"),
                   "rss_bytes": record.get("process_rss_bytes"),
                   "queue_growth_per_second": record.get("queue_growth_per_second"),
                   "recent_nodes_per_second": record.get("recent_nodes_per_second")}
            delegation = progress.get("delegation") or {}
            for key in ("delegated_publications", "native_publications", "pending_native_publications",
                        "transferred_obligations", "logical_publications"):
                if key in delegation:
                    row[key] = delegation[key]
            closure = progress.get("descendant_closure") or {}
            for key in ("initial_closed", "dependency_edges", "refresh_count", "refresh_seconds"):
                if key in closure:
                    row["closure_" + key] = closure[key]
            if "coordinator_duty" in parallel:
                row["coordinator_duty"] = parallel["coordinator_duty"]
                row["admission_preparation"] = parallel.get("admission_preparation")
                for key in ("computing_workers", "active_workers", "occupied_native_slots",
                            "returned_inspections", "backpressure_seconds", "finished_awaiting_poll"):
                    row[key] = parallel.get(key)
                busy = parallel.get("slot_busy_seconds")
                if isinstance(busy, list):
                    row["slot_busy_seconds_sum"] = sum(busy)
                    row["slot_busy_seconds_n"] = len(busy)
                back = parallel.get("slot_backpressure_seconds")
                if isinstance(back, list):
                    row["slot_backpressure_seconds_sum"] = sum(back)
                if self.restored is not None and self.first_duty is None:
                    self.first_duty = now
                    row["traversal_marker"] = True
            self.last = row
            self.sink.write(json.dumps(row) + "\n")
        self.sink.flush()


def record_cmd(perf_base, pages, options, tids, data, seconds):
    """perf record with explicit buffer sizes, retried with smaller buffers when the
    per-user perf mlock budget (516 KB for all of this user's sessions on the host,
    other lanes included) is exhausted: "Permission error mapping pages"."""
    base = " ".join(perf_base)
    tries = " ".join(str(p) for p in pages)
    return ["bash", "-c",
            f"for m in {tries}; do {base} record -m $m {options} -t {tids} -o {data} -- sleep {seconds} 2>{data}.err; "
            f"rc=$?; cat {data}.err >&2; grep -q 'Permission error mapping pages' {data}.err || exit $rc; "
            f"echo \"retry with smaller buffer after -m $m\" >&2; sleep 1; done; exit $rc"]


def pick(threads, cls, count):
    rows = sorted((tid for tid, r in threads.items() if r["class"] == cls),
                  key=lambda t: (len(threads[t]["comm"]), threads[t]["comm"]))
    if len(rows) <= count:
        return rows
    if count == 1:
        return [rows[len(rows) // 2]]
    step = (len(rows) - 1) / (count - 1)
    return [rows[round(i * step)] for i in range(count)]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--binary", required=True, type=Path)
    p.add_argument("--request", required=True, type=Path, help="campaign run request.json")
    p.add_argument("--checkpoint", required=True, type=Path, help="block clone to resume")
    p.add_argument("--inputs", required=True, type=Path, help="byte-identical clone of the campaign inputs")
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--cpus", default="128-227")
    p.add_argument("--perf", required=True)
    p.add_argument("--perf-cpus", default="82-99")
    p.add_argument("--interval", type=float, default=5.0)
    p.add_argument("--run-seconds", type=float, default=1500)
    p.add_argument("--budget-seconds", type=float, default=3540)
    p.add_argument("--save-reserve-seconds", type=float, default=600)
    p.add_argument("--early-offset", type=float, default=180)
    p.add_argument("--late-offset", type=float, default=1260)
    p.add_argument("--window-seconds", type=int, default=60)
    p.add_argument("--dwarf-offset", type=float, default=720)
    p.add_argument("--dwarf-seconds", type=int, default=20)
    p.add_argument("--numa-offset", type=float, default=1380)
    p.add_argument("--frequency", type=int, default=499)
    p.add_argument("--rss-stop-gib", type=float, default=300)
    p.add_argument("--rss-kill-gib", type=float, default=450)
    p.add_argument("--min-memavail-gib", type=float, default=60)
    p.add_argument("--dry-run", action="store_true")
    args = p.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=False)
    request = json.load(open(args.request))
    argv = list(request["command"])
    campaign_inputs = None
    for i, a in enumerate(argv):
        if a == "--owner-base":
            campaign_inputs = argv[i + 1]
    assert campaign_inputs, "request has no --owner-base"
    argv[0] = str(args.binary.resolve())
    replaced = {"--output": out / "result.json", "--events": out / "events.jsonl",
                "--stop-file": out / "stop-request.json"}
    for i, a in enumerate(argv):
        if a in replaced:
            argv[i + 1] = str(replaced[a])
        elif a in ("--manifest", "--queries", "--owner-base"):
            assert argv[i + 1].startswith(campaign_inputs), argv[i + 1]
            argv[i + 1] = str(args.inputs.resolve()) + argv[i + 1][len(campaign_inputs):]
        elif a == "--checkpoint":
            argv[i] = "--resume"
            argv[i + 1] = str(args.checkpoint.resolve())
    bad = [a for a in argv[1:] if CAMPAIGNS in a]
    if bad or "--resume" not in argv:
        sys.exit(f"refusing argv: campaign paths {bad} or no --resume")
    stop_file = out / "stop-request.json"
    if stop_file.exists():
        sys.exit("stop file exists")
    cpus = cpu_list(args.cpus)
    meta = {"argv": argv, "request_source": str(args.request), "binary": str(args.binary),
            "binary_sha256": sha256(args.binary), "cpus": args.cpus, "perf": args.perf,
            "perf_cpus": args.perf_cpus, "args": {k: str(v) for k, v in vars(args).items()},
            "host_meminfo_before": meminfo(), "loadavg_before": open("/proc/loadavg").read().split()[:3],
            "uname": os.uname().release}
    probe = subprocess.run([str(args.binary), "walk-semantics-version"], capture_output=True, text=True)
    meta["walk_semantics_probe"] = {"code": probe.returncode, "stdout": probe.stdout.strip(),
                                    "stderr": probe.stderr.strip()[-400:]}
    s0 = read_cpu_stat(cpus)
    time.sleep(5)
    s1 = read_cpu_stat(cpus)
    total = (s1["busy_jiffies"] - s0["busy_jiffies"]) + (s1["idle_jiffies"] - s0["idle_jiffies"])
    meta["foreign_busy_cpus_before"] = (s1["busy_jiffies"] - s0["busy_jiffies"]) / HZ / 5.0
    meta["pinned_cpu_busy_share_before"] = (s1["busy_jiffies"] - s0["busy_jiffies"]) / max(total, 1)
    json.dump(meta, open(out / "meta.json", "w"), indent=1)
    if args.dry_run:
        print(json.dumps(meta, indent=1))
        return

    stdout = open(out / "native.stdout", "w")
    stderr = open(out / "native.stderr", "w")
    launch = time.time()
    # Production environment (shared_owner_campaign.py): inner pools at 1 thread,
    # banner hidden, diagnostic seams removed; the license comes from the caller.
    env = dict(os.environ)
    env.pop("RUSTRED_WALK_DIAGNOSTIC_PAUSE", None)
    env.update({name: "1" for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                                       "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")})
    env["SYMBOLICA_HIDE_BANNER"] = "1"
    meta["license_set"] = bool(env.get("SYMBOLICA_LICENSE"))
    json.dump(meta, open(out / "meta.json", "w"), indent=1)
    proc = subprocess.Popen(argv, stdout=stdout, stderr=stderr, cwd=str(out), env=env,
                            preexec_fn=lambda: os.sched_setaffinity(0, cpus))
    pid = proc.pid
    (out / "native.pid").write_text(f"{pid}\n")
    timeline = {"launch_unix_time": launch, "pid": pid}
    heartbeats = open(out / "heartbeats.jsonl", "w")
    tail = EventTail(out / "events.jsonl", heartbeats)
    samples = open(out / "samples.jsonl", "w")
    perf_base = ["nice", "-n", "5", "taskset", "-c", args.perf_cpus, args.perf]
    pending = {"early": args.early_offset, "dwarf": args.dwarf_offset, "late": args.late_offset,
               "numa": args.numa_offset}
    running = []  # (tag, [procs], started)
    windows = {}
    stop_reason = None
    last_cpu = read_cpu_stat(cpus)
    last_own = None
    last_time = time.time()

    def own_cpu(threads):
        return sum(r.get("utime_s", 0) + r.get("stime_s", 0) for r in threads.values())

    def snapshot(tag, threads, status):
        doc = {"tag": tag, "unix_time": time.time(), "status": status,
               "heartbeat": tail.last, "threads": {str(k): v for k, v in threads.items()}}
        (out / f"sched-{tag}.json").write_text(json.dumps(doc))

    def request_stop(reason):
        nonlocal stop_reason
        if stop_reason is None:
            stop_reason = reason
            stop_file.write_text(json.dumps({"reason": reason, "unix_time": time.time(),
                                             "family_closure_claim": False}))
            timeline["stop_request_unix_time"] = time.time()
            timeline["stop_reason"] = reason

    while True:
        code = proc.poll()
        now = time.time()
        tail.poll()
        threads = read_threads(pid) if code is None else {}
        status = read_status(pid) if code is None else {}
        mem = meminfo()
        cpu = read_cpu_stat(cpus)
        own = own_cpu(threads) if threads else None
        foreign = None
        if last_own is not None and own is not None and now > last_time:
            foreign = ((cpu["busy_jiffies"] - last_cpu["busy_jiffies"]) / HZ - (own - last_own)) / (now - last_time)
        row = {"unix_time": now, "since_launch": now - launch, "status": status,
               "mem_available": mem.get("MemAvailable"), "loadavg": open("/proc/loadavg").read().split()[:3],
               "cpu_stat": cpu, "own_cpu_seconds": own, "foreign_busy_cpus_estimate": foreign,
               "threads": {str(k): v for k, v in threads.items()}}
        samples.write(json.dumps(row) + "\n")
        samples.flush()
        last_cpu, last_own, last_time = cpu, own, now
        if code is not None:
            timeline["exit_unix_time"] = now
            timeline["exit_code"] = code
            break
        try:
            if tail.restored is not None and "restored_unix_time" not in timeline:
                timeline["restored_unix_time"] = tail.restored
                snapshot("restored", threads, status)
            if tail.first_duty is not None and "traversal_unix_time" not in timeline:
                timeline["traversal_unix_time"] = tail.first_duty
                timeline["traversal_native_elapsed"] = tail.last and tail.last.get("elapsed_seconds")
                snapshot("traversal", threads, status)
            rss = status.get("VmRSS", 0) * 1024
            if rss > args.rss_kill_gib * 2**30:
                timeline["killed"] = f"rss {rss}"
                proc.send_signal(signal.SIGKILL)
            elif rss > args.rss_stop_gib * 2**30:
                request_stop(f"rss_guard_{rss}")
            if mem.get("MemAvailable", 1 << 62) < args.min_memavail_gib * 2**30:
                request_stop(f"host_memavailable_{mem.get('MemAvailable')}")
            if now - launch > args.budget_seconds:
                timeline["killed"] = "budget"
                proc.send_signal(signal.SIGKILL)
            if now - launch > args.budget_seconds - args.save_reserve_seconds:
                request_stop("budget_save_reserve")
            t0 = timeline.get("traversal_unix_time")
            if t0 is not None:
                if now - t0 >= args.run_seconds:
                    request_stop("run_seconds_elapsed")
                for tag, offset in list(pending.items()):
                    if now - t0 < offset or stop_reason is not None:
                        continue
                    del pending[tag]
                    if tag == "numa":
                        started = time.time()
                        try:
                            nodes = {}
                            for line in open(f"/proc/{pid}/numa_maps"):
                                for token in line.split()[2:]:
                                    if token.startswith("N") and "=" in token:
                                        node, pages = token[1:].split("=")
                                        if node.isdigit():
                                            nodes[node] = nodes.get(node, 0) + int(pages)
                            doc = {"pages_by_node": nodes, "seconds": time.time() - started,
                                   "unix_time": started, "status": read_status(pid), "heartbeat": tail.last}
                        except OSError as error:
                            doc = {"error": str(error)}
                        (out / "numa.json").write_text(json.dumps(doc, indent=1))
                        continue
                    snapshot(f"{tag}-start", threads, status)
                    procs = []
                    win = {"start_unix_time": time.time(), "heartbeat_start": tail.last}
                    if tag == "dwarf":
                        coord = [pid]
                        insp = pick(threads, "inspector", 1)
                        helper = pick(threads, "admission_helper", 1)
                        win["tids"] = {"coordinator": coord, "inspector": insp, "admission_helper": helper}
                        # one DWARF record at a time: the per-user budget fits one 256 KB buffer
                        chain = []
                        for cls, tids in win["tids"].items():
                            if not tids:
                                continue
                            secs = args.dwarf_seconds if cls == "coordinator" else max(5, args.dwarf_seconds // 2)
                            chain.append(" ".join(shlex.quote(x) for x in record_cmd(
                                perf_base, (64, 32, 16), "-F 99 -e cpu-clock:u --call-graph dwarf,8192",
                                ",".join(map(str, tids)), str(out / f"perf-dwarf-{cls}.data"), secs)))
                        cmd = ["bash", "-c", " ; ".join(chain)]
                        procs.append(subprocess.Popen(cmd, stdout=open(out / "perf-dwarf.stdout", "w"),
                                                      stderr=open(out / "perf-dwarf.stderr", "w")))
                    else:
                        coord = [pid]
                        insp = pick(threads, "inspector", 4)
                        helper = pick(threads, "admission_helper", 4)
                        win["tids"] = {"coordinator": coord, "inspector": insp, "admission_helper": helper}
                        win["comms"] = {str(t): threads[t]["comm"] for t in coord + insp + helper if t in threads}
                        for cls, tids in win["tids"].items():
                            if not tids:
                                continue
                            cmd = record_cmd(perf_base, (16, 8, 4), f"-F {args.frequency} -e cpu-clock:u --call-graph fp",
                                             ",".join(map(str, tids)), str(out / f"perf-{tag}-{cls}.data"),
                                             args.window_seconds)
                            procs.append(subprocess.Popen(cmd, stdout=open(out / f"perf-{tag}-{cls}.stdout", "w"),
                                                          stderr=open(out / f"perf-{tag}-{cls}.stderr", "w")))
                        cmd = perf_base + ["stat", "--per-thread", "-p", str(pid), "-e", STAT_EVENTS, "-x", ",",
                                           "-o", str(out / f"perf-stat-{tag}.csv"), "--", "sleep", str(args.window_seconds)]
                        procs.append(subprocess.Popen(cmd, stdout=open(out / f"perf-stat-{tag}.stdout", "w"),
                                                      stderr=open(out / f"perf-stat-{tag}.stderr", "w")))
                    win["commands"] = [pr.args for pr in procs]
                    windows[tag] = win
                    running.append((tag, procs))
            for tag, procs in list(running):
                if all(pr.poll() is not None for pr in procs):
                    running.remove((tag, procs))
                    windows[tag]["end_unix_time"] = time.time()
                    windows[tag]["heartbeat_end"] = tail.last
                    windows[tag]["exit_codes"] = [pr.returncode for pr in procs]
                    snapshot(f"{tag}-end", read_threads(pid), read_status(pid))
            json.dump({"timeline": timeline, "windows": windows}, open(out / "timeline.json", "w"), indent=1)
        except Exception:  # never lose control of a socket-1 run to a monitoring bug
            import traceback
            with open(out / "harness-errors.log", "a") as log:
                log.write(f"{time.time()}\n{traceback.format_exc()}\n")
            request_stop("harness_error")
        time.sleep(args.interval)

    for tag, procs in running:
        for pr in procs:
            pr.wait()
        windows[tag]["note"] = "process exited during window"
        windows[tag]["exit_codes"] = [pr.returncode for pr in procs]
    time.sleep(1)
    tail.poll()
    try:
        ru = os.wait4(pid, os.WNOHANG)
    except ChildProcessError:
        ru = None
    timeline["wall_seconds"] = time.time() - launch
    timeline["host_meminfo_after"] = meminfo()
    json.dump({"timeline": timeline, "windows": windows, "markers": tail.markers},
              open(out / "timeline.json", "w"), indent=1)
    print(json.dumps(timeline, indent=1))


if __name__ == "__main__":
    main()
