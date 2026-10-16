"""Resource supervision for native generation, using the campaign's RAM service.

Unlike the owner walker, family-candidates cannot save a live sector on request.
A guarded stop preserves only previously completed native sector checkpoints.
"""
import importlib.util
from contextlib import ExitStack
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


def terminal_input():
    # stderr may itself be write-only (e.g. `2>/dev/tty`). Reopen the same
    # device for cursor replies; never acquire/steal a controlling terminal.
    return open(os.ttyname(2), "rb", buffering=0,
                opener=lambda path, flags: os.open(path, flags | os.O_NOCTTY))


def run(command, directory, resources, *, poll_seconds=2.0, grace_seconds=10.0,
        cooperative_stop_file=None, native_tty_progress=False):
    """Own process-group drain and finite RSS/host guards, without CAS.

    Default generation children receive SIGTERM. A caller supervising a native
    owner walk may instead supply its stop-file in this new job's directory or
    immediate parent. Such walks poll the file but do not handle SIGTERM: give
    them the full grace before owned-group SIGKILL. No checkpoint success is
    inferred from requesting a stop. The read-only cold verifier has no stop-file.

    Opted-in terminal callers connect all native standard descriptors to the
    caller's stderr terminal. Crossterm's inline viewport reads terminal replies
    on stdin and writes its cursor-position query on stdout, even when rendering
    to stderr. Merely inheriting stderr makes that viewport silently fail under
    the isolated session. This mode is only for commands with structured output
    in separate files: the guard's stdout/stderr captures remain empty. Resource
    telemetry and all stop/drain guards remain active. Non-TTY and default
    callers keep file capture and the periodic guard summary.
    """
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=False)
    checkpoint_metadata = ({"completed_sector_checkpoint_only": None, "in_sector_resume": None,
                            "stop_mechanism": "native_stop_file_then_owned_group_cleanup",
                            "checkpoint_evidence_required": True, "checkpoint_durability_claim": False}
                           if cooperative_stop_file is not None else
                           {"completed_sector_checkpoint_only": True, "in_sector_resume": False})
    try:
        if cooperative_stop_file is not None:
            cooperative_stop_file = Path(cooperative_stop_file).absolute()
            if cooperative_stop_file.parent.resolve() not in (directory.resolve(), directory.parent.resolve()):
                raise ValueError("native stop-file must be local to this owned job")
        cpus = SUPERVISOR.parse_cpu_set(resources["cpus"])
        if len(cpus) != resources["workers"] or not cpus <= os.sched_getaffinity(0):
            raise ValueError("exactly workers permitted CPUs required")
        host = SUPERVISOR.memory_host_snapshot(resources)
        hard, soft, floor = SUPERVISOR.memory_admission_for_resources(resources, host)
        guard = SUPERVISOR.RamGuard(hard, soft, floor,
            swap_growth_bytes_per_second=resources.get("swap_growth_stop_bytes_per_second",
                SUPERVISOR.DEFAULT_SWAP_GROWTH_STOP_BYTES_PER_SECOND),
            swap_growth_seconds=resources.get("swap_growth_stop_seconds",
                SUPERVISOR.DEFAULT_SWAP_GROWTH_STOP_SECONDS))
    except (OSError, ValueError, TypeError, KeyError) as error:
        (directory / "result.json").write_text(json.dumps(dict(exit_code=None, child_started=False,
            owned_group_drained=True, stop_reason=f"resource_admission_failed: {error}"), indent=2) + "\n")
        raise
    native_tty = native_tty_progress and os.isatty(2)
    request = dict(command=command, cpus=sorted(cpus), started_unix=time.time(),
        memory=SUPERVISOR.memory_admission_record(resources["max_memory_bytes"], host, floor, hard, soft,
            resources.get("memory_policy", "host-aware")),
        ram_guard=guard.policy(),
        stop_contract=("native stop-file then bounded owned-group cleanup; native results alone attest checkpoint durability"
                       if cooperative_stop_file is not None else
                       "SIGTERM then owned-group drain; only completed sector shards are resumable"),
        hard_timeout=None, environment_recorded=False,
        native_tty_progress_requested=bool(native_tty_progress),
        native_stderr="inherited_tty" if native_tty else "captured_file",
        native_stdin="stderr_tty" if native_tty else "devnull",
        native_stdout="stderr_tty" if native_tty else "captured_file")
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
            if cooperative_stop_file is not None:
                try:
                    with cooperative_stop_file.open("x") as stream:
                        json.dump({"reason": why, "requested_unix": time.time()}, stream)
                        stream.write("\n")
                        stream.flush()
                except FileExistsError:
                    pass  # An existing owned stop request already cancels work.
                except OSError:
                    reason += ":stop_file_unavailable"
                    stopping -= grace_seconds
                    send(signal.SIGKILL)
            else:
                send(signal.SIGTERM)

    previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
    for sig in previous:
        signal.signal(sig, lambda number, frame: stop(f"operator_signal_{number}"))
    environment = native_environment()
    environment["RUSTRED_PROGRESS_MEMORY_LIMIT_BYTES"] = str(hard)
    environment["RUSTRED_PROGRESS_MEMORY_SOFT_BYTES"] = str(soft)
    started, peak, last_print, cpu_previous, sample_previous = time.monotonic(), 0, 0, {}, None
    result, failure = None, None
    checkpoint = (Path(command[command.index("--checkpoint-dir")+1])
                  if "--checkpoint-dir" in command else None)
    try:
        collector = SUPERVISOR.ProcessTreeCollector()
        collector.register(os.getpid())
        with (directory / "stdout").open("xb") as stdout, (directory / "stderr").open("xb") as stderr, \
                (directory / "resources.jsonl").open("x") as telemetry, ExitStack() as descriptors:
            native_input = subprocess.DEVNULL
            if native_tty:
                try:
                    native_input = descriptors.enter_context(terminal_input())
                except OSError as error:
                    # Losing interactive display must not prevent a reduction.
                    # Fall back to captured output and the periodic summary.
                    native_tty = False
                    request.update(native_stdin="devnull", native_stdout="captured_file",
                                   native_stderr="captured_file", native_tty_error=str(error))
                    (directory / "request.json").write_text(json.dumps(request, indent=2) + "\n")
                    print(f"RustRed: native terminal unavailable ({error}); using periodic progress.", flush=True)
            child = subprocess.Popen(command, cwd=ROOT, env=environment,
                stdin=native_input,
                stdout=2 if native_tty else stdout, stderr=None if native_tty else stderr,
                start_new_session=True,
                preexec_fn=lambda: os.sched_setaffinity(0, cpus))
            (directory / "child.json").write_text(json.dumps(dict(pid=child.pid, process_group=child.pid)) + "\n")
            try:
                collector.register(child.pid)
            except (OSError, ValueError):
                pass  # A very short command can exit before its first sample.
            if reason and cooperative_stop_file is None:
                send(signal.SIGTERM)
            while child.poll() is None or group_running(child.pid):
                now = time.monotonic()
                try:
                    table, diagnostics = collector.sample()
                    rss = sum(row["rss_bytes"] for row in table.values())
                    peak = max(peak, rss)
                    host = SUPERVISOR.memory_host_snapshot(resources)
                    decision = guard.observe(now, rss, host["available_bytes"], SUPERVISOR.tree_swap_bytes(table))
                    delta, cpu_previous, processes = SUPERVISOR.process_cpu_sample(table, cpu_previous,
                        None if sample_previous is None else now - sample_previous, os.getpid(), child.pid)
                    busy = None if sample_previous is None else delta / (now - sample_previous)
                    sample_previous = now
                    telemetry.write(json.dumps(dict(elapsed_seconds=now-started, rss_bytes=rss,
                        observed_cores=busy, host_available_bytes=host.get("host_available_bytes", host["available_bytes"]),
                        guard=decision,
                        processes=processes, sampling=diagnostics)) + "\n")
                    telemetry.flush()
                    if decision["hard"]:
                        stop(decision["hard"])
                        send(signal.SIGKILL)
                    elif decision["cooperative"]:
                        stop(decision["cooperative"])
                    if not native_tty and now - last_print >= 30:
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
                          **checkpoint_metadata)
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
                      child_started=child is not None, **checkpoint_metadata)
    (directory / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    if failure is not None:
        raise RuntimeError(f"supervisor failed after draining owned children: {directory / 'result.json'}") from failure
    if result["exit_code"] != 0 or reason is not None:
        raise RuntimeError(f"native phase stopped/failed: {directory / 'result.json'}; "
                           + ("inspect native checkpoint evidence; requesting a stop does not establish durable completion"
                              if cooperative_stop_file is not None else
                              "completed sector shards are kept; an unfinished sector must restart"))
    return result
