"""Optional two-phase campaign steering. Native Rust owns all mathematical state.

Phase one retains its CP6 worklist. Phase two has a separate checkpoint and
bounded, exact terminal-relation artifact. This module only binds scopes,
launches owned processes and feeds the generic telemetry/dashboard consumers.
"""
from __future__ import annotations

from contextlib import ExitStack, contextmanager
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import signal
import shutil
import subprocess
import sys
import tempfile
import time

SCHEMA = "rustred.campaign-phases.v1"
MAX_JSON_BYTES = 16 * 1024 * 1024


def read_json(path):
    with Path(path).open("rb") as stream:
        raw = stream.read(MAX_JSON_BYTES + 1)
    if len(raw) > MAX_JSON_BYTES:
        raise ValueError(f"bounded campaign metadata exceeds 16 MiB: {path}")
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise ValueError(f"campaign metadata must be an object: {path}")
    return value


def digest(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        while block := stream.read(1024 * 1024):
            value.update(block)
    return value.hexdigest()


def configuration(campaign, enabled=False, seed_depth=None, executable=None):
    """Read-only configuration; an existing opt-in survives ordinary resumes."""
    path = Path(campaign) / "master-reduction" / "policy.json"
    existing = read_json(path) if path.is_file() else None
    if seed_depth is not None and (type(seed_depth) is not int or seed_depth < 0):
        raise ValueError("master seed depth must be nonnegative")
    if existing is not None:
        if (existing.get("schema") != SCHEMA or existing.get("enabled") is not True
                or type(existing.get("seed_depth")) is not int or existing["seed_depth"] < 0):
            raise ValueError("invalid persisted master-reduction policy")
        if seed_depth is not None and seed_depth < existing["seed_depth"]:
            raise ValueError("cannot lower the previous master seed depth")
        if executable is not None and digest(executable) != existing.get("executable", {}).get("sha256"):
            raise ValueError("the independently frozen master executable cannot be silently replaced")
        return {**existing, "seed_depth": existing["seed_depth"] if seed_depth is None else seed_depth}
    if not enabled:
        if seed_depth is not None or executable is not None:
            raise ValueError("master options require --master-reduction")
        return None
    policy = {"schema": SCHEMA, "enabled": True, "seed_depth": 0 if seed_depth is None else seed_depth,
            "terminal_policy": "bounded exact search; finite nonminimal basis permitted",
            "master_minimality_claim": False}
    if executable is not None:
        executable = Path(executable).resolve()
        if not executable.is_file() or not os.access(executable, os.X_OK):
            raise ValueError("master reduction executable must be an executable file")
        sha = digest(executable)
        policy["executable"] = {"sha256": sha, "path": "bin/rustred-" + sha,
                                "source": str(executable)}
    return policy


def freeze_master_executable(directory, policy, driver):
    """Optional phase-two binary, deliberately separate from the phase-one build."""
    executable = policy.get("executable")
    if executable is None:
        return
    sha = executable.get("sha256")
    if (not isinstance(sha, str) or len(sha) != 64 or any(c not in "0123456789abcdef" for c in sha)
            or executable.get("path") != "bin/rustred-" + sha):
        raise ValueError("invalid independently frozen master executable identity")
    target = directory / executable["path"]
    if target.is_file():
        if digest(target) != sha:
            raise ValueError("frozen master executable contents changed")
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    source = Path(executable["source"])
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=target.parent, prefix=".freeze-", delete=False) as stream:
            temporary = Path(stream.name)
            with source.open("rb") as reader:
                shutil.copyfileobj(reader, stream, 1024 * 1024)
            stream.flush()
            os.fsync(stream.fileno())
        if digest(temporary) != sha:
            raise ValueError("master executable changed while being frozen")
        temporary.chmod(0o700)
        os.replace(temporary, target)
        driver.sync_directory(target.parent)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def _chain(campaign):
    return [{"name": path.name, "sha256": digest(path)}
            for path in sorted((Path(campaign) / "amendments").glob("amendment-*.json"))]


def scope_binding(campaign, checkpoint, seed_depth=0):
    """Cheap launch identity, not an alternative to Rust's cold verification."""
    campaign, checkpoint = Path(campaign), Path(checkpoint)
    latest = checkpoint / "latest.json"
    envelope = read_json(latest)
    manifest = envelope.get("manifest", envelope)
    if not isinstance(manifest, dict) or manifest.get("resumable") is not True:
        raise ValueError("master reduction requires a resumable native checkpoint")
    components = {"checkpoint_manifest_sha256": digest(latest), "amendments": _chain(campaign),
                  "selection_sha256": digest(campaign / "inputs/selection.json"),
                  "queries_sha256": digest(campaign / "inputs/queries.json"), "seed_depth": seed_depth}
    key = hashlib.sha256(json.dumps(components, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {"key": key, "components": components, "generation": manifest.get("generation"),
            "manifest_blake3": bytes(envelope["blake3"]).hex() if isinstance(envelope.get("blake3"), list) else None}


def _arguments(command, option):
    return [command[index + 1] for index, value in enumerate(command[:-1]) if value == option]


def completed_request(campaign, checkpoint, binding, driver):
    """Find a drained native receipt bound to this exact CP6 and amendment chain.

    This is only admission to the native phase-two cold verifier, not closure.
    In particular a cached root-closure progress bar is never consulted.
    """
    candidates = driver.campaign_runs(Path(campaign))
    for run in sorted(candidates, key=lambda path: path.name, reverse=True):
        try:
            result = read_json(run / "result.json")
            request = read_json(run / "request.json")
            command = request.get("command", [])
            cp = result.get("checkpoint", {})
            if (result.get("recursive_worklist_exhausted") is not True
                    or result.get("admission_complete") is not True
                    or result.get("admission_failure") is not None
                    or result.get("failed_nodes") != 0 or result.get("frontiers") != 0
                    or cp.get("state") != "saved" or cp.get("resumable") is not True
                    or cp.get("generation") != binding["generation"]
                    or not isinstance(command, list) or not all(isinstance(part, str) for part in command)
                    or len(command) < 2 or command[1] != "owner-domain-match"):
                continue
            if binding["manifest_blake3"] is not None and cp.get("manifest_blake3") != binding["manifest_blake3"]:
                continue
            if Path(cp.get("directory", "")).resolve() != Path(checkpoint).resolve():
                continue
            chain = [{"name": Path(path).name, "sha256": digest(path)}
                     for path in _arguments(command, "--amend-queries")]
            if chain != binding["components"]["amendments"]:
                continue
            queries, manifests = _arguments(command, "--queries"), _arguments(command, "--manifest")
            if (len(queries) != 1 or len(manifests) != 1
                    or digest(queries[0]) != binding["components"]["queries_sha256"]
                    or digest(manifests[0]) != binding["components"]["selection_sha256"]):
                continue
            return run / "request.json"
        except (OSError, ValueError, KeyError, TypeError):
            continue
    return None


@contextmanager
def dispatcher_lock(directory):
    directory.mkdir(parents=True, exist_ok=True)
    with (directory / "dispatcher.lock").open("a+b") as stream:
        try:
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("this campaign already has a running phase dispatcher") from None
        yield


def phase_one(command):
    """Forward operator signals to the existing supervisor, then wait for save."""
    stopped = False
    pending_signal = None
    child = None

    def stop(signum, _frame):
        nonlocal stopped, pending_signal
        stopped = True
        pending_signal = signum
        if child is not None and child.poll() is None:
            try:
                os.killpg(child.pid, signum)
            except ProcessLookupError:
                pass

    handlers = {sig: signal.signal(sig, stop) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        if stopped:
            return 4, True
        child = subprocess.Popen(command, start_new_session=True)
        if pending_signal is not None and child.poll() is None:
            try:
                os.killpg(child.pid, pending_signal)
            except ProcessLookupError:
                pass
        status = child.wait()
        return (status if status >= 0 else 128 - status), stopped
    finally:
        for sig, handler in handlers.items():
            signal.signal(sig, handler)


def phase_two(plan, policy, request, binding, directory, driver):
    """Native bounded relation search under the existing CPU and RAM policies."""
    supervisor = driver.SUPERVISOR
    telemetry, dashboard = supervisor.MONITOR.TELEMETRY, supervisor.MONITOR.DASHBOARD
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    attempt = directory / "runs" / timestamp
    attempt.mkdir(parents=True)
    events, stop_file = attempt / "events.jsonl", attempt / "stop-request.json"
    executable = driver.steering_executable(plan["steering_policy"])[1]
    if policy.get("executable"):
        executable = Path(plan["campaign_directory"]) / "master-reduction" / policy["executable"]["path"]
    command = [str(executable), "walk-master-reduce", "--command", str(request),
               "--checkpoint", plan["checkpoint_directory"], "--directory", str(directory),
               "--events", str(events), "--stop-file", str(stop_file),
               "--checkpoint-interval-seconds", str(plan["checkpoint_interval_seconds"]),
               "--threads", str(plan["requested_workers"]), "--seed-depth", str(policy["seed_depth"])]
    if (directory / "latest.json").is_file():
        command.append("--resume")
    else:
        previous_path = directory.parent.parent / "completed-phase.json"
        if previous_path.is_file():
            previous = read_json(previous_path)
            old = previous.get("scope_binding", {})
            if (old.get("key") != binding["key"]
                    and old.get("components", {}).get("selection_sha256") == binding["components"]["selection_sha256"]
                    and (Path(previous.get("directory", "")) / "artifact.json").is_file()):
                # Native authenticates the family/program binding and replays
                # retained exact rows against the newly enlarged terminal set.
                command += ["--previous-artifact", previous["directory"]]
    options = plan["supervisor_ram_policy"]
    cpus = supervisor.parse_cpu_set(plan["cpus"]) if plan["cpus"] else set(sorted(os.sched_getaffinity(0))[:plan["requested_workers"]])
    if len(cpus) != plan["requested_workers"] or not cpus <= os.sched_getaffinity(0):
        raise ValueError("master reduction requires the campaign's reserved, permitted CPU set")
    hard, soft, reserve = supervisor.memory_admission(options["max_memory_bytes"], None,
        supervisor.host_memory(), options.get("host_memory_reserve_bytes"), options["ram_guard_margin_percent"])
    guard = supervisor.RamGuard(hard, soft, reserve,
        options.get("swap_growth_stop_bytes_per_second") or 0,
        options.get("swap_growth_stop_seconds") or supervisor.DEFAULT_SWAP_GROWTH_STOP_SECONDS)
    environment = dict(os.environ, SYMBOLICA_HIDE_BANNER="1")
    for name in supervisor.INNER_POOLS:
        environment[name] = "1"  # Native --threads owns the outer compute budget.
    for name in supervisor.DIAGNOSTIC_ONLY_ENVIRONMENT:
        environment.pop(name, None)
    driver.write_json(attempt / "request.json", {"command": command, "cpus": sorted(cpus),
                      "scope_binding": binding, "phase": "Master reduction"})
    driver.write_json(directory.parent.parent / "active-phase.json", {
        "schema": SCHEMA, "phase": "Master reduction", "scope_binding": binding["key"],
        "directory": str(directory), "run_directory": str(attempt), "request": str(request)})
    presenter = dashboard.Presenter()
    stream = telemetry.TelemetryStream(attempt / "telemetry.jsonl")
    tail = supervisor.MONITOR.EventTail(events)
    latest = {"phase": "Master reduction", "stage": "cold closure verification", "status": "running",
              "seed_depth": policy["seed_depth"], "scope_binding": binding["key"]}
    checkpoint = {}

    def observe(event):
        nonlocal checkpoint
        payload = event.get("progress", event)
        if not isinstance(payload, dict):
            return
        if payload.get("event", "").startswith("master_reduction") or payload.get("phase") == "Master reduction":
            latest.update({key: value for key, value in payload.items() if value is not None})
            if isinstance(payload.get("checkpoint"), dict):
                checkpoint = dict(payload["checkpoint"])
                if "saved_unix_time" not in checkpoint:
                    checkpoint["saved_unix_time"] = checkpoint.get("completed_unix_seconds")
    tail.observers.append(observe)
    stop_reason = None

    def request_stop(reason):
        nonlocal stop_reason
        if stop_reason is None:
            stop_reason = str(reason)
            driver.write_json(stop_file, {"reason": stop_reason, "unix_time": time.time()})

    handlers = {sig: signal.signal(sig, lambda signum, _: request_stop(signal.Signals(signum).name))
                for sig in (signal.SIGINT, signal.SIGTERM)}
    collector = supervisor.ProcessTreeCollector()
    identities = {"supervisor": {"pid": os.getpid(), "start_ticks": collector._stat(os.getpid())["start"]}}
    try:
        identities["boot_id"] = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except OSError:
        identities["boot_id"] = None
    previous, previous_time, peak = {}, None, 0
    started = time.monotonic()
    last_resources = {}

    def publish(state, force=False):
        now = time.monotonic()
        tail.poll(now)
        status = {"schema": "campaign-status.v1", "state": state, "elapsed_seconds": now - started,
                  "heartbeat_unix_time": time.time(), "heartbeat_age_seconds": 0,
                  "process_identity": identities,
                  "run_directory": str(attempt), "workers": plan["requested_workers"],
                  "hard_memory_bytes": hard, "soft_memory_bytes": soft, "stop_reason": stop_reason,
                  "resources": last_resources, "progress": {"phase": "Master reduction"},
                  "master_reduction": latest, "checkpoint": checkpoint}
        frame = stream.emit(status)
        driver.write_json(attempt / "status.json", status)
        presenter.render_frame(frame, force=force)

    try:
        with (attempt / "stdout.json").open("wb") as stdout, (attempt / "stderr.log").open("wb") as stderr:
            with supervisor.owned_process(command, environment, cpus, request_stop,
                                          stdout=stdout, stderr=stderr) as child:
                collector.register(child.pid)
                identities["native"] = {"pid": child.pid, "start_ticks": collector.identities[child.pid]}
                while child.poll() is None:
                    now = time.monotonic()
                    rows, _ = collector.sample()
                    elapsed = None if previous_time is None else now - previous_time
                    cpu_seconds, previous, _ = supervisor.process_cpu_sample(rows, previous, elapsed, os.getpid(), child.pid)
                    rss = sum(row["rss_bytes"] for row in rows.values())
                    peak = max(peak, rss)
                    try:
                        host = supervisor.host_memory()
                    except (OSError, ValueError):
                        host = {"available_bytes": None}
                        request_stop("host_memory_monitor_unavailable")
                    decision = guard.observe(now, rss, host.get("available_bytes"), supervisor.tree_swap_bytes(rows),
                                             supervisor.read_swap_in_pages())
                    if decision["cooperative"]:
                        request_stop(decision["cooperative"])
                    if decision["hard"]:
                        request_stop(decision["hard"])
                        try:
                            os.killpg(child.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    last_resources = {"native_busy_cores": cpu_seconds / elapsed if elapsed else None,
                                      "aggregate_rss_bytes": rss, "peak_observed_rss_bytes": peak,
                                      "host_available_bytes": host.get("available_bytes")}
                    previous_time = now
                    publish("stopping" if stop_reason else "running")
                    time.sleep(.5)
                status = child.wait()
        tail.poll()
        # The CLI also uses exit 4 for input errors. Only the native event
        # attesting a cooperative pause justifies displaying a resumable pause.
        paused = status == 4 and latest.get("status") == "paused"
        publish("completed_nonminimal" if status == 0 else "paused" if paused else "failed", force=True)
        driver.write_json(attempt / "supervisor-result.json", {"exit_status": status, "stop_reason": stop_reason,
                          "phase": "Master reduction", "scope_binding": binding["key"],
                          "peak_observed_rss_bytes": peak, "elapsed_seconds": time.monotonic() - started})
        if status == 0:
            driver.write_json(directory.parent.parent / "completed-phase.json", {
                "schema": SCHEMA, "directory": str(directory), "scope_binding": binding,
                "completed_unix_time": time.time(), "master_minimality_claim": False})
            print("Master reduction completed for the configured bounded search (not a minimality proof).", flush=True)
            if latest.get("artifact"):
                print("Symbolic artifact: " + str(latest["artifact"]), flush=True)
        else:
            print(f"Master reduction {'paused' if paused else 'stopped'}; checkpoint: {directory}", flush=True)
            print(f"Resume with: {sys.executable} {Path(driver.__file__).resolve()} --campaign-directory "
                  f"{plan['campaign_directory']} --resume --start", flush=True)
            if not paused:
                print(f"Native error details: {attempt / 'stderr.log'}", file=sys.stderr)
        return status if status >= 0 else 128 - status
    finally:
        for sig, handler in handlers.items():
            signal.signal(sig, handler)


def run(plan, policy, resume, driver):
    campaign, checkpoint = Path(plan["campaign_directory"]), Path(plan["checkpoint_directory"])
    directory = campaign / "master-reduction"
    with ExitStack() as held:
        # Same acquisition order as the rank-extension helper. Creating the
        # first dispatcher.lock is serialized against an existing CP6 lock,
        # closing the first-opt-in race with an amendment publication. Release
        # CP6 before starting a native walk; retain the phase lock throughout.
        with driver.checkpoint_lock(checkpoint):
            held.enter_context(dispatcher_lock(directory))
            evidence = driver.campaign_run_liveness(campaign)
            if evidence:
                raise ValueError("campaign is still running: " + "; ".join(evidence))
            freeze_master_executable(directory, policy, driver)
            driver.write_json(directory / "policy.json", policy)
            binding = scope_binding(campaign, checkpoint, policy["seed_depth"]) if (checkpoint / "latest.json").is_file() else None
            request = completed_request(campaign, checkpoint, binding, driver) if resume and binding else None
        if request is None:
            driver.write_json(campaign / "active-run.json", plan)
            driver.write_json(directory / "active-phase.json", {"schema": SCHEMA, "phase": "IBP closure",
                              "run_directory": plan["run_directory"]})
            status, stopped = phase_one(plan["command"])
            if stopped:
                return 4  # Native phase one may have finished just before Ctrl+C.
            if status not in (0, 4):
                return status
            binding = scope_binding(campaign, checkpoint, policy["seed_depth"]) if (checkpoint / "latest.json").is_file() else None
            request = completed_request(campaign, checkpoint, binding, driver) if binding else None
            if request is None:
                print("IBP campaign remains paused/incomplete; master reduction has not started.", flush=True)
                return status if status else 4
        target = directory / "scopes" / binding["key"]
        target.mkdir(parents=True, exist_ok=True)
        driver.write_json(target / "scope.json", binding)
        return phase_two(plan, policy, request, binding, target, driver)
