"""Solve, publish, and explicitly refine a saved campaign.

Phase one retains its CP6 worklist. Publication and optional refinement have
separate checkpoints and portable symbolic artifacts. This module only binds scopes,
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
NORMALIZATION_PROFILES = {"conservative": "conservative-v1", "standard": "standard-v1"}
COLLECTION_STRATEGY = "full-u-diagonal-v1"
FINITE_FEEDBACK_RECIPE = "finite-row-feedback-v1"


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


def configuration(campaign, enabled=False, seed_depth=None, executable=None, saved_rule_assistance=None,
                  containing_sector_depth=None, circuit_symmetry_assistance=None, normalization_profile=None,
                  collection_artifacts=None, finite_feedback=None):
    """Read-only configuration. Only this invocation can request refinement.

    Historical ``enabled`` policies retain executable/search preferences, never
    authority to refine an ordinary solve or extension automatically.
    """
    path = Path(campaign) / "master-reduction" / "policy.json"
    existing = read_json(path) if path.is_file() else None
    if normalization_profile is not None and (not isinstance(normalization_profile, str)
                                               or normalization_profile not in NORMALIZATION_PROFILES):
        raise ValueError("master normalization profile must be conservative or standard")
    if seed_depth is not None and (type(seed_depth) is not int or seed_depth < 0):
        raise ValueError("master seed depth must be nonnegative")
    if saved_rule_assistance is not None and type(saved_rule_assistance) is not bool:
        raise ValueError("master saved-rule assistance must be boolean")
    if circuit_symmetry_assistance is not None and type(circuit_symmetry_assistance) is not bool:
        raise ValueError("master circuit-symmetry assistance must be boolean")
    if finite_feedback is not None and type(finite_feedback) is not bool:
        raise ValueError("master finite feedback must be boolean")
    if containing_sector_depth is not None and (type(containing_sector_depth) is not int or containing_sector_depth < 0):
        raise ValueError("master containing-sector depth must be nonnegative")
    if existing is not None:
        if (existing.get("schema") != SCHEMA or existing.get("enabled") is not True
                or type(existing.get("seed_depth")) is not int or existing["seed_depth"] < 0
                or type(existing.get("saved_rule_assistance", False)) is not bool
                or type(existing.get("circuit_symmetry_assistance", False)) is not bool
                or type(existing.get("finite_feedback", True)) is not bool
                or existing.get("normalization_profile") not in (None, *NORMALIZATION_PROFILES)
                or type(existing.get("containing_sector_depth", 0)) is not int
                or existing.get("containing_sector_depth", 0) < 0):
            raise ValueError("invalid persisted master-reduction policy")
        if enabled and seed_depth is not None and seed_depth < existing["seed_depth"]:
            raise ValueError("cannot lower the previous master seed depth")
        if (enabled and containing_sector_depth is not None
                and containing_sector_depth < existing.get("containing_sector_depth", 0)):
            raise ValueError("cannot lower the previous master containing-sector depth")
    if seed_depth is not None and not enabled:
        raise ValueError("master seed depth requires explicit --refine-masters")
    if saved_rule_assistance is not None and not enabled:
        raise ValueError("master saved-rule assistance requires explicit --refine-masters")
    if circuit_symmetry_assistance is not None and not enabled:
        raise ValueError("master circuit-symmetry assistance requires explicit --refine-masters")
    if finite_feedback is not None and not enabled:
        raise ValueError("master finite feedback requires explicit --refine-masters")
    if containing_sector_depth is not None and not enabled:
        raise ValueError("master containing-sector depth requires explicit --refine-masters")
    if normalization_profile is not None and not enabled:
        raise ValueError("master normalization profile requires explicit --refine-masters")
    if collection_artifacts is not None and not enabled:
        raise ValueError("master collection artifacts require explicit --refine-masters")
    members = ((existing or {}).get("collection_artifacts", []) if collection_artifacts is None
               else [str(Path(path).resolve()) for path in collection_artifacts])
    if not isinstance(members, list) or any(not isinstance(path, str) for path in members):
        raise ValueError("invalid persisted master collection artifacts")
    policy = {**(existing or {}), "schema": SCHEMA, "enabled": True,
            "operation": "refine" if enabled else "publish",
            "seed_depth": (existing or {}).get("seed_depth", 0) if seed_depth is None else seed_depth,
            "saved_rule_assistance": ((existing or {}).get("saved_rule_assistance", False)
                                      if saved_rule_assistance is None else saved_rule_assistance),
            "circuit_symmetry_assistance": ((existing or {}).get("circuit_symmetry_assistance", False)
                                            if circuit_symmetry_assistance is None else circuit_symmetry_assistance),
            "finite_feedback": ((existing or {}).get("finite_feedback", True)
                                if finite_feedback is None else finite_feedback),
            "containing_sector_depth": ((existing or {}).get("containing_sector_depth", 0)
                                        if containing_sector_depth is None else containing_sector_depth),
            "terminal_policy": "bounded exact search; finite nonminimal basis permitted",
            "master_minimality_claim": False}
    policy["collection_strategy"] = COLLECTION_STRATEGY
    policy["finite_feedback_recipe"] = FINITE_FEEDBACK_RECIPE
    policy["collection_artifacts"] = members
    if normalization_profile is not None:
        policy["normalization_profile"] = normalization_profile
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


def scope_binding(campaign, checkpoint, seed_depth=0, operation="publish"):
    """Cheap launch identity, not an alternative to Rust's cold verification."""
    campaign, checkpoint = Path(campaign), Path(checkpoint)
    latest = checkpoint / "latest.json"
    envelope = read_json(latest)
    manifest = envelope.get("manifest", envelope)
    if not isinstance(manifest, dict) or manifest.get("resumable") is not True:
        raise ValueError("master reduction requires a resumable native checkpoint")
    components = {"checkpoint_manifest_sha256": digest(latest), "amendments": _chain(campaign),
                  "selection_sha256": digest(campaign / "inputs/selection.json"),
                  "queries_sha256": digest(campaign / "inputs/queries.json"),
                  "operation": operation, "seed_depth": seed_depth if operation == "refine" else 0}
    key = hashlib.sha256(json.dumps(components, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {"key": key, "components": components, "generation": manifest.get("generation"),
            "manifest_blake3": bytes(envelope["blake3"]).hex() if isinstance(envelope.get("blake3"), list) else None}


def same_input_scope(left, right):
    """Operation/depth change the search, not the admitted physics scope."""
    fields = ("checkpoint_manifest_sha256", "amendments", "selection_sha256", "queries_sha256")
    return all(left.get("components", {}).get(key) == right.get("components", {}).get(key) for key in fields)


def completed_artifact(campaign):
    path = Path(campaign) / "artifacts/latest.json"
    if not path.is_file():
        return None
    record = read_json(path)
    relative = record.get("directory")
    if record.get("schema") != "rustred.saved-artifact-pointer.v1" or not isinstance(relative, str):
        raise ValueError("invalid saved artifact pointer")
    directory = (Path(campaign) / relative).resolve()
    if Path(relative).is_absolute() or not directory.is_relative_to(Path(campaign).resolve()):
        raise ValueError("artifact pointer must remain relative inside the campaign")
    if not (directory / "artifact.json").is_file():
        raise ValueError("saved artifact pointer has no completed native artifact")
    native = read_json(directory / "artifact.json")
    profile = normalization_metadata(native)
    for summary in (record, record.get("refinement", {})):
        if summary.get("normalization_profile", profile["normalization_profile"]) != profile["normalization_profile"]:
            raise ValueError("saved artifact pointer normalization profile differs from native artifact")
        if "normalization_limits" in summary and summary["normalization_limits"] != profile.get("normalization_limits"):
            raise ValueError("saved artifact pointer normalization limits differ from native artifact")
    return {**record, **profile, "resolved_directory": directory}


def normalization_metadata(report):
    """Read launch policy, not mathematical proof: native authenticates limits."""
    profile = report.get("normalization_profile", "conservative-v1")
    if profile not in NORMALIZATION_PROFILES.values():
        raise ValueError("unknown native normalization profile")
    if "normalization_limits" in report and "normalization_profile" not in report:
        raise ValueError("native normalization limits have no versioned profile")
    return {"normalization_profile": profile,
            **({"normalization_limits": report["normalization_limits"]} if "normalization_limits" in report else {})}


def publish_pointer(campaign, directory, binding, operation, driver, refinement=None):
    # Called under the dispatcher lock, only after native success. An interrupted
    # refinement leaves the last valid publication visible and untouched.
    (Path(campaign) / "artifacts").mkdir(exist_ok=True)
    driver.write_json(Path(campaign) / "artifacts/latest.json", {
        "schema": "rustred.saved-artifact-pointer.v1",
        "directory": str(Path(directory).resolve().relative_to(Path(campaign).resolve())),
        "scope_binding": binding, "operation": operation,
        "status": "published_unrefined" if operation == "publish" else "completed_nonminimal",
        "completed_unix_time": time.time(), "master_minimality_claim": False,
        **normalization_metadata(read_json(Path(directory) / "artifact.json")),
        **({"refinement": refinement} if refinement is not None else {})})


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
    """Native publication or explicit refinement, never Python algebra."""
    operation = policy.get("operation", "publish")
    publishing = operation == "publish"
    phase = "Artifact publication" if publishing else "Master refinement"
    supervisor = driver.SUPERVISOR
    telemetry, dashboard = supervisor.MONITOR.TELEMETRY, supervisor.MONITOR.DASHBOARD
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    attempt = directory / "runs" / timestamp
    attempt.mkdir(parents=True)
    events, stop_file = attempt / "events.jsonl", attempt / "stop-request.json"
    executable = driver.steering_executable(plan["steering_policy"])[1]
    if policy.get("executable"):
        executable = Path(plan["campaign_directory"]) / "master-reduction" / policy["executable"]["path"]
    command = [str(executable), "walk-publish" if publishing else "walk-master-reduce"]
    if publishing:
        command += ["--command", str(request), "--checkpoint", plan["checkpoint_directory"]]
    else:
        command += ["--artifact", str(policy["source_artifact"])]
    command += ["--directory", str(directory),
               "--events", str(events), "--stop-file", str(stop_file),
               "--checkpoint-interval-seconds", str(plan["checkpoint_interval_seconds"]),
               "--threads", str(plan["requested_workers"])]
    if not publishing:
        command += ["--seed-depth", str(policy["seed_depth"])]
        command.append("--finite-feedback" if policy.get("finite_feedback", True) else "--no-finite-feedback")
        for member in policy.get("collection_artifacts", []):
            command += ["--collection-artifact", member]
        if policy.get("containing_sector_depth", 0):
            command += ["--containing-sector-depth", str(policy["containing_sector_depth"])]
        if policy.get("saved_rule_assistance", False):
            command.append("--saved-rule-assistance")
        if policy.get("circuit_symmetry_assistance", False):
            command.append("--circuit-symmetry-assistance")
        profile = policy.get("effective_normalization_profile", "conservative-v1")
        if profile != "conservative-v1" or policy.get("normalization_profile") is not None:
            command += ["--normalization-profile", profile.removesuffix("-v1")]
    if (directory / "latest.json").is_file():
        if not publishing:
            snapshot = read_json(directory / "latest.json")
            pinned = normalization_metadata(snapshot)["normalization_profile"]
            if pinned != profile:
                raise ValueError("cannot change normalization profile on resume; start a new phase directory")
            if (snapshot.get("finite_feedback", False) != policy.get("finite_feedback", True)
                    or (snapshot.get("finite_feedback", False)
                        and snapshot.get("finite_feedback_recipe") != FINITE_FEEDBACK_RECIPE)):
                raise ValueError("cannot change finite feedback on resume; start a new phase directory")
        command.append("--resume")
    elif publishing:
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
                      "scope_binding": binding, "phase": phase, "operation": operation})
    driver.write_json(directory.parent.parent / "active-phase.json", {
        "schema": SCHEMA, "phase": phase, "operation": operation, "scope_binding": binding["key"],
        **({"normalization_profile": profile} if not publishing else {}),
        "directory": str(directory), "run_directory": str(attempt), "request": str(request)})
    presenter = dashboard.Presenter()
    stream = telemetry.TelemetryStream(attempt / "telemetry.jsonl")
    tail = supervisor.MONITOR.EventTail(events)
    latest = {"phase": phase, "operation": operation,
              "stage": "cold closure verification" if publishing else "loading published artifact", "status": "running",
              "seed_depth": policy.get("effective_seed_depth", policy["seed_depth"]), "scope_binding": binding["key"],
              "containing_sector_depth": (policy.get("effective_containing_sector_depth",
                                                    policy.get("containing_sector_depth", 0)) if not publishing else 0),
              "saved_rule_assistance": not publishing and policy.get("saved_rule_assistance", False),
              "circuit_symmetry_assistance": not publishing and policy.get("circuit_symmetry_assistance", False)}
    latest["finite_feedback"] = not publishing and policy.get("finite_feedback", True)
    if not publishing:
        latest["normalization_profile"] = profile
    checkpoint = {}

    def observe(event):
        nonlocal checkpoint
        payload = event.get("progress", event)
        if not isinstance(payload, dict):
            return
        if payload.get("event", "").startswith(("master_reduction", "artifact_")) or payload.get("phase") in ("Master reduction", phase):
            latest.update({key: value for key, value in payload.items() if value is not None})
            latest.update(phase=phase, operation=operation)
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
                  "resources": last_resources, "progress": {"phase": phase},
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
        finished_state = "published_unrefined" if publishing else "completed_nonminimal"
        publish(finished_state if status == 0 else "paused" if paused else "failed", force=True)
        driver.write_json(attempt / "supervisor-result.json", {"exit_status": status, "stop_reason": stop_reason,
                          "phase": phase, "operation": operation, "scope_binding": binding["key"],
                          "peak_observed_rss_bytes": peak, "elapsed_seconds": time.monotonic() - started})
        if status == 0:
            if not (directory / "artifact.json").is_file():
                raise ValueError("native success did not produce its completed artifact.json")
            native_artifact = read_json(directory / "artifact.json")
            native_profile = normalization_metadata(native_artifact)
            if not publishing and native_profile["normalization_profile"] != profile:
                raise ValueError("native artifact normalization profile differs from requested refinement")
            if not publishing and (native_artifact.get("finite_feedback") != policy.get("finite_feedback", True)
                                   or native_artifact.get("finite_feedback_recipe") != FINITE_FEEDBACK_RECIPE):
                raise ValueError("native artifact finite feedback recipe differs from requested refinement")
            current = scope_binding(plan["campaign_directory"], plan["checkpoint_directory"])
            if not same_input_scope(current, binding):
                raise ValueError("campaign scope changed during postprocessing; artifact retained but latest pointer not updated")
            previous = completed_artifact(plan["campaign_directory"])
            keep_refined = publishing and previous and same_input_scope(previous.get("scope_binding", {}), binding) \
                and previous.get("operation") == "refine"
            if not keep_refined:
                refinement = None if publishing else {
                        **native_profile,
                        "collection_strategy": COLLECTION_STRATEGY,
                        "finite_feedback": policy.get("finite_feedback", True),
                        "finite_feedback_recipe": FINITE_FEEDBACK_RECIPE,
                        "collection_inputs": policy.get("collection_inputs", []),
                        "seed_depth": latest["seed_depth"],
                        "containing_sector_depth": latest["containing_sector_depth"],
                        "circuit_symmetry_assistance": policy.get("circuit_symmetry_assistance", False),
                        "saved_rule_assistance": policy.get("saved_rule_assistance", False), "source_artifact": str(
                            Path(policy["source_artifact"]).relative_to(Path(plan["campaign_directory"])))}
                publish_pointer(plan["campaign_directory"], directory, binding, operation, driver, refinement)
                if not publishing:
                    # Peers are now native members of the portable publication.
                    # Future plain refinement inherits those copies and never
                    # requires the original external source directories again.
                    policy_path = Path(plan["campaign_directory"]) / "master-reduction/policy.json"
                    if policy_path.is_file():
                        stored_policy = read_json(policy_path)
                        stored_policy["collection_artifacts"] = []
                        driver.write_json(policy_path, stored_policy)
            driver.write_json(directory.parent.parent / "completed-phase.json", {
                "schema": SCHEMA, "directory": str(directory), "scope_binding": binding,
                "completed_unix_time": time.time(), "master_minimality_claim": False})
            print("Scoped artifact published; master refinement was not requested." if publishing else
                  "Explicit master refinement completed (bounded search, not a minimality proof).", flush=True)
            if latest.get("artifact"):
                print("Symbolic artifact: " + str(latest["artifact"]), flush=True)
        else:
            print(f"{phase} {'paused' if paused else 'stopped'}; checkpoint: {directory}", flush=True)
            refinement_flags = ("--refine-masters " + ("--master-saved-rule-assistance "
                                if policy.get("saved_rule_assistance", False)
                                else "--no-master-saved-rule-assistance ")
                                + ("--master-circuit-symmetry-assistance "
                                   if policy.get("circuit_symmetry_assistance", False)
                                   else "--no-master-circuit-symmetry-assistance ")
                                + ("--master-finite-feedback " if policy.get("finite_feedback", True)
                                   else "--no-master-finite-feedback ")
                                + "--master-normalization-profile " + profile.removesuffix("-v1") + " ") if not publishing else ""
            print(f"Resume with: {sys.executable} {Path(driver.__file__).resolve()} --campaign-directory "
                  f"{plan['campaign_directory']} --resume {refinement_flags}--start", flush=True)
            if not paused:
                print(f"Native error details: {attempt / 'stderr.log'}", file=sys.stderr)
        return status if status >= 0 else 128 - status
    finally:
        for sig, handler in handlers.items():
            signal.signal(sig, handler)


def run(plan, policy, resume, driver, postprocess_only=False):
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
            driver.write_json(directory / "policy.json", {key: value for key, value in policy.items()
                              if key not in ("operation", "source_artifact")})
            binding = scope_binding(campaign, checkpoint) if (checkpoint / "latest.json").is_file() else None
            request = completed_request(campaign, checkpoint, binding, driver) if (resume or postprocess_only) and binding else None
        operation = policy.get("operation", "publish")
        if operation == "refine":
            previous = completed_artifact(campaign)
            if not previous or not binding or not same_input_scope(previous.get("scope_binding", {}), binding):
                raise ValueError("refine requires a completed artifact for the current scope; run or publish it first")
            refinement = previous.get("refinement", {})
            effective_profile = (NORMALIZATION_PROFILES[policy["normalization_profile"]]
                                 if policy.get("normalization_profile") is not None
                                 else previous["normalization_profile"])
            # Cheap launch identity only. Rust authenticates every native member
            # and snapshots it into the portable output package.
            collection_inputs = []
            missing_collection_input = False
            for path in policy.get("collection_artifacts", []):
                member = Path(path)
                manifest = member / "artifact.json" if member.is_dir() else member
                if not manifest.exists():
                    missing_collection_input = True
                    break
                collection_inputs.append({"manifest_sha256": digest(manifest)})
            if missing_collection_input:
                active = read_json(directory / "active-phase.json")
                phase_directory = Path(active["directory"])
                native = read_json(phase_directory / "latest.json")
                if (active.get("operation") != "refine"
                        or native.get("collection_requested_paths") != policy.get("collection_artifacts", [])):
                    raise ValueError("missing collection input has not been snapshotted into the paused phase")
                collection_inputs = read_json(phase_directory / "steering-binding.json")["collection_inputs"]
            collection_inputs.sort(key=lambda value: value["manifest_sha256"])
            policy = {**policy, "collection_inputs": collection_inputs}
            if (previous.get("operation") == "refine"
                    and refinement.get("collection_strategy") == COLLECTION_STRATEGY
                    and refinement.get("finite_feedback", False) == policy.get("finite_feedback", True)
                    and (not policy.get("finite_feedback", True)
                         or refinement.get("finite_feedback_recipe") == FINITE_FEEDBACK_RECIPE)
                    and (not collection_inputs or refinement.get("collection_inputs", []) == collection_inputs)
                    and refinement.get("seed_depth", -1) >= policy["seed_depth"]
                    and refinement.get("containing_sector_depth", 0) >= policy.get("containing_sector_depth", 0)
                    and refinement.get("circuit_symmetry_assistance", False) == policy.get("circuit_symmetry_assistance", False)
                    and previous["normalization_profile"] == effective_profile
                    and refinement.get("saved_rule_assistance", False) == policy.get("saved_rule_assistance", False)):
                print("Requested refinement already completed: " + str(previous["resolved_directory"]), flush=True)
                return 0
            policy = {**policy, "source_artifact": str(previous["resolved_directory"]),
                      "effective_normalization_profile": effective_profile,
                      "effective_seed_depth": max(policy["seed_depth"], refinement.get("seed_depth", 0)),
                      "effective_containing_sector_depth": max(policy.get("containing_sector_depth", 0),
                                                               refinement.get("containing_sector_depth", 0))}
        elif binding:
            previous = completed_artifact(campaign)
            if previous and same_input_scope(previous.get("scope_binding", {}), binding):
                print("Current scope already published: " + str(previous["resolved_directory"]), flush=True)
                active_path = directory / "active-phase.json"
                try:
                    if active_path.is_file():
                        active = read_json(active_path)
                        status_path = Path(active.get("run_directory", "")) / "status.json"
                        if active.get("operation") == "refine" and status_path.is_file() \
                                and read_json(status_path).get("state") == "paused":
                            print("Manual refinement is paused; repeat saved_campaign.py refine to resume it explicitly.", flush=True)
                except (OSError, ValueError, TypeError):
                    pass  # An optional monitor hint cannot invalidate the artifact.
                return 0
        if request is None:
            if postprocess_only or operation == "refine":
                raise ValueError("current scope is not completed; postprocess-only never starts a solve")
            driver.write_json(campaign / "active-run.json", plan)
            driver.write_json(directory / "active-phase.json", {"schema": SCHEMA, "phase": "IBP closure",
                              "run_directory": plan["run_directory"]})
            status, stopped = phase_one(plan["command"])
            if stopped:
                return 4  # Native phase one may have finished just before Ctrl+C.
            if status not in (0, 4):
                return status
            binding = scope_binding(campaign, checkpoint) if (checkpoint / "latest.json").is_file() else None
            request = completed_request(campaign, checkpoint, binding, driver) if binding else None
            if request is None:
                print("IBP campaign remains paused/incomplete; no final artifact was published.", flush=True)
                return status if status else 4
        identity = {"scope": binding["key"], "operation": operation,
                    "seed_depth": policy["seed_depth"] if operation == "refine" else 0,
                    "source": str(Path(policy["source_artifact"]).relative_to(campaign)) if policy.get("source_artifact") else None,
                    "executable": policy.get("executable", {}).get("sha256", plan.get("executable_sha256"))}
        if operation == "refine":
            identity["collection_strategy"] = COLLECTION_STRATEGY
            identity["collection_inputs"] = policy.get("collection_inputs", [])
            identity["finite_feedback"] = policy.get("finite_feedback", True)
            identity["finite_feedback_recipe"] = FINITE_FEEDBACK_RECIPE
        if operation == "refine" and policy.get("saved_rule_assistance", False):
            # Preserve legacy ordinary checkpoint paths. Assisted work has a
            # distinct identity and can never resume an ordinary row cursor.
            identity["saved_rule_assistance"] = True
        if operation == "refine" and policy.get("circuit_symmetry_assistance", False):
            identity["circuit_symmetry_assistance"] = True
        if operation == "refine" and policy["effective_normalization_profile"] != "conservative-v1":
            # Legacy conservative phases keep their original checkpoint path.
            identity["normalization_profile"] = policy["effective_normalization_profile"]
        if operation == "refine" and policy.get("containing_sector_depth", 0):
            # Zero retains ordinary legacy paths; different source strategies
            # must not resume one another's native cursors.
            identity["containing_sector_depth"] = policy["containing_sector_depth"]
        phase_key = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
        target = directory / "scopes" / phase_key
        target.mkdir(parents=True, exist_ok=True)
        driver.write_json(target / "steering-binding.json", {"scope_binding": binding, **identity})
        return phase_two(plan, policy, request, binding, target, driver)
