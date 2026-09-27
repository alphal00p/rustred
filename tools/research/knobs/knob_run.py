#!/usr/bin/env python
"""W0.8 knob runner (fable_5_1 next push): one owner-domain walk control with
per-thread CPU accounting.

Reuses the historical control command lines of TMP/fable51-controls/run_control.py
(four-loop FG/BMW/H/X, five-loop 1,324-tuple finite control) plus the hot-owner
sub-box family `hot-sub` (queries file given with --queries). The binary runs
directly (no `nix develop`), pinned with sched_setaffinity and niced.

Measured per run (written to <out>/metrics.json):
  - wall, rusage CPU of the native process (wait4);
  - inspector CPU: sum over threads named `owner-domain-*` of
    /proc/<pid>/task/<tid>/schedstat on-CPU ns (last sample before exit),
    plus their run-queue delay; a 1 Hz time series in <out>/cpu.jsonl;
  - foreign load on the run's CPUs from /proc/stat (busy minus own CPU);
  - peak RSS; counters from result.json (head/tail scan) including the
    `parallel.dispatch_order` telemetry of the W0.8 build.

Usage:
  knob_run.py --binary BIN --family fg --label NAME --cpus 118-123
              [--policy ordered|ready] [--workers N] [--env K=V ...]
              [--queries Q.json] [--timeout-seconds S] [--keep-result]
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
COMMANDS = {
    "fg": ROOT / "TMP/four-loop-saved-descendants.VaNmUN/fg/command-rank12orthant.json",
    "bmw": ROOT / "TMP/four-loop-saved-descendants.VaNmUN/bmw/command-upstream-a19.json",
    "h": ROOT / "TMP/four-loop-saved-descendants.VaNmUN/h/command-rank12orthant.json",
    "x": ROOT / "TMP/four-loop-saved-descendants.VaNmUN/x/command-rank12orthant.json",
    "five-finite": ROOT / "TMP/ready-five-loop-finite-w50.a6ABXd/ready-first/command.json",
    "hot-sub": ROOT / "TMP/qcd-feynman-d9d10-pilot-hot-owner/matrix-32fdec/hot-owner-physics-ready/run/request.json",
}
OUT_ROOT = Path(os.environ.get("KNOB_OUT_ROOT", ROOT / "TMP/w0/knobs/runs"))
ENV_ONE = {
    "RAYON_NUM_THREADS": "1", "OMP_NUM_THREADS": "1", "OMP_THREAD_LIMIT": "1",
    "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1", "BLIS_NUM_THREADS": "1",
}
RETIRED = ROOT / "TMP/retired-campaigns-20260925.UtI4ay"
BUSY_FIELDS = (0, 1, 2, 5, 6, 7)  # user nice system irq softirq steal
CLK_TCK = os.sysconf("SC_CLK_TCK")


def cpu_list(spec):
    cpus = []
    for part in spec.split(","):
        if "-" in part:
            a, b = part.split("-")
            cpus.extend(range(int(a), int(b) + 1))
        else:
            cpus.append(int(part))
    return cpus


def load_argv(family):
    data = json.load(open(COMMANDS[family]))
    return list(data["command"] if isinstance(data, dict) else data)


def rewrite(argv, binary, out, args):
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
    if args.policy:
        set_opt("--publication-policy", args.policy)
    if args.workers:
        set_opt("--workers", args.workers)
    if args.queries:
        queries = Path(args.queries).resolve()
        set_opt("--queries", queries)
        set_opt("--max-query-bytes", queries.stat().st_size)
        set_opt("--max-queries", len(json.load(open(queries))["queries"]))
    if args.family == "five-finite":
        for i, a in enumerate(argv):
            if "campaigns/five-loop-saved-coarse-cover/inputs" in a:
                argv[i] = a.replace(str(ROOT / "campaigns/five-loop-saved-coarse-cover/inputs"),
                                    str(RETIRED / "five-loop-saved-coarse-cover/inputs"))
    argv.extend(args.extra)
    return argv


def proc_stat_cpus(cpus):
    busy, total = 0, 0
    wanted = {f"cpu{c}" for c in cpus}
    with open("/proc/stat") as f:
        for line in f:
            parts = line.split()
            if parts and parts[0] in wanted:
                values = [int(v) for v in parts[1:]]
                busy += sum(values[i] for i in BUSY_FIELDS if i < len(values))
                total += sum(values[:8])
    return busy, total


class Poller(threading.Thread):
    def __init__(self, pid, series_path, interval=0.25):
        super().__init__(daemon=True)
        self.pid = pid
        self.interval = interval
        self.stop = threading.Event()
        self.tasks = {}  # tid -> (comm, cpu_ns, delay_ns, slices)
        self.peak_rss = 0
        self.series = open(series_path, "w")
        self.t0 = time.time()

    def sample(self):
        base = f"/proc/{self.pid}"
        try:
            with open(base + "/status") as f:
                for line in f:
                    if line.startswith("VmRSS:"):
                        self.peak_rss = max(self.peak_rss, int(line.split()[1]) * 1024)
                        break
            tids = os.listdir(base + "/task")
        except OSError:
            return False
        for tid in tids:
            try:
                with open(f"{base}/task/{tid}/comm") as f:
                    comm = f.read().strip()
                with open(f"{base}/task/{tid}/schedstat") as f:
                    cpu, delay, slices = (int(v) for v in f.read().split()[:3])
            except (OSError, ValueError):
                continue
            self.tasks[tid] = (comm, cpu, delay, slices)
        return True

    def totals(self):
        insp = [v for v in self.tasks.values() if v[0].startswith("owner-domain-")]
        return {
            "inspector_threads": len(insp),
            "inspector_cpu_seconds": sum(v[1] for v in insp) / 1e9,
            "inspector_run_delay_seconds": sum(v[2] for v in insp) / 1e9,
            "all_threads_seen": len(self.tasks),
            "threads_cpu_seconds": sum(v[1] for v in self.tasks.values()) / 1e9,
            "threads_run_delay_seconds": sum(v[2] for v in self.tasks.values()) / 1e9,
        }

    def run(self):
        last_write = 0.0
        while not self.stop.is_set():
            alive = self.sample()
            now = time.time()
            if alive and now - last_write >= 1.0:
                row = {"t": round(now - self.t0, 3), "rss": self.peak_rss}
                row.update(self.totals())
                self.series.write(json.dumps(row) + "\n")
                self.series.flush()
                last_write = now
            self.stop.wait(self.interval)
        self.series.close()


TOP_KEYS = ("traversal_seconds", "prepared_seconds", "elapsed_seconds", "completed_nodes",
            "scheduled_nodes", "queued_nodes", "containment_checks",
            "containment_maintenance_checks", "containment_retired_candidates",
            "containment_semantic_hits", "containment_semantic_retirements",
            "successors", "events", "deduplication_hits", "route_masks", "routed_domains",
            "max_scheduled_finite_rank", "native_processed_nodes", "frontiers", "status",
            "recursive_worklist_exhausted", "error")
NESTED_KEYS = ("transferred_obligations", "delegated_publications", "native_publications",
               "all_ledger_obligations_discharged", "initial_closed", "initial_total",
               "total_closed", "total_domains")
NESTED_OBJECTS = ("dispatch_order", "coordinator_duty")


def scan_result(path):
    """Pretty-printed result.json (2-space indent, sorted keys): top-level
    keys sit at indent 2, delegation/descendant_closure/parallel members at
    indent 4. Only the head and tail are read (the domains array is huge)."""
    metrics = {}
    if not path.exists():
        return metrics
    size = path.stat().st_size
    with open(path, "rb") as f:
        head = f.read(min(size, 4_000_000)).decode("utf-8", "replace")
        tail = ""
        if size > 4_000_000:
            f.seek(max(0, size - 400_000))
            tail = f.read().decode("utf-8", "replace")
    value = r'("[^"]*"|[0-9.eE+-]+|null|true|false)'
    for indent, keys in (("  ", TOP_KEYS), ("    ", NESTED_KEYS)):
        for key in keys:
            for blob in (head, tail):
                m = re.search(r'\n%s"%s": %s' % (indent, key, value), blob)
                if m:
                    metrics[key] = json.loads(m.group(1))
                    break
    decoder = json.JSONDecoder()
    for key in NESTED_OBJECTS:
        marker = '\n    "%s": ' % key
        for blob in (tail, head):
            i = blob.find(marker)
            if i >= 0:
                try:
                    metrics[key], _ = decoder.raw_decode(blob, i + len(marker))
                except ValueError:
                    pass
                break
    return metrics


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--binary", required=True)
    p.add_argument("--family", required=True, choices=sorted(COMMANDS))
    p.add_argument("--label", required=True)
    p.add_argument("--cpus", required=True)
    p.add_argument("--policy")
    p.add_argument("--workers", type=int)
    p.add_argument("--queries")
    p.add_argument("--env", action="append", default=[])
    p.add_argument("--extra", nargs="*", default=[])
    p.add_argument("--nice", type=int, default=5)
    p.add_argument("--timeout-seconds", type=float)
    p.add_argument("--keep-result", action="store_true",
                   help="keep result.json even when larger than 2 GB")
    p.add_argument("--no-checkpoint", action="store_true")
    args = p.parse_args()
    out = OUT_ROOT / args.label / args.family
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    argv = rewrite(load_argv(args.family), args.binary, out, args)
    if args.no_checkpoint and "--checkpoint" in argv:
        i = argv.index("--checkpoint")
        del argv[i:i + 2]
        if "--checkpoint-interval-seconds" in argv:
            i = argv.index("--checkpoint-interval-seconds")
            del argv[i:i + 2]
    env = dict(os.environ)
    env.update(ENV_ONE)
    env["TMPDIR"] = str(ROOT / "TMP")
    extra_env = dict(item.split("=", 1) for item in args.env)
    env.update(extra_env)
    json.dump({"argv": argv, "env": extra_env, "cpus": args.cpus, "nice": args.nice},
              open(out / "command.json", "w"), indent=1)
    json.dump(argv, open(out / "argv.json", "w"), indent=1)
    cpus = cpu_list(args.cpus)
    stderr = open(out / "stderr", "w")
    stdout = open(out / "stdout", "w")
    stat0 = proc_stat_cpus(cpus)
    start = time.time()

    def pre():
        os.sched_setaffinity(0, cpus)
        os.nice(args.nice)

    proc = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=stdout, stderr=stderr,
                            preexec_fn=pre)
    poller = Poller(proc.pid, out / "cpu.jsonl")
    poller.start()
    timed_out = False
    while True:
        pid, status, rusage = os.wait4(proc.pid, os.WNOHANG)
        if pid == proc.pid:
            break
        if (args.timeout_seconds and not timed_out
                and time.time() - start > args.timeout_seconds):
            (out / "stop-request.json").write_text('{"reason":"knob_run timeout"}\n')
            timed_out = True
        time.sleep(0.2)
    wall = time.time() - start
    poller.stop.set()
    poller.join()
    stat1 = proc_stat_cpus(cpus)
    code = os.waitstatus_to_exitcode(status)
    own_cpu = rusage.ru_utime + rusage.ru_stime
    busy = (stat1[0] - stat0[0]) / CLK_TCK
    capacity = wall * len(cpus)
    metrics = {"family": args.family, "label": args.label, "binary": args.binary,
               "exit_code": code, "timed_out": timed_out, "whole_command_seconds": round(wall, 3),
               "cpus": args.cpus, "ncpus": len(cpus), "policy": args.policy,
               "workers": args.workers, "env": extra_env, "queries": args.queries,
               "rusage_cpu_seconds": own_cpu, "rusage_maxrss_bytes": rusage.ru_maxrss * 1024,
               "peak_rss_bytes": poller.peak_rss,
               "cpus_busy_seconds": busy,
               "foreign_cpu_seconds": max(0.0, busy - own_cpu),
               "foreign_load_fraction": max(0.0, busy - own_cpu) / capacity if capacity else None,
               "started_unix": start}
    metrics.update(poller.totals())
    metrics.update(scan_result(out / "result.json"))
    natives = metrics.get("native_publications") or metrics.get("native_processed_nodes")
    if natives:
        metrics["inspector_cpu_ms_per_native"] = 1e3 * metrics["inspector_cpu_seconds"] / natives
    json.dump(metrics, open(out / "metrics.json", "w"), indent=1)
    result = out / "result.json"
    if result.exists() and result.stat().st_size > 2_000_000_000 and not args.keep_result:
        result.unlink()
        (out / "result.json.deleted").write_text("result.json > 2 GB removed after metric scan\n")
    print(json.dumps({k: metrics.get(k) for k in (
        "family", "label", "exit_code", "whole_command_seconds", "native_publications",
        "inspector_cpu_seconds", "inspector_cpu_ms_per_native", "foreign_load_fraction",
        "frontiers", "initial_closed", "initial_total")}))


if __name__ == "__main__":
    main()
