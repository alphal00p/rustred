"""Resource supervision for native generation, using the campaign's RAM service.

Unlike the owner walker, family-candidates cannot save a live sector on request.
A guarded stop preserves only previously completed native sector checkpoints.
"""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


SUPERVISOR = module("selected_generation_resources", ROOT / "examples/python/shared_owner_campaign.py")


def native_environment():
    environment = os.environ.copy()
    for name in SUPERVISOR.INNER_POOLS:
        environment[name] = "1"
    environment["SYMBOLICA_HIDE_BANNER"] = "1"
    return environment


def group_running(group):
    for entry in Path("/proc").iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            fields = (entry / "stat").read_text().rsplit(")", 1)[1].split()
            if int(fields[2]) == group and fields[0] != "Z":
                return True
        except (OSError, ValueError, IndexError):
            continue
    return False


def run(command, directory, resources, *, poll_seconds=2.0, grace_seconds=10.0):
    """No deadline. Own process-group drain, finite RSS/host guards, no CAS."""
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=False)
    try:
        cpus = SUPERVISOR.parse_cpu_set(resources["cpus"])
        if len(cpus) != resources["workers"] or not cpus <= os.sched_getaffinity(0):
            raise ValueError("exactly workers permitted CPUs required")
        host = SUPERVISOR.host_memory()
        hard, soft, floor = SUPERVISOR.memory_admission(resources["max_memory_bytes"], None, host,
            resources["host_memory_reserve_bytes"], resources["ram_guard_margin_percent"])
        guard = SUPERVISOR.RamGuard(hard, soft, floor)
    except (OSError, ValueError, KeyError) as error:
        (directory / "result.json").write_text(json.dumps(dict(exit_code=None, child_started=False,
            owned_group_drained=True, stop_reason=f"resource_admission_failed: {error}"), indent=2) + "\n")
        raise
    request = dict(command=command, cpus=sorted(cpus), started_unix=time.time(),
        memory=SUPERVISOR.memory_admission_record(resources["max_memory_bytes"], host, floor, hard, soft),
        stop_contract="SIGTERM then owned-group drain; only completed sector shards are resumable",
        hard_timeout=None, environment_recorded=False)
    (directory / "request.json").write_text(json.dumps(request, indent=2) + "\n")
    reason, stopping, child = None, None, None

    def send(sig):
        if child is not None:
            try:
                os.killpg(child.pid, sig)
            except ProcessLookupError:
                pass

    def stop(why):
        nonlocal reason, stopping
        if reason is None:
            reason, stopping = why, time.monotonic()
            send(signal.SIGTERM)

    previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
    for sig in previous:
        signal.signal(sig, lambda number, frame: stop(f"operator_signal_{number}"))
    environment = native_environment()
    started, peak, last_print, cpu_previous, sample_previous = time.monotonic(), 0, 0, {}, None
    result, failure = None, None
    checkpoint = (Path(command[command.index("--checkpoint-dir")+1])
                  if "--checkpoint-dir" in command else None)
    try:
        collector = SUPERVISOR.ProcessTreeCollector()
        collector.register(os.getpid())
        with (directory / "stdout").open("xb") as stdout, (directory / "stderr").open("xb") as stderr, \
                (directory / "resources.jsonl").open("x") as telemetry:
            child = subprocess.Popen(command, cwd=ROOT, env=environment, stdin=subprocess.DEVNULL,
                stdout=stdout, stderr=stderr, start_new_session=True,
                preexec_fn=lambda: os.sched_setaffinity(0, cpus))
            (directory / "child.json").write_text(json.dumps(dict(pid=child.pid, process_group=child.pid)) + "\n")
            try:
                collector.register(child.pid)
            except (OSError, ValueError):
                pass  # A very short command can exit before its first sample.
            if reason:
                send(signal.SIGTERM)
            while child.poll() is None or group_running(child.pid):
                now = time.monotonic()
                try:
                    table, diagnostics = collector.sample()
                    rss = sum(row["rss_bytes"] for row in table.values())
                    peak = max(peak, rss)
                    host = SUPERVISOR.host_memory()
                    decision = guard.observe(now, rss, host["available_bytes"], SUPERVISOR.tree_swap_bytes(table))
                    delta, cpu_previous, processes = SUPERVISOR.process_cpu_sample(table, cpu_previous,
                        None if sample_previous is None else now - sample_previous, os.getpid(), child.pid)
                    busy = None if sample_previous is None else delta / (now - sample_previous)
                    sample_previous = now
                    telemetry.write(json.dumps(dict(elapsed_seconds=now-started, rss_bytes=rss,
                        observed_cores=busy, host_available_bytes=host["available_bytes"], guard=decision,
                        processes=processes, sampling=diagnostics)) + "\n")
                    telemetry.flush()
                    if decision["hard"]:
                        stop(decision["hard"])
                        send(signal.SIGKILL)
                    elif decision["cooperative"]:
                        stop(decision["cooperative"])
                    if now - last_print >= 30:
                        colour = "\033[36m" if os.isatty(1) else ""
                        reset = "\033[0m" if colour else ""
                        cores = "sampling" if busy is None else f"{busy:.2f}/{resources['workers']} cores"
                        print(f"{colour}Generation/admission{reset} {now-started:.0f}s | {cores} | "
                              f"RSS {rss/1e9:.2f}/{hard/1e9:.2f} GB | {directory}", flush=True)
                        if checkpoint is not None:
                            count = sum(1 for _ in checkpoint.glob("sector-*.rrbin"))
                            print(f"  {checkpoint.parent.name}: {count} completed-sector checkpoints; "
                                  "the active sector is not checkpointed", flush=True)
                        with (directory / "stderr").open("rb") as progress:
                            progress.seek(max(0, progress.seek(0, 2)-4096))
                            lines = progress.read().decode("utf-8", errors="replace").replace("\r", "\n").splitlines()
                        if lines:
                            print(f"  Native: {lines[-1][:1000]}", flush=True)
                        last_print = now
                except (OSError, ValueError, KeyError) as error:
                    stop(f"resource_monitor_failed: {error}")
                if stopping is not None and now - stopping >= grace_seconds:
                    send(signal.SIGKILL)
                time.sleep(poll_seconds)
            code = child.wait()
            result = dict(exit_code=code, stop_reason=reason, elapsed_seconds=time.monotonic()-started,
                          sampled_peak_tree_rss_bytes=peak, owned_group_drained=True,
                          completed_sector_checkpoint_only=True, in_sector_resume=False)
    except BaseException as error:
        failure = error
        stop(f"supervisor_exception: {type(error).__name__}: {error}")
    finally:
        if child is not None and (child.poll() is None or group_running(child.pid)):
            send(signal.SIGKILL)
            child.wait()
            while group_running(child.pid):
                time.sleep(0.1)
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    if result is None:
        result = dict(exit_code=None if child is None else child.returncode, stop_reason=reason,
                      elapsed_seconds=time.monotonic()-started, sampled_peak_tree_rss_bytes=peak,
                      owned_group_drained=child is None or not group_running(child.pid),
                      child_started=child is not None, completed_sector_checkpoint_only=True, in_sector_resume=False)
    (directory / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    if failure is not None:
        raise RuntimeError(f"supervisor failed after draining owned children: {directory / 'result.json'}") from failure
    if result["exit_code"] != 0 or reason is not None:
        raise RuntimeError(f"native phase stopped/failed: {directory / 'result.json'}; "
                           "completed sector shards are kept; an unfinished sector must restart")
    return result
