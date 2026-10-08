#!/usr/bin/env python3
"""Prepare or manually launch the persistent saved-owner production campaign.

By default this verifies staged input and freezes the supplied executable, then
prints a command. Only --start launches the solver. No algebra lives in Python.
--prepare-from copies verified immutable inputs into a new, disjoint campaign;
its default helper-first query order is only an admission-order heuristic.
With --queries it stages a verified replacement query document instead, and
--attach copies planner receipts read-only beside the inputs. The frozen
steering (schema v3) fixes workers, CPUs, checkpoint interval, RAM policy
(hard cap, margin, host MemAvailable floor, own swap-growth guard), publication
policy, transfer lookahead, inspection workers, G2 residual anchors and the frontier policy (new
campaigns default to stop: save and stop at the first frontier, exit 4);
only the RAM options may be overridden per resume.
--resume --upgrade-executable NEW moves a paused campaign onto a
performance-only binary: NEW's `walk-semantics-version` probe must equal the
saved CP5 checkpoint's walk semantics version, unless bin/executable.json
history already lists NEW for that version (a rollback, which needs no
probe). Without --start this is a read-only dry run. With --start it refuses
a live run, freezes NEW beside the kept old binary, rewrites only the
--executable value of the frozen steering, records the history in
bin/executable.json and then resumes normally. The probe is necessary, not
sufficient: the native resume also compares the saved request binding.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import selectors
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time

_PHASE_SPEC = importlib.util.spec_from_file_location("campaign_phases", Path(__file__).with_name("campaign_phases.py"))
PHASES = importlib.util.module_from_spec(_PHASE_SPEC)
_PHASE_SPEC.loader.exec_module(PHASES)

# Supervisor RAM guard options: frozen at preparation, overridable per start/resume.
# None (steering frozen before the option existed) means the supervisor's
# default at launch; new steering freezes each default explicitly.
RAM_POLICY_OPTIONS = ("max_memory_bytes", "ram_guard_margin_percent", "host_memory_reserve_bytes",
                      "swap_growth_stop_bytes_per_second", "swap_growth_stop_seconds")
OPTIONAL_RAM_POLICY_OPTIONS = RAM_POLICY_OPTIONS[2:]
# Frontier rescue (schema v5, explicit query roles): new campaigns resume automatically after a
# frontier stop of a known class (supervisor --auto-rescue); amendments live
# in <campaign>/amendments and are re-supplied on every --resume. Steering
# v5 requires explicit immutable query roles; untyped older steering is rejected.
RESCUE_OPTIONS = ("auto_rescue", "helper_id_prefix", "max_rescues")
DEFAULT_HELPER_ID_PREFIX = "owner-anchor-"
DEFAULT_MAX_RESCUES = 32
AMENDMENTS_DIRECTORY = "amendments"
STEERING_SCHEMA = "rustred.production-steering.v5"
STEERING_SCHEMAS = ("rustred.production-steering.v1", "rustred.production-steering.v2",
                    "rustred.production-steering.v3", STEERING_SCHEMA)
FROZEN_OPTIONS = ("workers", "cpus", "checkpoint_interval_seconds", "max_memory_bytes",
                  "ram_guard_margin_percent", "apply_subdivision_axis", "apply_subdivision_cut",
                  "apply_cell_refinement_max_cardinality", "publication_policy",
                  "transfer_unreserved_lookahead", "inspection_workers", "frontier_policy", "g2_residual_anchors",
                  "epoch_inspector_lookup", "epoch_rolling", "epoch_dispatch",
                  "epoch_publication_order", "epoch_cut_size", "epoch_window",
                  "epoch_result_escrow_jobs", "epoch_result_escrow_bytes",
                  "epoch_preparation_workers", "epoch_preparation_max_obligations", "epoch_preparation_max_retirements",
                  *OPTIONAL_RAM_POLICY_OPTIONS, *RESCUE_OPTIONS)
DEFAULT_PUBLICATION_POLICY = "ready"
# A10: new campaigns save and stop at the first frontier; steering written
# before the option existed keeps its historical (record) native argv.
DEFAULT_FRONTIER_POLICY = "stop"
LEGACY_FRONTIER_POLICY = "record"
# Liveness of RAM-guard stops (the legacy walk has no per-ID attempt counter):
# a resume is refused after this many consecutive runs that the RAM guard
# stopped without any committed progress (a head whose inspection alone trips
# the guard is saved Reserved, dispatched first on resume and trips it again).
# Stops on the campaign's own signals always count; host-wide stops (the
# MemAvailable floor and emergency) count only when the supervisor attributed
# them to the campaign (ram_guard_stop.own_memory_signal), so other users'
# memory use on a shared host cannot trip the refusal. host_swap_in_sustained
# is the retired host-wide swap-in reason (never attributed).
OWN_RAM_GUARD_STOPS = ("aggregate_rss_soft_limit", "aggregate_rss_hard_limit", "own_swap_growth_sustained")
HOST_RAM_GUARD_STOPS = ("host_memory_reserve", "host_memory_emergency", "host_swap_in_sustained")
RAM_GUARD_STOPS = OWN_RAM_GUARD_STOPS + HOST_RAM_GUARD_STOPS
DEFAULT_MAX_ZERO_PROGRESS_RAM_STOPS = 2
PROGRESS_KEYS = ("committed_domains", "completed_native_inspections", "committed_events")
QUERY_SCHEMA = "rustred.owner-domain-queries.json.v2"
QUERY_ROW_FIELDS = frozenset({"id", "owner", "lower", "upper", "max_numerator_rank", "power_bounds"})
ENTRY_PLAN_RECEIPT_NAME = "entry-plan-receipt.json"
CHECKPOINT_FORMAT = "RUSTRED-WALK-CP5"
CHECKPOINT_SCHEMA = 5
CHECKPOINT_KINDS = ("state", "bootstrap")
MAX_MANIFEST_BYTES = 16 * 1024 * 1024  # native OWNER_DOMAIN_WALK_CHECKPOINT_MANIFEST_MAX_BYTES
MAX_RECEIPT_BYTES = 1024 * 1024
PROBE_COMMAND = "walk-semantics-version"
PROBE_TIMEOUT_SECONDS = 60.0
MAX_PROBE_BYTES = 64 * 1024
UPGRADE_REASON = "upgrade_executable"
ROLLBACK_REASON = "rollback_executable"
_SUPERVISOR_SPEC = importlib.util.spec_from_file_location(
    "shared_owner_campaign", Path(__file__).with_name("shared_owner_campaign.py"))
SUPERVISOR = importlib.util.module_from_spec(_SUPERVISOR_SPEC)
_SUPERVISOR_SPEC.loader.exec_module(SUPERVISOR)
MAX_WORKERS = SUPERVISOR.MAX_WORKERS


def application_cardinality(text):
    if not text.isascii() or not text.isdecimal() or not 1 <= int(text) <= 2 * sys.maxsize + 1:
        raise argparse.ArgumentTypeError("application cardinality must be a positive native unsigned pointer-sized integer")
    return int(text)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, value):
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, prefix="." + path.name,
                                         delete=False) as stream:
            temporary = Path(stream.name)
            json.dump(value, stream, indent=2)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        sync_directory(path.parent)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def sync_directory(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def copy_executable(directory, source, expected=None, check=None):
    """Copy source to directory/rustred-<sha256>, read-only and synced.

    The bytes are written under a temporary name, synced, digest-checked and
    passed to check (if given) before they are linked under the final name,
    so an interrupted or refused copy never occupies it. An existing file of
    that name is reused only when its digest matches (and check accepts it).
    expected is the digest the caller already validated, if any.
    """
    if not source.is_file() or not os.access(source, os.X_OK):
        raise ValueError("supplied executable must be an executable file")
    source_hash = digest(source)
    if expected is not None and source_hash != expected:
        raise ValueError(f"executable changed since validation (now sha256 {source_hash}, validated {expected})")
    target = directory / ("rustred-" + source_hash)
    if target.is_symlink() or (target.exists() and not target.is_file()):
        raise ValueError(f"frozen executable path is not a regular file: {target}")
    if target.exists():
        if digest(target) != source_hash:
            raise ValueError(f"existing {target} does not match the SHA-256 in its name (the leftover of an "
                             f"interrupted copy, or a damaged frozen binary); if {directory / 'executable.json'} "
                             "does not name it, remove it and rerun")
        if check is not None:
            check(target)
    else:
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(dir=directory, prefix="." + target.name + ".",
                                             delete=False) as outgoing:
                temporary = Path(outgoing.name)
                with source.open("rb") as incoming:
                    shutil.copyfileobj(incoming, outgoing, 1024 * 1024)
                outgoing.flush()
                os.fsync(outgoing.fileno())
            if digest(temporary) != source_hash or digest(source) != source_hash:
                raise ValueError("executable changed during freezing")
            temporary.chmod(0o555)
            if check is not None:
                check(temporary)
            os.link(temporary, target)  # Never replaces an existing name.
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
        sync_directory(directory)
    target.chmod(0o555)
    with target.open("rb") as stream:
        os.fsync(stream.fileno())
    return target, source_hash


def freeze_executable(campaign, source):
    directory = campaign / "bin"
    directory.mkdir(parents=True, exist_ok=True)
    sync_directory(campaign)
    receipt_path = directory / "executable.json"
    if receipt_path.exists():
        receipt = json.loads(receipt_path.read_text())
        target = directory / receipt["file"]
        if digest(target) != receipt["sha256"]:
            raise ValueError("frozen executable digest changed")
        if source is not None and digest(source) != receipt["sha256"]:
            raise ValueError("campaign already has a different frozen executable; use a new campaign directory, "
                             "or --resume --upgrade-executable for a semantics-compatible binary")
        return target.resolve(), receipt["sha256"]
    if source is None:
        raise ValueError("first preparation requires --executable; resume retains the frozen binary")
    target, source_hash = copy_executable(directory, source)
    write_json(receipt_path, {"sha256": source_hash, "file": target.name, "source": str(source.resolve())})
    return target.resolve(), source_hash


def read_bounded_json(path, limit, what):
    with Path(path).open("rb") as stream:
        raw = stream.read(limit + 1)
    if len(raw) > limit:
        raise ValueError(f"{what} exceeds {limit} bytes")
    value = json.loads(raw)
    if not isinstance(value, dict):
        raise ValueError(f"{what} must be a JSON object")
    return value


def natural(value):
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def checkpoint_identity(checkpoint):
    """Resume binding of the latest native manifest (bounded, read-only)."""
    path = checkpoint / "latest.json"
    if not path.is_file():
        raise ValueError(f"executable upgrade requires a saved native checkpoint: {path} is missing")
    manifest = read_bounded_json(path, MAX_MANIFEST_BYTES, "checkpoint manifest")
    if manifest.get("format") != CHECKPOINT_FORMAT or manifest.get("schema") != CHECKPOINT_SCHEMA \
            or not natural(manifest.get("schema")):
        raise ValueError(f"checkpoint manifest is not {CHECKPOINT_FORMAT} schema {CHECKPOINT_SCHEMA}; "
                         "only CP5 campaigns can change executable")
    if manifest.get("kind") not in CHECKPOINT_KINDS:
        raise ValueError("checkpoint manifest kind must be state or bootstrap")
    if not natural(manifest.get("walk_semantics_version")):
        raise ValueError("checkpoint manifest has no walk_semantics_version")
    return {"manifest": str(path), "format": manifest["format"], "schema": manifest["schema"],
            "kind": manifest["kind"], "generation": manifest.get("generation"),
            "walk_semantics_version": manifest["walk_semantics_version"],
            "executable_blake3": manifest.get("executable"),
            "executable_first_blake3": manifest.get("executable_first")}


def probe_walk_semantics(executable):
    """Run `EXECUTABLE walk-semantics-version` with a timeout and bounded output."""
    environment = dict(os.environ, SYMBOLICA_HIDE_BANNER="1")
    try:
        process = subprocess.Popen([str(executable), PROBE_COMMAND], stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=environment,
                                   start_new_session=True)
    except OSError as error:
        raise ValueError(f"cannot run {executable} {PROBE_COMMAND}: {error}") from error
    captured = {process.stdout: bytearray(), process.stderr: bytearray()}
    deadline = time.monotonic() + PROBE_TIMEOUT_SECONDS
    try:
        with selectors.DefaultSelector() as selector:
            for stream in captured:
                selector.register(stream, selectors.EVENT_READ)
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise ValueError(f"{PROBE_COMMAND} probe did not finish within {PROBE_TIMEOUT_SECONDS:g} s")
                for key, _ in selector.select(remaining):
                    chunk = os.read(key.fd, 65536)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    captured[key.fileobj] += chunk
                    if len(captured[key.fileobj]) > MAX_PROBE_BYTES:
                        raise ValueError(f"{PROBE_COMMAND} probe output exceeds {MAX_PROBE_BYTES} bytes")
        try:
            status = process.wait(max(0.0, deadline - time.monotonic()))
        except subprocess.TimeoutExpired as error:
            raise ValueError(f"{PROBE_COMMAND} probe did not exit within {PROBE_TIMEOUT_SECONDS:g} s") from error
    finally:
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
        process.stdout.close()
        process.stderr.close()
    if status != 0:
        detail = bytes(captured[process.stderr]).decode("utf-8", "replace").strip().splitlines()
        raise ValueError(f"{executable} has no usable {PROBE_COMMAND} probe (exit status {status}"
                         + (f": {detail[0]}" if detail else "") + "); a binary without the probe "
                         "cannot be shown to share the checkpoint's walk semantics")
    lines = bytes(captured[process.stdout]).decode("utf-8").splitlines()
    try:
        probe = json.loads(lines[0]) if len(lines) == 1 else None
    except ValueError:
        probe = None
    if (not isinstance(probe, dict) or not natural(probe.get("walk_semantics_version"))
            or not isinstance(probe.get("checkpoint_format"), str) or not natural(probe.get("checkpoint_schema"))):
        raise ValueError(f"{PROBE_COMMAND} probe must print one JSON object with walk_semantics_version, "
                         "checkpoint_format and checkpoint_schema")
    return {name: probe[name] for name in ("walk_semantics_version", "checkpoint_format", "checkpoint_schema")}


def process_alive(identity):
    """PID plus kernel start ticks, the identity campaign_monitor verifies."""
    try:
        pid, expected = int(identity["pid"]), int(identity["start_ticks"])
        stat = Path(f"/proc/{pid}/stat").read_text()
        return int(stat[stat.rfind(")") + 2:].split()[19]) == expected
    except (OSError, ValueError, KeyError, TypeError, IndexError):
        return False


def campaign_runs(campaign):
    """Run directories that may hold a live supervisor of this campaign.

    The run named by active-run.json, every directory under campaign/runs and
    the `<run>.resume-<id>` siblings of the active run: the supervisor's
    printed resume command creates those without updating active-run.json.
    """
    runs = set()
    path = campaign / "active-run.json"
    if path.exists():
        run = read_bounded_json(path, MAX_RECEIPT_BYTES, "active-run.json").get("run_directory")
        if not isinstance(run, str) or not run:
            raise ValueError("active-run.json does not name a run directory")
        run = Path(run)
        runs.add(run)
        if run.parent.is_dir():
            runs.update(sibling for sibling in run.parent.iterdir()
                        if sibling.name.startswith(run.name + ".resume-"))
    if (campaign / "runs").is_dir():
        runs.update((campaign / "runs").iterdir())
    phase = campaign / "master-reduction/active-phase.json"
    if phase.is_file():
        run = read_bounded_json(phase, MAX_RECEIPT_BYTES, "active-phase.json").get("run_directory")
        if not isinstance(run, str) or not run:
            raise ValueError("active-phase.json does not name a run directory")
        runs.add(Path(run))
    return sorted(run for run in runs if run.is_dir())


def campaign_run_liveness(campaign):
    """Evidence that any run of this campaign may still be alive (empty: none).

    For every run of campaign_runs it reads the processes.json/status.json
    identities; before the first identity is published, a live run.pid or
    request.json supervisor PID counts as alive. The native checkpoint.lock
    remains the final guard.
    """
    try:
        boot = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except OSError:
        boot = None
    evidence = []
    for run in campaign_runs(campaign):
        try:
            evidence += run_liveness(run, boot)
        except ValueError as error:
            raise ValueError(f"cannot read the process identity of run {run}: {error}") from error
    return evidence


def run_liveness(run, boot):
    """Liveness evidence for one supervisor run directory."""
    evidence = []
    identified = False
    for name in ("processes.json", "status.json"):
        if not (run / name).is_file():
            continue
        document = read_bounded_json(run / name, MAX_RECEIPT_BYTES, name)
        identity = document if name == "processes.json" else document.get("process_identity")
        if not isinstance(identity, dict):
            continue
        identified = True
        if boot is not None and identity.get("boot_id") not in (None, boot):
            continue
        for role in ("supervisor", "native"):
            process = identity.get(role)
            if isinstance(process, dict) and process_alive(process):
                evidence.append(f"{role} pid {process['pid']} from {run / name} is alive")
    finished = (run / "run.status").exists() or (run / "supervisor-result.json").exists()
    if not finished and not (run / "processes.json").is_file():
        pids = []
        if (run / "run.pid").is_file():
            pids.append(("run.pid", (run / "run.pid").read_text().strip()))
        if not identified and (run / "request.json").is_file():
            pids.append(("request.json", read_bounded_json(run / "request.json", MAX_RECEIPT_BYTES,
                                                           "request.json").get("supervisor_pid")))
        for name, pid in pids:
            if str(pid).isdecimal() and Path(f"/proc/{int(pid)}").exists():
                evidence.append(f"pid {pid} from {run / name} exists and its identity is not yet published")
    return evidence


def run_started(run):
    """Start time of a run directory (its request.json, else the directory)."""
    for path in (run / "request.json", run):
        try:
            return path.stat().st_mtime
        except OSError:
            continue
    return 0.0


def checkpoint_progress(checkpoint):
    """The progress counters of a supervisor receipt's checkpoint metadata (None when absent)."""
    if not isinstance(checkpoint, dict) or checkpoint.get("state") != "saved":
        return None
    values = tuple(checkpoint.get(key) for key in PROGRESS_KEYS)
    return values if all(natural(value) for value in values) else None


def ram_guard_counts(result):
    """Whether a supervisor receipt's stop is a RAM-guard stop attributable to the campaign."""
    stop = result.get("operator_or_resource_stop")
    if stop in OWN_RAM_GUARD_STOPS:
        return True
    record = result.get("ram_guard_stop")
    return stop in HOST_RAM_GUARD_STOPS and isinstance(record, dict) and record.get("own_memory_signal") is True


def ram_guard_liveness(campaign):
    """Trailing consecutive runs stopped by the RAM guard without committed progress.

    Runs are ordered by start time; a run counts when its supervisor receipt
    names a RAM-guard stop reason attributable to the campaign (own RSS or
    swap; a host-wide stop only with the supervisor's own_memory_signal) and
    its saved checkpoint's committed domains, completed native inspections and
    committed events all equal those of the run before it (the state it
    resumed). Any other run, including an unattributed host-wide stop, ends
    the streak.
    """
    rows = []
    for run in sorted(campaign_runs(campaign), key=run_started):
        path = run / "supervisor-result.json"
        if not path.is_file():
            continue
        try:
            result = read_bounded_json(path, MAX_RECEIPT_BYTES, "supervisor-result.json")
        except ValueError:
            continue
        rows.append({"run": str(run), "stop": result.get("operator_or_resource_stop"),
                     "counts": ram_guard_counts(result),
                     "progress": checkpoint_progress(result.get("checkpoint"))})
    streak = []
    for previous, row in zip([None, *rows], rows):
        stalled = (row["counts"] and previous is not None
                   and row["progress"] is not None and row["progress"] == previous["progress"])
        streak = [*streak, row] if stalled else []
    return {"consecutive_zero_progress_ram_stops": len(streak),
            "runs": [row["run"] for row in streak],
            "stop_reasons": [row["stop"] for row in streak],
            "progress": None if not streak else dict(zip(PROGRESS_KEYS, streak[-1]["progress"]))}


@contextmanager
def existing_phase_lock(campaign):
    """Prevent query amendments while the optional phase dispatcher is active."""
    path = campaign / "master-reduction/dispatcher.lock"
    try:
        descriptor = os.open(path, os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC)
    except FileNotFoundError:
        descriptor = None
    if descriptor is None:
        yield
        return
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("master reduction dispatcher is still running; pause it before extending the scope") from None
        yield
    finally:
        os.close(descriptor)


@contextmanager
def checkpoint_lock(checkpoint):
    """Hold the native checkpoint.lock (never created here) while bin/ changes."""
    try:
        descriptor = os.open(checkpoint / "checkpoint.lock", os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC)
    except FileNotFoundError:
        descriptor = None
    if descriptor is None:
        yield
        return
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("checkpoint is in use by a live native process; pause it and wait for exit 4") from None
        yield
    finally:
        os.close(descriptor)


def steering_executable(policy):
    command = policy.get("command_arguments")
    if not isinstance(command, list) or command.count("--executable") != 1:
        raise ValueError("frozen steering must contain exactly one --executable")
    return command.index("--executable") + 1, Path(command[command.index("--executable") + 1])


def receipt_history(receipt):
    history = receipt.get("history", [])
    if not isinstance(history, list) or not all(isinstance(row, dict) for row in history):
        raise ValueError("bin/executable.json history must be a list of objects")
    return history


def history_vouches(receipt, sha256, walk_semantics_version):
    """Whether this campaign's history records sha256 under the checkpoint's semantics version."""
    return any(row.get("sha256") == sha256 and natural(row.get("walk_semantics_version"))
               and row["walk_semantics_version"] == walk_semantics_version for row in receipt_history(receipt))


def plan_executable_upgrade(campaign, checkpoint, source, frozen, frozen_hash):
    """Read-only half of --upgrade-executable: everything the dry run prints.

    NEW's probe must report the checkpoint's format, schema and walk
    semantics version, unless bin/executable.json history lists NEW's digest
    with that version: the campaign established it earlier (the original
    binary wrote the checkpoint, later ones passed the probe), so returning
    to it (a rollback, e.g. to the original pre-probe binary) needs no probe.
    The native resume stays the final check; it also compares the request
    binding, which no probe covers.
    """
    if not source.is_file() or not os.access(source, os.X_OK):
        raise ValueError("--upgrade-executable must name an executable file")
    source = source.resolve()
    new_hash = digest(source)
    if new_hash == frozen_hash:
        raise ValueError("--upgrade-executable names the already frozen executable; plain --resume suffices")
    saved = checkpoint_identity(checkpoint)
    receipt = read_bounded_json(campaign / "bin" / "executable.json", MAX_RECEIPT_BYTES, "executable receipt")
    if history_vouches(receipt, new_hash, saved["walk_semantics_version"]):
        reason, probe, evidence = ROLLBACK_REASON, None, "executable_history"
    else:
        reason, probe, evidence = UPGRADE_REASON, probe_walk_semantics(source), "probe"
        if (probe["checkpoint_format"], probe["checkpoint_schema"]) != (saved["format"], saved["schema"]):
            raise ValueError(f"new executable resumes {probe['checkpoint_format']} schema "
                             f"{probe['checkpoint_schema']}, but the checkpoint is {saved['format']} "
                             f"schema {saved['schema']}")
        if probe["walk_semantics_version"] != saved["walk_semantics_version"]:
            raise ValueError(f"walk semantics version differs (checkpoint {saved['walk_semantics_version']}, "
                             f"new executable {probe['walk_semantics_version']}); only a semantics-compatible "
                             "binary may resume this campaign; start a new campaign instead")
    return {"reason": reason, "frozen": {"sha256": frozen_hash, "path": str(frozen)},
            "new": {"sha256": new_hash, "source": str(source),
                    "path": str((campaign / "bin" / ("rustred-" + new_hash)).resolve()), "probe": probe,
                    "semantics_evidence": evidence},
            "checkpoint": saved, "walk_semantics_version": saved["walk_semantics_version"],
            "live_run_evidence": campaign_run_liveness(campaign)}


def upgraded_steering(policy, upgrade, replaced_unix_time):
    """Steering with only the --executable value replaced, plus the upgrade note."""
    index, current = steering_executable(policy)
    current = current.resolve()
    old, new = Path(upgrade["frozen"]["path"]), Path(upgrade["new"]["path"])
    note = {"replaced_sha256": upgrade["frozen"]["sha256"], "sha256": upgrade["new"]["sha256"],
            "walk_semantics_version": upgrade["walk_semantics_version"], "reason": upgrade["reason"]}
    history = policy.get("executable_upgrades", [])
    if not isinstance(history, list) or not all(isinstance(row, dict) for row in history):
        raise ValueError("frozen steering executable_upgrades must be a list of objects")
    if current == new and history and {key: history[-1].get(key) for key in note} == note:
        return policy  # An interrupted earlier upgrade already rewrote the steering.
    if current != old:
        raise ValueError(f"frozen steering executable {current} is not the frozen receipt's {old}; refusing to rewrite")
    command = list(policy["command_arguments"])
    command[index] = str(new)
    upgraded = dict(policy, command_arguments=command)
    upgraded["executable_upgrades"] = [*history, {**note, "replaced_unix_time": replaced_unix_time}]
    return upgraded


def apply_executable_upgrade(campaign, checkpoint, upgrade, source):
    """Mutating half: freeze NEW, rewrite steering, then commit executable.json.

    Every precondition is checked under the lock before the copy, and the
    copy is probed before it takes its final name, so a refusal here leaves
    bin/ unchanged.
    """
    evidence = campaign_run_liveness(campaign)
    if evidence:
        raise ValueError("a run of this campaign is alive (" + "; ".join(evidence)
                         + "); pause it with Ctrl-C and wait for exit 4 first")
    directory = campaign / "bin"
    receipt_path, steering_path = directory / "executable.json", directory / "steering.json"

    def same_probe(path):
        if probe_walk_semantics(path) != upgrade["new"]["probe"]:
            raise ValueError("frozen copy of the new executable reports a different walk semantics probe")

    with checkpoint_lock(checkpoint):
        if checkpoint_identity(checkpoint)["walk_semantics_version"] != upgrade["walk_semantics_version"]:
            raise ValueError("checkpoint walk semantics version changed during the upgrade")
        previous = read_bounded_json(receipt_path, MAX_RECEIPT_BYTES, "executable receipt")
        if previous.get("sha256") != upgrade["frozen"]["sha256"] or (
                upgrade["new"]["probe"] is None
                and not history_vouches(previous, upgrade["new"]["sha256"], upgrade["walk_semantics_version"])):
            raise ValueError("frozen executable receipt changed during the upgrade")
        if str((directory / ("rustred-" + upgrade["new"]["sha256"])).resolve()) != upgrade["new"]["path"]:
            raise ValueError("campaign bin directory moved since validation")
        now = time.time()
        policy = read_bounded_json(steering_path, MAX_RECEIPT_BYTES, "frozen steering")
        if policy.get("schema") not in STEERING_SCHEMAS:
            raise ValueError("unknown frozen steering policy")
        upgraded = upgraded_steering(policy, upgrade, now)
        target, new_hash = copy_executable(directory, source, expected=upgrade["new"]["sha256"],
                                           check=None if upgrade["new"]["probe"] is None else same_probe)
        if upgraded is not policy:
            write_json(steering_path, upgraded)
        steering_path.chmod(0o444)
        # executable.json is the commit point; an interruption before it is
        # refused by a plain --resume and completed by rerunning the upgrade.
        replaced = {key: value for key, value in previous.items() if key != "history"}
        replaced.update(replaced_unix_time=now, walk_semantics_version=upgrade["walk_semantics_version"],
                        reason=upgrade["reason"])
        write_json(receipt_path, {"sha256": new_hash, "file": target.name, "source": upgrade["new"]["source"],
                                  "walk_semantics_version": upgrade["walk_semantics_version"],
                                  "history": [*receipt_history(previous), replaced]})


def verify_inputs(directory):
    if (directory / "STAGING_INCOMPLETE").exists():
        raise ValueError("input snapshot staging is incomplete")
    receipt = json.loads((directory / "input-receipt.json").read_text())
    for name, key in (("selection.json", "selection_sha256"), ("queries.json", "queries_sha256")):
        if digest(directory / name) != receipt[key]:
            raise ValueError(f"staged {name} digest changed")
    if "original_queries" in receipt:
        original = receipt["original_queries"]
        path = (directory / original["path"]).resolve()
        if (directory.resolve() not in path.parents or path.stat().st_size != original["bytes"]
                or digest(path) != original["sha256"]):
            raise ValueError("staged original query identity changed")
    for owner in receipt["owners"]:
        path = (directory / owner["path"]).resolve()
        if directory.resolve() not in path.parents or path.stat().st_size != owner["bytes"] or digest(path) != owner["sha256"]:
            raise ValueError(f"staged owner identity changed: {owner['mask']}")
    overlays = receipt.get("domain_rule_overlays", [])
    selection = json.loads((directory / "selection.json").read_bytes())
    declared = selection.get("domain_rule_overlays", [])
    fields = ("owner_mask", "path", "bytes")
    if ([tuple(row.get(key) for key in fields) for row in declared]
            != [tuple(row.get(key) for key in fields) for row in overlays]):
        raise ValueError("staged partial rule inventory differs from selection")
    for overlay in overlays:
        path = (directory / overlay["path"]).resolve()
        if (directory.resolve() not in path.parents or path.stat().st_size != overlay["bytes"]
                or digest(path) != overlay["sha256"]):
            raise ValueError(f"staged partial rule identity changed: {overlay['owner_mask']}")
    preferred = receipt.get("preferred_owner_programs", [])
    fields = ("owner_mask", "path", "bytes", "residual_policy", "rule_ordinals")
    if ([tuple(row.get(key) for key in fields) for row in selection.get("preferred_owner_programs", [])]
            != [tuple(row.get(key) for key in fields) for row in preferred]):
        raise ValueError("staged preferred program inventory differs from selection")
    for program in preferred:
        path = (directory / program["path"]).resolve()
        if (directory.resolve() not in path.parents or path.stat().st_size != program["bytes"]
                or digest(path) != program["sha256"]):
            raise ValueError(f"staged preferred program identity changed: {program['owner_mask']}")
    for attachment in receipt.get("attachments", []):
        path = (directory / attachment["path"]).resolve()
        if (path.parent != directory.resolve() or path.stat().st_size != attachment["bytes"]
                or digest(path) != attachment["sha256"]):
            raise ValueError(f"staged attachment identity changed: {attachment['name']}")
    query_path = directory / "queries.json"
    document = json.loads(query_path.read_text())
    if document.get("schema") != QUERY_SCHEMA or not isinstance(document.get("queries"), list) or not document["queries"]:
        raise ValueError("staged query document is not a nonempty owner-domain query set")
    return len(document["queries"]), query_path.stat().st_size, receipt


def selection_masks(selection_path):
    selection = json.loads(selection_path.read_text())
    owners = selection.get("owners")
    if not isinstance(owners, list) or not owners:
        raise ValueError("selection must contain a nonempty owner list")
    masks = [row.get("mask") for row in owners]
    if any(not isinstance(mask, str) or not mask or set(mask) - {"0", "1"} for mask in masks):
        raise ValueError("selection owner masks must be nonempty binary strings")
    return masks


def verify_query_override(path, masks):
    """A replacement query document: schema v2, only the six native row fields, selected owners only."""
    def unique_object(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON field: {key}")
            result[key] = value
        return result

    document = json.loads(Path(path).read_bytes(), object_pairs_hook=unique_object)
    if (not isinstance(document, dict) or document.get("schema") != QUERY_SCHEMA
            or not isinstance(document.get("queries"), list) or not document["queries"]):
        raise ValueError("query override must be a nonempty rustred.owner-domain-queries.json.v2 document")
    permitted = set(masks)
    arity = len(masks[0])
    ids = set()
    for index, row in enumerate(document["queries"]):
        if not isinstance(row, dict) or set(row) != QUERY_ROW_FIELDS:
            raise ValueError(f"query row {index} must contain exactly the fields {sorted(QUERY_ROW_FIELDS)}")
        if not isinstance(row["id"], str) or not 1 <= len(row["id"].encode("utf-8")) <= 128 or row["id"] in ids:
            raise ValueError(f"query row {index} needs a unique id of 1..128 UTF-8 bytes")
        ids.add(row["id"])
        owner = row["owner"]
        if not isinstance(owner, str) or len(owner) != arity or owner not in permitted:
            raise ValueError(f"query row {index} owner {owner!r} is not a selected owner mask")
        for name in ("lower", "upper"):
            if not isinstance(row[name], list) or len(row[name]) != arity:
                raise ValueError(f"query row {index} {name} must be an arity-{arity} list")
        if not isinstance(row["power_bounds"], dict):
            raise ValueError(f"query row {index} power_bounds must be an object")
    SUPERVISOR.ROLES.query_roles(document)
    return len(document["queries"])


def prepare_from(source_campaign, campaign, query_order, queries_override=None, attachments=()):
    """Copy existing input obligations, never mutate/resume/reorder the source.

    With queries_override the owner payloads and selection are copied from the
    source while the new, verified query document becomes the staged input;
    attachments (planner receipts) are copied read-only beside the inputs.
    """
    source_campaign = source_campaign.resolve(strict=True)
    destination = campaign.resolve()
    if (source_campaign == destination or source_campaign in destination.parents
            or destination in source_campaign.parents):
        raise ValueError("source and destination campaigns must be distinct and not nested")
    if campaign.exists() or campaign.is_symlink() or destination.exists():
        raise ValueError("fresh preparation requires a nonexistent destination campaign")
    source_inputs = (source_campaign / "inputs").resolve(strict=True)
    if (source_inputs == destination or source_inputs in destination.parents
            or destination in source_inputs.parents):
        raise ValueError("source inputs and destination campaign must be distinct and not nested")
    count, _, original = verify_inputs(source_inputs)  # Read-only; no source freezing or steering.
    spec = importlib.util.spec_from_file_location("campaign_input_stager", Path(__file__).with_name("stage_saved_owner_campaign.py"))
    stager = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(stager)
    attachments = stager.check_attachments(attachments)
    queries = source_inputs / "queries.json"
    if queries_override is not None:
        queries = Path(queries_override).resolve(strict=True)
        if destination in queries.parents:
            raise ValueError("query override must not live inside the destination campaign")
        count = verify_query_override(queries, selection_masks(source_inputs / "selection.json"))
    expected_query_hash = original["queries_sha256"] if queries_override is None else digest(queries)
    staged = stager.stage(source_inputs / "selection.json", queries, destination / "inputs", source_inputs,
                          query_order=query_order, attachments=attachments)
    if (staged["source_manifest_sha256"] != original["selection_sha256"]
            or staged["source_queries_sha256"] != expected_query_hash
            or [(row["mask"], row["bytes"], row["sha256"]) for row in staged["owners"]]
            != [(row["mask"], row["bytes"], row["sha256"]) for row in original["owners"]]
            or [(row["owner_mask"], row["bytes"], row["sha256"])
                for row in staged.get("domain_rule_overlays", [])]
            != [(row["owner_mask"], row["bytes"], row["sha256"])
                for row in original.get("domain_rule_overlays", [])]
            or [(row["owner_mask"], row["bytes"], row["sha256"], row["residual_policy"], row.get("rule_ordinals"))
                for row in staged.get("preferred_owner_programs", [])]
            != [(row["owner_mask"], row["bytes"], row["sha256"], row["residual_policy"], row.get("rule_ordinals"))
                for row in original.get("preferred_owner_programs", [])]
            or verify_inputs(source_inputs)[2] != original
            or verify_inputs(destination / "inputs")[0] != count):
        raise ValueError("source input identity changed during fresh preparation")
    return staged


def frozen_options(policy):
    """Frozen options with v1 defaults for options a v1 steering file never recorded."""
    options = dict(policy["options"])
    command = policy.get("command_arguments", [])
    # Existing steering without this field remains implicitly Off, with its
    # original options/argv untouched. New opt-in steering binds both copies.
    g2_mode = options.get("g2_residual_anchors", "off")
    g2_flag = "--" + SUPERVISOR.G2_RESIDUAL_ANCHORS
    g2_values = [command[index + 1] if index + 1 < len(command) else None
                 for index, flag in enumerate(command) if flag == g2_flag]
    alternate_spelling = any(isinstance(flag, str) and flag.startswith(g2_flag + "=") for flag in command)
    if (g2_mode not in SUPERVISOR.G2_RESIDUAL_MODES or alternate_spelling
            or g2_values != (["union"] if g2_mode == "union" else [])):
        raise ValueError("frozen G2 residual-anchor mode and command disagree; use a new campaign directory")

    def flag_value(flag):
        return command[command.index(flag) + 1] if command.count(flag) == 1 else None

    if "publication_policy" not in options:
        options["publication_policy"] = flag_value("--publication-policy") or "ordered"
    lookup_flag = "--" + SUPERVISOR.DOMAIN.EPOCH_INSPECTOR_LOOKUP
    lookup_mode = options.get("epoch_inspector_lookup", "all-miss")
    lookup_values = [command[index + 1] if index + 1 < len(command) else None
                     for index, flag in enumerate(command) if flag == lookup_flag]
    alternate = any(isinstance(flag, str) and flag.startswith(lookup_flag + "=") for flag in command)
    if (lookup_mode not in SUPERVISOR.DOMAIN.EPOCH_INSPECTOR_LOOKUP_MODES or alternate
            or lookup_values != (["snapshot"] if lookup_mode == "snapshot" else [])
            or (options["publication_policy"] != "epoch"
                and ("epoch_inspector_lookup" in options or lookup_values))):
        raise ValueError("frozen Epoch inspector lookup mode and command disagree; use a new campaign directory")
    if options["publication_policy"] == "epoch":
        options.setdefault("epoch_inspector_lookup", "all-miss")
    rolling_flag = "--" + SUPERVISOR.DOMAIN.EPOCH_ROLLING
    rolling = options.get("epoch_rolling", False)
    if (type(rolling) is not bool or command.count(rolling_flag) != int(rolling)
            or any(isinstance(flag, str) and flag.startswith(rolling_flag + "=") for flag in command)
            or (rolling and options["publication_policy"] != "epoch")):
        raise ValueError("frozen Epoch rolling mode and command disagree; use a new campaign directory")
    if options["publication_policy"] == "epoch":
        options.setdefault("epoch_rolling", False)
    dispatch_flag = "--" + SUPERVISOR.DOMAIN.EPOCH_DISPATCH
    dispatch = options.get("epoch_dispatch", "fifo")
    dispatch_values = [command[index + 1] if index + 1 < len(command) else None
                       for index, flag in enumerate(command) if flag == dispatch_flag]
    if (dispatch not in SUPERVISOR.DOMAIN.EPOCH_DISPATCH_POLICIES
            or dispatch_values != (["adaptive"] if dispatch == "adaptive" else [])
            or any(isinstance(flag, str) and flag.startswith(dispatch_flag + "=") for flag in command)
            or (dispatch == "adaptive" and not rolling)
            or (options["publication_policy"] != "epoch" and ("epoch_dispatch" in options or dispatch_values))):
        raise ValueError("frozen Epoch dispatch policy and command disagree; use a new campaign directory")
    if options["publication_policy"] == "epoch":
        options.setdefault("epoch_dispatch", "fifo")
    for option in SUPERVISOR.DOMAIN.EPOCH_DATA_OPTIONS:
        key = option.replace("-", "_")
        value = options.get(key)
        flag = "--" + option
        values = [command[index + 1] if index + 1 < len(command) else None
                  for index, item in enumerate(command) if item == flag]
        if (values != ([] if value is None else [str(value)])
                or any(isinstance(item, str) and item.startswith(flag + "=") for item in command)):
            raise ValueError("frozen Epoch batch options and command disagree; use a new campaign directory")
    SUPERVISOR.DOMAIN.validate_epoch_batch(
        options.get("epoch_publication_order"), options.get("epoch_cut_size"), options.get("epoch_window"),
        rolling, True, options["publication_policy"], True,
        options.get("epoch_result_escrow_jobs"), options.get("epoch_result_escrow_bytes"))
    if "transfer_unreserved_lookahead" not in options:
        lookahead = flag_value("--transfer-unreserved-lookahead")
        options["transfer_unreserved_lookahead"] = 256 if lookahead is None else int(lookahead)
    if "inspection_workers" not in options:
        inspectors = flag_value("--inspection-workers")
        options["inspection_workers"] = None if inspectors is None else int(inspectors)
    SUPERVISOR.DOMAIN.validate_epoch_preparation(
        options, True, options["publication_policy"], options.get("workers", 1), options["inspection_workers"])
    # Older frozen policies omitted this opt-in and therefore mean Off.
    options.setdefault("apply_cell_refinement_max_cardinality", None)
    if "frontier_policy" not in options:
        options["frontier_policy"] = flag_value("--frontier-policy") or LEGACY_FRONTIER_POLICY
    for name in OPTIONAL_RAM_POLICY_OPTIONS:
        if name not in options:
            value = flag_value("--" + name.replace("_", "-"))
            options[name] = None if value is None else (float(value) if name == "swap_growth_stop_seconds" else int(value))
    # Steering before v4 had no automatic frontier rescue.
    options.setdefault("auto_rescue", False)
    options.setdefault("helper_id_prefix", None)
    options.setdefault("max_rescues", None)
    return options


def native_command(options, executable, inputs, count, size):
    """The supervisor argument list is built from frozen options only."""
    command = ["--executable", str(executable), "--manifest", str(inputs / "selection.json"),
               "--queries", str(inputs / "queries.json"), "--owner-base", str(inputs),
               "--workers", str(options["workers"]), "--cpus", options["cpus"],
               "--max-memory-bytes", str(options["max_memory_bytes"]),
               "--ram-guard-margin-percent", str(options["ram_guard_margin_percent"]),
               "--unbounded-work", "--max-queries", str(count), "--max-query-bytes", str(size),
               "--bounded-refinement-axes", "finite-axes", "--max-guard-univariate-degree", "64",
               "--publication-policy", options["publication_policy"], "--route-domain-overcover",
               "--transfer-unreserved-lookahead", str(options["transfer_unreserved_lookahead"]),
               "--reuse-initial-d-bands",
               "--checkpoint-interval-seconds", str(options["checkpoint_interval_seconds"])]
    if options["apply_subdivision_axis"] is not None:
        command += ["--apply-subdivision-axis", str(options["apply_subdivision_axis"]),
                    "--apply-subdivision-cut", str(options["apply_subdivision_cut"])]
    if options["apply_cell_refinement_max_cardinality"] is not None:
        command += ["--apply-cell-refinement-max-cardinality",
                    str(options["apply_cell_refinement_max_cardinality"])]
    if options["inspection_workers"] is not None:
        command += ["--inspection-workers", str(options["inspection_workers"])]
    if options.get("g2_residual_anchors", "off") == "union":
        command += ["--g2-residual-anchors", "union"]
    if options.get("epoch_inspector_lookup") == "snapshot":
        command += ["--epoch-inspector-lookup", "snapshot"]
    if options.get("epoch_rolling", False):
        command.append("--epoch-rolling")
    if options.get("epoch_dispatch") == "adaptive":
        command += ["--epoch-dispatch", "adaptive"]
    for option in SUPERVISOR.DOMAIN.EPOCH_DATA_OPTIONS:
        if (value := options.get(option.replace("-", "_"))) is not None:
            command += ["--" + option, str(value)]
    command += ["--frontier-policy", options["frontier_policy"]]
    for name in OPTIONAL_RAM_POLICY_OPTIONS:
        if options[name] is not None:
            command += ["--" + name.replace("_", "-"), str(options[name])]
    if options.get("auto_rescue"):
        command += ["--auto-rescue", "--helper-id-prefix", options["helper_id_prefix"],
                    "--max-rescues", str(options["max_rescues"]),
                    "--amendments-directory", str(inputs.parent / AMENDMENTS_DIRECTORY)]
    return command


def frozen_policy(campaign, args, executable, inputs, count, size):
    """Persist original steering; only per-invocation supervisor RAM may differ."""
    path = campaign / "bin" / "steering.json"
    if path.exists():
        policy = json.loads(path.read_text())
        if policy.get("schema") not in STEERING_SCHEMAS:
            raise ValueError("unknown frozen steering policy")
        frozen = frozen_options(policy)
        for name in FROZEN_OPTIONS:
            supplied = getattr(args, name, None)
            if name == "cpus" and supplied is not None:
                # Ranges and lists naming the same CPUs are the same frozen set.
                supplied = SUPERVISOR.format_cpu_set(SUPERVISOR.parse_cpu_set(supplied))
            expected = frozen.get(name, "off" if name == "g2_residual_anchors" else None)
            if supplied is not None and supplied != expected:
                if (getattr(args, "resume", False) or getattr(args, "start", False)) and name in RAM_POLICY_OPTIONS:
                    continue
                raise ValueError(f"--{name.replace('_', '-')} differs from frozen policy; use a new campaign directory")
        return policy
    if getattr(args, "resume", False):
        raise ValueError("resume requires the original frozen steering.json; refusing to guess native policy")
    options = {name: getattr(args, name, None) for name in FROZEN_OPTIONS}
    affinity = os.sched_getaffinity(0)
    defaults = {"workers": min(50, len(affinity)),
                "checkpoint_interval_seconds": 3600, "max_memory_bytes": 500_000_000_000,
                "ram_guard_margin_percent": 5.0, "publication_policy": DEFAULT_PUBLICATION_POLICY,
                "transfer_unreserved_lookahead": 256, "frontier_policy": DEFAULT_FRONTIER_POLICY,
                "g2_residual_anchors": "off",
                "host_memory_reserve_bytes": SUPERVISOR.DEFAULT_HOST_MEMORY_RESERVE_BYTES,
                "swap_growth_stop_bytes_per_second": SUPERVISOR.DEFAULT_SWAP_GROWTH_STOP_BYTES_PER_SECOND,
                "swap_growth_stop_seconds": SUPERVISOR.DEFAULT_SWAP_GROWTH_STOP_SECONDS,
                "helper_id_prefix": DEFAULT_HELPER_ID_PREFIX, "max_rescues": DEFAULT_MAX_RESCUES}
    for name, default in defaults.items():
        if options[name] is None:
            options[name] = default
    if options["auto_rescue"] is None:
        # Rescue resumes after a frontier STOP; a record campaign never stops.
        options["auto_rescue"] = (options["frontier_policy"] == "stop"
                                  and options["publication_policy"] != "epoch")
    if not 1 <= options["workers"] <= MAX_WORKERS:
        raise ValueError(f"workers must be in 1..{MAX_WORKERS}")
    cpus = (SUPERVISOR.parse_cpu_set(options["cpus"]) if options["cpus"] else
            set(sorted(affinity)[:options["workers"]]))
    if len(cpus) != options["workers"] or not cpus <= affinity:
        raise ValueError("CPU affinity must contain exactly the requested number of permitted CPUs")
    options["cpus"] = SUPERVISOR.format_cpu_set(cpus)
    if options["publication_policy"] not in ("ordered", "ready", "epoch"):
        raise ValueError("publication policy must be ordered, ready or epoch")
    if options["publication_policy"] == "epoch":
        if options["epoch_inspector_lookup"] is None:
            options["epoch_inspector_lookup"] = "all-miss"
        if options["epoch_rolling"] is None:
            options["epoch_rolling"] = False
        if options["epoch_dispatch"] is None:
            options["epoch_dispatch"] = "fifo"
        if options["transfer_unreserved_lookahead"] is None:
            raise ValueError("epoch requires --transfer-unreserved-lookahead")
    SUPERVISOR.DOMAIN.validate_epoch_inspector_lookup(
        options["epoch_inspector_lookup"], True, options["publication_policy"], True)
    SUPERVISOR.DOMAIN.validate_epoch_rolling(
        options["epoch_rolling"], True, options["publication_policy"], True)
    SUPERVISOR.DOMAIN.validate_epoch_dispatch(
        options["epoch_dispatch"], options["epoch_rolling"], True, options["publication_policy"], True)
    SUPERVISOR.DOMAIN.validate_epoch_batch(
        options["epoch_publication_order"], options["epoch_cut_size"], options["epoch_window"],
        options["epoch_rolling"], True, options["publication_policy"], True,
        options.get("epoch_result_escrow_jobs"), options.get("epoch_result_escrow_bytes"))
    SUPERVISOR.DOMAIN.validate_epoch_preparation(
        options, True, options["publication_policy"], options["workers"], options["inspection_workers"])
    # Keep default and historical steering/argv free of new optional fields.
    for option in SUPERVISOR.DOMAIN.EPOCH_DATA_OPTIONS:
        key = option.replace("-", "_")
        if options[key] is None:
            del options[key]
    if options["publication_policy"] != "epoch":
        del options["epoch_inspector_lookup"]  # Historical CP5 options/argv remain absent.
        del options["epoch_rolling"]
        del options["epoch_dispatch"]
    if options["frontier_policy"] not in ("record", "stop"):
        raise ValueError("frontier policy must be record or stop")
    if options["auto_rescue"] and options["frontier_policy"] != "stop":
        raise ValueError("automatic frontier rescue requires --frontier-policy stop (or --no-auto-rescue)")
    if options["auto_rescue"] and (not options["helper_id_prefix"] or options["max_rescues"] < 1):
        raise ValueError("automatic frontier rescue needs a nonempty --helper-id-prefix and --max-rescues >= 1")
    if options["auto_rescue"]:
        SUPERVISOR.ROLES.query_roles(SUPERVISOR.ROLES.loads_document((inputs / "queries.json").read_text()), require_explicit=True)
    if options["publication_policy"] != "ordered" and options["apply_subdivision_axis"] is not None:
        raise ValueError("physical subdivision requires --publication-policy ordered")
    SUPERVISOR.validate_g2_residual_anchors(
        options["g2_residual_anchors"], options["transfer_unreserved_lookahead"],
        options["publication_policy"], options["apply_subdivision_axis"] is not None)
    inspectors = options["inspection_workers"]
    if inspectors is not None:
        available = 1 if options["workers"] == 1 else options["workers"] - 1
        if not 1 <= inspectors <= available:
            raise ValueError("inspection workers must be positive and leave one coordinator when workers > 1")
    policy = {"schema": STEERING_SCHEMA, "options": options,
              "command_arguments": native_command(options, executable, inputs, count, size)}
    write_json(path, policy)
    path.chmod(0o444)
    return policy


def memory_admission_preview(options):
    """The supervisor's admission rule applied to the host now (it re-evaluates it at launch)."""
    reserve = options.get("host_memory_reserve_bytes") or SUPERVISOR.DEFAULT_HOST_MEMORY_RESERVE_BYTES
    try:
        snapshot = SUPERVISOR.host_memory()
        hard, soft, reserve = SUPERVISOR.memory_admission(
            options["max_memory_bytes"], None, snapshot, reserve, options["ram_guard_margin_percent"])
    except (OSError, ValueError) as error:
        return {"rule": SUPERVISOR.ADMISSION_RULE, "error": str(error), "evaluated_unix_time": time.time()}
    return dict(SUPERVISOR.memory_admission_record(options["max_memory_bytes"], snapshot, reserve, hard, soft),
                evaluated_unix_time=time.time(), scope="preview at planning time; the supervisor re-evaluates at launch")


def effective_supervisor_policy(policy, args):
    """Overlay start/resume RAM settings without rewriting frozen solver policy."""
    options = frozen_options(policy)
    command = list(policy["command_arguments"])
    overrides = {}
    if args.resume or getattr(args, "start", False):
        for name in RAM_POLICY_OPTIONS:
            supplied = getattr(args, name)
            if supplied is not None and supplied != options[name]:
                overrides[name] = supplied
                options[name] = supplied
                flag = "--" + name.replace("_", "-")
                if name in OPTIONAL_RAM_POLICY_OPTIONS and command.count(flag) == 0:
                    command += [flag, str(supplied)]  # Older steering relied on the default.
                    continue
                if command.count(flag) != 1:
                    raise ValueError(f"frozen steering must contain exactly one {flag}")
                command[command.index(flag) + 1] = str(supplied)
    return command, options, overrides


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--campaign-directory", type=Path,
                        default=Path.cwd() / "campaigns/five-loop-saved")
    parser.add_argument("--executable", type=Path, help="freeze once; later cargo rebuilds cannot change this campaign")
    parser.add_argument("--prepare-from", type=Path, metavar="SOURCE_CAMPAIGN",
                        help="copy verified existing inputs into a nonexistent, disjoint campaign; does not launch")
    parser.add_argument("--queries", type=Path, metavar="NEW_QUERIES",
                        help="only with --prepare-from: stage this verified query document instead of the source queries")
    parser.add_argument("--attach", type=Path, action="append", default=[], metavar="FILE",
                        help="only with --prepare-from: copy a planner receipt read-only beside the inputs; repeatable")
    parser.add_argument("--query-order", choices=("preserve", "helpers-first"),
                        help="only with --prepare-from; default helpers-first (preserve when --queries is given); never rewrites existing inputs")
    parser.add_argument("--start", action="store_true", help="manually launch after preparation")
    parser.add_argument("--resume", action="store_true",
                        help="continue the latest native checkpoint with the frozen executable, or with "
                             "--upgrade-executable onto a semantics-compatible replacement")
    postprocess = parser.add_mutually_exclusive_group()
    postprocess.add_argument("--refine-masters", "--masters-only", "--master-reduction",
                            dest="refine_masters", action="store_true",
                            help="explicit refinement only: require the current completed artifact; never start a solve")
    postprocess.add_argument("--publish-only", action="store_true",
                            help="publish an already completed current checkpoint; never start a solve")
    parser.add_argument("--master-seed-depth", type=int,
                        help="phase-two ordinary-IBP seed depth (default 0); may increase with exact previous-row reuse")
    parser.add_argument("--master-containing-sector-depth", type=int,
                        help="only with --refine-masters: promote up to N inactive indices for extra ordinary IBP sources (default saved preference or 0)")
    parser.add_argument("--master-normalization-profile", choices=("conservative", "standard"),
                        help="only with --refine-masters: finite normalization budget (default saved preference, then source profile)")
    parser.add_argument("--master-collection-artifact", type=Path, action="append", default=None,
                        help="only with --refine-masters: compatible published terminal inventory to collect alongside this campaign (repeatable)")
    parser.add_argument("--master-finite-feedback", action=argparse.BooleanOptionalAction, default=None,
                        help="combine retained finite rows after full-U aliases (explicit refinement only; default saved preference or on)")
    parser.add_argument("--master-saved-rule-assistance", action=argparse.BooleanOptionalAction, default=None,
                        help="only with --refine-masters: add saved-rule/routing equations; omitted resumes the saved preference")
    parser.add_argument("--master-circuit-symmetry-assistance", action=argparse.BooleanOptionalAction, default=None,
                        help="only with --refine-masters: add native circuit-reflection equations; omitted resumes the saved preference")
    parser.add_argument("--master-reduction-executable", type=Path,
                        help="freeze a separate publisher/refiner binary; leaves the original campaign binary and CP6 untouched")
    parser.add_argument("--upgrade-executable", type=Path, metavar="NEW",
                        help="only with --resume: freeze NEW in place of the frozen binary when its "
                             "walk-semantics-version equals the checkpoint's, or return to a binary that "
                             "bin/executable.json history lists; without --start a read-only dry run")
    parser.add_argument("--workers", type=int, help=f"initial default: at most 50 permitted CPUs (cap {MAX_WORKERS}); frozen for resume")
    parser.add_argument("--cpus", help="optional explicit affinity: comma list or ranges (128-177, 0-3,8); exactly --workers IDs")
    parser.add_argument("--run-directory", type=Path)
    parser.add_argument("--publication-policy", choices=("ordered", "ready", "epoch"),
                        help=f"initial default: {DEFAULT_PUBLICATION_POLICY}; ordered remains selectable; frozen for resume")
    parser.add_argument("--epoch-inspector-lookup", choices=SUPERVISOR.DOMAIN.EPOCH_INSPECTOR_LOOKUP_MODES,
                        action=SUPERVISOR.DOMAIN.StoreOnce,
                        help="explicit CP6 Epoch comparison control; initial default all-miss; frozen on resume")
    parser.add_argument("--epoch-rolling", action=SUPERVISOR.DOMAIN.StoreTrueOnce, nargs=0, default=None,
                        help="opt into bounded rolling CP6 execution; frozen on resume")
    parser.add_argument("--epoch-dispatch", choices=SUPERVISOR.DOMAIN.EPOCH_DISPATCH_POLICIES,
                        action=SUPERVISOR.DOMAIN.StoreOnce,
                        help="pending-job dispatch; adaptive requires rolling; frozen on resume")
    SUPERVISOR.DOMAIN.add_epoch_batch_arguments(parser)
    SUPERVISOR.DOMAIN.add_epoch_preparation_arguments(parser)
    parser.add_argument("--transfer-unreserved-lookahead", type=int,
                        help="initial default: 256 logical dispatch lookahead; frozen for resume")
    parser.add_argument("--g2-residual-anchors", choices=SUPERVISOR.G2_RESIDUAL_MODES,
                        action=SUPERVISOR.DOMAIN.StoreOnce,
                        help="fresh-campaign opt-in, initial default off; union requires no physical subdivision; "
                             "frozen for resume (no off-to-union activation here)")
    parser.add_argument("--inspection-workers", type=int,
                        help="explicit native inspectors (default: native split); frozen for resume")
    parser.add_argument("--checkpoint-interval-seconds", type=int, help="initial default: 3600")
    parser.add_argument("--max-memory-bytes", type=int,
                        help="positive requested RAM ceiling; initial default: 500000000000; may override per start/resume")
    parser.add_argument("--ram-guard-margin-percent", type=float,
                        help="initial default: 5 (save+stop at 95%%); may override per resume")
    parser.add_argument("--host-memory-reserve-bytes", "--host-available-floor-bytes",
                        dest="host_memory_reserve_bytes", type=int,
                        help="host MemAvailable floor of the cooperative save+stop (hard stop at a quarter); "
                             f"initial default: {SUPERVISOR.DEFAULT_HOST_MEMORY_RESERVE_BYTES} (50 GB), frozen explicitly; "
                             "may override per resume")
    parser.add_argument("--swap-growth-stop-bytes-per-second", type=int,
                        help="save+stop when the campaign's own swapped-out memory grows at or above this rate "
                             "for the window; 0 disables; initial default: "
                             f"{SUPERVISOR.DEFAULT_SWAP_GROWTH_STOP_BYTES_PER_SECOND}; may override per resume")
    parser.add_argument("--swap-growth-stop-seconds", type=float,
                        help="sustained own swap-growth window; initial default: "
                             f"{SUPERVISOR.DEFAULT_SWAP_GROWTH_STOP_SECONDS:g}; may override per resume")
    parser.add_argument("--max-zero-progress-ram-stops", type=int, default=DEFAULT_MAX_ZERO_PROGRESS_RAM_STOPS,
                        help="refuse --resume after this many consecutive RAM-guard stops without committed "
                             f"progress (default {DEFAULT_MAX_ZERO_PROGRESS_RAM_STOPS}; 0 disables); raise the RAM "
                             "cap or investigate the head that trips the guard instead of looping")
    parser.add_argument("--frontier-policy", choices=("record", "stop"),
                        help=f"initial default: {DEFAULT_FRONTIER_POLICY} (save+stop at the first frontier, exit 4); "
                             "frozen for resume")
    rescue = parser.add_mutually_exclusive_group()
    rescue.add_argument("--auto-rescue", dest="auto_rescue", action="store_const", const=True, default=None,
                        help="initial default with --frontier-policy stop: after a frontier stop of a known class the "
                             "supervisor writes the next amendment (campaign/amendments) and resumes automatically; "
                             "frozen for resume")
    rescue.add_argument("--no-auto-rescue", dest="auto_rescue", action="store_const", const=False,
                        help="freeze a campaign without automatic frontier rescue (a frontier stop waits for the owner)")
    parser.add_argument("--helper-id-prefix",
                        help=f"cosmetic prefix for appended auxiliary IDs; initial default: {DEFAULT_HELPER_ID_PREFIX}; "
                             "scope comes only from the bound query_roles declaration")
    parser.add_argument("--max-rescues", type=int,
                        help=f"automatic rescue resumes per campaign; initial default: {DEFAULT_MAX_RESCUES}; frozen for resume")
    parser.add_argument("--apply-subdivision-axis", type=int)
    parser.add_argument("--apply-subdivision-cut", type=int)
    parser.add_argument("--apply-cell-refinement-max-cardinality", type=application_cardinality, action="append",
                        help="opt-in singleton refinement eligibility; positive cardinality, default off, unchanged by unbounded work; frozen for resume")
    parser.add_argument("--json", action="store_true", help="print the prepared command as JSON")
    args = parser.parse_args(argv)
    if args.master_seed_depth is not None and args.master_seed_depth < 0:
        parser.error("--master-seed-depth must be nonnegative")
    if args.upgrade_executable is not None:
        if args.prepare_from is not None:
            parser.error("--upgrade-executable cannot be combined with --prepare-from")
        if not args.resume:
            parser.error("--upgrade-executable requires --resume")
        if args.executable is not None:
            parser.error("--upgrade-executable replaces --executable; supply only the new binary")
    if args.prepare_from is not None and args.resume:
        parser.error("--prepare-from cannot be combined with --resume")
    if args.query_order is not None and args.prepare_from is None:
        parser.error("--query-order requires --prepare-from; existing inputs cannot be reordered")
    if (args.queries is not None or args.attach) and args.prepare_from is None:
        parser.error("--queries and --attach require --prepare-from; existing inputs are immutable")
    cardinalities = args.apply_cell_refinement_max_cardinality
    if cardinalities is not None and len(cardinalities) != 1:
        parser.error("--apply-cell-refinement-max-cardinality may be supplied only once")
    args.apply_cell_refinement_max_cardinality = cardinalities[0] if cardinalities else None
    if ((args.workers is not None and not 1 <= args.workers <= MAX_WORKERS) or
            (args.checkpoint_interval_seconds is not None and args.checkpoint_interval_seconds <= 0)):
        parser.error(f"workers must be in 1..{MAX_WORKERS} and checkpoint interval must be positive")
    if ((args.transfer_unreserved_lookahead is not None and args.transfer_unreserved_lookahead <= 0) or
            (args.inspection_workers is not None and args.inspection_workers <= 0)):
        parser.error("transfer lookahead and inspection workers must be positive")
    if ((args.max_memory_bytes is not None and args.max_memory_bytes <= 0) or
            (args.ram_guard_margin_percent is not None and not 0 < args.ram_guard_margin_percent < 100)):
        parser.error("RAM limit must be positive and guard margin strictly between 0 and 100 percent")
    if args.max_zero_progress_ram_stops < 0:
        parser.error("--max-zero-progress-ram-stops must be nonnegative")
    if ((args.host_memory_reserve_bytes is not None and args.host_memory_reserve_bytes <= 0) or
            (args.swap_growth_stop_bytes_per_second is not None and args.swap_growth_stop_bytes_per_second < 0) or
            (args.swap_growth_stop_seconds is not None and not 0 < args.swap_growth_stop_seconds < float("inf"))):
        parser.error("host memory floor must be positive, swap-growth rate nonnegative and swap-growth window "
                     "positive and finite")
    if (args.apply_subdivision_axis is None) != (args.apply_subdivision_cut is None):
        parser.error("subdivision requires both axis and cut")
    if args.publication_policy in ("ready", "epoch") and args.apply_subdivision_axis is not None:
        parser.error("nonordered publication cannot be combined with physical subdivision")
    if any(value is not None and value < 0 for value in (args.apply_subdivision_axis, args.apply_subdivision_cut)):
        parser.error("subdivision axis and cut must be nonnegative")
    try:
        SUPERVISOR.validate_g2_residual_anchors(
            args.g2_residual_anchors, args.transfer_unreserved_lookahead or 256,
            args.publication_policy or DEFAULT_PUBLICATION_POLICY, args.apply_subdivision_axis is not None)
    except ValueError as error:
        parser.error(str(error))
    campaign = args.campaign_directory.resolve()
    phases = PHASES
    try:
        phase_policy = phases.configuration(campaign, args.refine_masters, args.master_seed_depth,
                                            args.master_reduction_executable,
                                            saved_rule_assistance=args.master_saved_rule_assistance,
                                            containing_sector_depth=args.master_containing_sector_depth,
                                            circuit_symmetry_assistance=args.master_circuit_symmetry_assistance,
                                            normalization_profile=args.master_normalization_profile,
                                            collection_artifacts=args.master_collection_artifact,
                                            finite_feedback=args.master_finite_feedback)
    except (OSError, ValueError, TypeError) as error:
        parser.error(str(error))
    if (args.epoch_inspector_lookup is not None or args.epoch_rolling or args.epoch_dispatch is not None
            or any(getattr(args, name.replace("-", "_")) is not None
                   for name in SUPERVISOR.DOMAIN.EPOCH_DATA_OPTIONS)) \
            and not (campaign / "bin" / "steering.json").is_file():
        # An existing policy supplies publication on an ordinary resume/plan.
        # Fresh explicit misuse is rejected before staging/freezing any input.
        selected_policy = args.publication_policy
        if selected_policy is None and not (campaign / "bin" / "steering.json").is_file():
            selected_policy = DEFAULT_PUBLICATION_POLICY
        if selected_policy is not None:
            try:
                SUPERVISOR.DOMAIN.validate_epoch_inspector_lookup(
                    args.epoch_inspector_lookup, True, selected_policy, True)
                SUPERVISOR.DOMAIN.validate_epoch_rolling(args.epoch_rolling, True, selected_policy, True)
                SUPERVISOR.DOMAIN.validate_epoch_dispatch(
                    args.epoch_dispatch, args.epoch_rolling, True, selected_policy, True)
                SUPERVISOR.DOMAIN.validate_epoch_batch(
                    args.epoch_publication_order, args.epoch_cut_size, args.epoch_window,
                    args.epoch_rolling, True, selected_policy, True,
                    args.epoch_result_escrow_jobs, args.epoch_result_escrow_bytes)
                SUPERVISOR.DOMAIN.validate_epoch_preparation(
                    vars(args), True, selected_policy, args.workers or min(50, len(os.sched_getaffinity(0))),
                    args.inspection_workers)
            except ValueError as error:
                parser.error(str(error))
    inputs = campaign / "inputs"
    checkpoint = campaign / "checkpoints" / "main"
    upgrade = None
    try:
        if args.prepare_from is not None:
            if args.executable is None or not args.executable.is_file() or not os.access(args.executable, os.X_OK):
                raise ValueError("fresh preparation requires an executable --executable")
            default_order = "preserve" if args.queries is not None else "helpers-first"
            prepare_from(args.prepare_from, args.campaign_directory, args.query_order or default_order,
                         queries_override=args.queries, attachments=args.attach)
        count, size, receipt = verify_inputs(inputs)
        executable, executable_hash = freeze_executable(campaign, args.executable)
        # Read-only on --resume (steering must exist), so every frozen-option
        # refusal happens before an upgrade changes anything.
        policy = frozen_policy(campaign, args, executable, inputs, count, size)
        if frozen_options(policy).get("auto_rescue"):
            SUPERVISOR.ROLES.query_roles(SUPERVISOR.ROLES.loads_document((inputs / "queries.json").read_text()), require_explicit=True)
        if args.upgrade_executable is not None:
            upgrade = plan_executable_upgrade(campaign, checkpoint, args.upgrade_executable,
                                              executable, executable_hash)
        steering_sha256 = digest(campaign / "bin" / "steering.json")
        _, steered = steering_executable(policy)
        if steered.resolve() != executable and not (upgrade and str(steered.resolve()) == upgrade["new"]["path"]):
            raise ValueError(f"frozen steering executable {steered} differs from the frozen receipt's {executable} "
                             "(interrupted --upgrade-executable: rerun it; or a moved campaign)")
        if upgrade is not None:
            upgrade["steering_sha256_before"] = steering_sha256
            policy = upgraded_steering(policy, upgrade, None)
            effective_supervisor_policy(policy, args)  # RAM overrides, validated before any change.
            if args.start:
                apply_executable_upgrade(campaign, checkpoint, upgrade, args.upgrade_executable)
                executable, executable_hash = freeze_executable(campaign, None)
                policy = frozen_policy(campaign, args, executable, inputs, count, size)
                steering_sha256 = digest(campaign / "bin" / "steering.json")
                if steering_executable(policy)[1].resolve() != executable:
                    raise ValueError("upgraded steering and receipt name different executables")
            else:
                # Dry run: show the command the upgrade would launch, change nothing.
                executable_hash = upgrade["new"]["sha256"]
                steering_sha256 = None  # Only --start rewrites steering.json.
        command_arguments, options, ram_overrides = effective_supervisor_policy(policy, args)
        liveness = ram_guard_liveness(campaign) if args.resume else None
        if (liveness is not None and args.max_zero_progress_ram_stops
                and liveness["consecutive_zero_progress_ram_stops"] >= args.max_zero_progress_ram_stops):
            raise ValueError(
                f"the last {liveness['consecutive_zero_progress_ram_stops']} runs were stopped by the RAM guard "
                f"({', '.join(liveness['stop_reasons'])}) without committed progress "
                f"(at {liveness['progress']}); resuming would repeat it: raise --max-memory-bytes or the host "
                "floor, or investigate the head in flight; --max-zero-progress-ram-stops 0 overrides")
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    run = args.run_directory.resolve() if args.run_directory else campaign / "runs" / timestamp
    supervisor = Path(__file__).with_name("shared_owner_campaign.py").resolve()
    command = [sys.executable, str(supervisor), *command_arguments,
               "--run-directory", str(run), "--resume" if args.resume else "--checkpoint", str(checkpoint)]
    # Every rescue amendment of the chain, in order (append-only; the native
    # checks each digest against the checkpoint and applies unrecorded ones).
    amendments = sorted((campaign / AMENDMENTS_DIRECTORY).glob("amendment-*.json")) if args.resume else []
    for amendment in amendments:
        command += ["--amend-queries", str(amendment)]
    attachments = receipt.get("attachments", [])
    entry_plan = next(({"path": str(inputs / row["path"]), "sha256": row["sha256"], "bytes": row["bytes"]}
                       for row in attachments if row.get("name") == ENTRY_PLAN_RECEIPT_NAME), None)
    plan = {"command": command, "campaign_directory": str(campaign), "run_directory": str(run),
            "checkpoint_directory": str(checkpoint), "executable_sha256": executable_hash,
            "selection_sha256": receipt["selection_sha256"], "queries_sha256": receipt["queries_sha256"],
            "query_count": count, "query_bytes": size,
            "anchor_plan": receipt.get("anchor_plan"),
            "query_order": receipt.get("query_order", "preserve"),
            "query_order_plan": receipt.get("query_order_plan"),
            "attachments": [{key: row[key] for key in ("name", "path", "bytes", "sha256")} for row in attachments],
            "entry_plan_receipt": entry_plan,
            "requested_workers": options["workers"], "cpus": options["cpus"], "hard_timeout_seconds": None,
            "publication_policy": options["publication_policy"],
            "g2_residual_anchors": options.get("g2_residual_anchors", "off"),
            "transfer_unreserved_lookahead": options["transfer_unreserved_lookahead"],
            "inspection_workers": options["inspection_workers"],
            "frontier_policy": options["frontier_policy"],
            "auto_rescue": {name: options[name] for name in RESCUE_OPTIONS},
            "amendments": [str(path) for path in amendments],
            "requested_hard_memory_bytes": options["max_memory_bytes"],
            "ram_guard_margin_percent": options["ram_guard_margin_percent"],
            "supervisor_ram_policy": {name: options[name] for name in RAM_POLICY_OPTIONS},
            "supervisor_ram_overrides": ram_overrides,
            "supervisor_ram_override_scope": "this_invocation_only; omitted_values_use_original_frozen_policy",
            "checkpoint_interval_seconds": options["checkpoint_interval_seconds"],
            "steering_policy": policy, "steering_policy_sha256": steering_sha256,
            "unbounded_cumulative_work": True, "scratch_and_algebra_admission_remain_bounded": True,
            "family_closure_claim": False, "launch_requested": args.start}
    if upgrade is not None:
        plan["executable_upgrade"] = dict(upgrade, applied=args.start)
    if options["publication_policy"] == "epoch":
        plan["epoch_inspector_lookup"] = options.get("epoch_inspector_lookup", "all-miss")
        plan["epoch_rolling"] = options.get("epoch_rolling", False)
        plan["epoch_dispatch"] = options.get("epoch_dispatch", "fifo")
        plan["epoch_publication_order"] = options.get("epoch_publication_order", "oldest-prefix")
        plan["epoch_cut_size"] = options.get("epoch_cut_size", 16)
        plan["epoch_window"] = options.get("epoch_window")
        plan.update({name.replace("-", "_"): options[name.replace("-", "_")]
                     for name in SUPERVISOR.DOMAIN.EPOCH_ESCROW_OPTIONS
                     if name.replace("-", "_") in options})
        plan.update({name.replace("-", "_"): options[name.replace("-", "_")]
                     for name in SUPERVISOR.DOMAIN.EPOCH_PREPARATION_OPTIONS
                     if name.replace("-", "_") in options})
        plan["epoch_checkpoint"] = {
            "inspector_lookup_mode": options.get("epoch_inspector_lookup", "all-miss"),
            "rolling": options.get("epoch_rolling", False),
            "dispatch": options.get("epoch_dispatch", "fifo"),
            "publication_order": options.get("epoch_publication_order", "oldest-prefix"),
            "cut_size": options.get("epoch_cut_size", 16),
            "requested_window": options.get("epoch_window"),
            "format": "RUSTRED-WALK-CP6", "schema": 3, "walk_semantics_version": 4,
            "resumable": True, "terminal_output": "checkpoint_only",
            "completion_report": "not_evaluated; raw cold reinspection required",
            "executable_policy": "launcher_frozen_binary; native_metadata_does_not_hash_executable",
            "executable_upgrade_supported": False}
    if liveness is not None:
        plan["ram_guard_liveness"] = dict(liveness, limit=args.max_zero_progress_ram_stops)
    plan["memory_admission_preview"] = memory_admission_preview(options)
    if phase_policy is not None:
        plan["master_reduction"] = phase_policy
    preview = plan["memory_admission_preview"]
    if not args.json and preview.get("hard_capped_by_available_memory"):
        print(f"RAM admission now: effective hard cap {preview['effective_hard_memory_bytes']} B = MemAvailable "
              f"{preview['available_bytes']} B - floor {preview['host_memory_reserve_bytes']} B, below the requested "
              f"{preview['requested_hard_memory_bytes']} B (re-evaluated by the supervisor at launch)",
              file=sys.stderr)
    if not args.start:
        if upgrade is not None and not args.json:
            new = upgrade["new"]
            rollback = new["probe"] is None
            print(f"Executable {'rollback' if rollback else 'upgrade'} dry run; nothing was changed.")
            print(f"  frozen: sha256 {upgrade['frozen']['sha256']}  {upgrade['frozen']['path']}")
            print(f"  new:    sha256 {new['sha256']}  {new['source']}")
            print(f"  walk semantics: checkpoint {upgrade['checkpoint']['walk_semantics_version']} "
                  f"(generation {upgrade['checkpoint']['generation']}, {upgrade['checkpoint']['kind']}), "
                  + (f"new executable {upgrade['walk_semantics_version']} from bin/executable.json history "
                     "(rollback; no probe needed)" if rollback else
                     f"new executable {new['probe']['walk_semantics_version']}"))
            for line in upgrade["live_run_evidence"]:
                print(f"  refusal with --start while alive: {line}")
            print("  apply by rerunning with --start; it will launch:")
        if phase_policy is not None and not args.json:
            invocation = list(sys.argv[1:] if argv is None else argv)
            print(shlex.join([sys.executable, str(Path(__file__).resolve()), *invocation, "--start"]))
        else:
            print(json.dumps(plan, indent=2) if args.json else shlex.join(command))
        return 0
    if upgrade is not None and not args.json:
        action = "rolled back" if upgrade["reason"] == ROLLBACK_REASON else "upgraded"
        print(f"Executable {action} to sha256 {executable_hash} (walk semantics "
              f"{upgrade['walk_semantics_version']}); resuming.", flush=True)
    checkpoint.parent.mkdir(parents=True, exist_ok=True)
    sync_directory(campaign)
    if phase_policy is not None:
        try:
            # This module is often loaded through importlib by tests/steering;
            # no dependency on a particular sys.modules registration is needed.
            from types import SimpleNamespace
            driver = SimpleNamespace(**globals())
            return phases.run(plan, phase_policy, args.resume, driver,
                              postprocess_only=args.publish_only or args.refine_masters)
        except (OSError, ValueError, TypeError) as error:
            parser.error(str(error))
    write_json(campaign / "active-run.json", plan)
    os.execv(command[0], command)


if __name__ == "__main__":
    raise SystemExit(main())
