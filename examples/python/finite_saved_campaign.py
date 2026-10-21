"""Finite-demand campaign steering. Native Rust owns all geometry and closure.

This is a separate checkpoint engine, never a CP6 importer. Publication and
explicit refinement use the existing campaign_phases tail and native commands.
"""
from __future__ import annotations

from contextlib import contextmanager, nullcontext
import argparse
from datetime import datetime, timezone
import fcntl
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import shlex
import stat
import subprocess
import sys
import tempfile


def sibling(name):
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    value = importlib.util.module_from_spec(spec)
    sys.modules[name] = value
    spec.loader.exec_module(value)
    return value


PRODUCTION = sibling("production_saved_owner_campaign")
STARTUP = sibling("finite_startup_progress")
PHASES = PRODUCTION.PHASES
SUPERVISOR = PRODUCTION.SUPERVISOR
ENGINE = "rustred.finite-demand.v1"
STEERING = "rustred.finite-campaign-steering.v1"
write_json = PRODUCTION.write_json
sync_directory = PRODUCTION.sync_directory
campaign_runs = PRODUCTION.campaign_runs
campaign_run_liveness = PRODUCTION.campaign_run_liveness
steering_executable = PRODUCTION.steering_executable


def read(path, max_bytes=None):
    """Strict JSON transport; native caller policy owns resource allowances.

    An optional byte allowance is useful to callers reading untrusted metadata.
    Do not silently impose a smaller steering-only ceiling than the native one.
    """
    if max_bytes is not None and (type(max_bytes) is not int or max_bytes < 0):
        raise ValueError("metadata byte allowance must be nonnegative or None")
    with Path(path).open("rb") as stream:
        raw = stream.read() if max_bytes is None else stream.read(max_bytes + 1)
    if max_bytes is not None and len(raw) > max_bytes:
        raise ValueError(f"finite steering document exceeds requested {max_bytes}-byte allowance: {path}")
    return SUPERVISOR.ROLES.loads_document(raw)


def encoded(value):
    return (json.dumps(value, indent=2, allow_nan=False) + "\n").encode()


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def engine(campaign, requested=None):
    """Fresh omission selects legacy; saved finite markers retain their engine.

    Must run before any legacy reader/lock/preflight (including freshness).
    Keep the caller's omitted value distinct from an explicit legacy request.
    """
    path = Path(campaign) / "bin/steering.json"
    saved = read(path) if path.is_file() else None
    if saved is not None and not isinstance(saved, dict):
        raise ValueError("saved campaign steering must be an object")
    marker = saved.get("engine") if isinstance(saved, dict) else None
    if marker not in (None, ENGINE):
        raise ValueError("unsupported saved campaign engine marker")
    if requested == "finite-demand" and saved is not None and marker != ENGINE:
        raise ValueError("finite-demand requires a fresh campaign, not a legacy steering file")
    if requested == "legacy" and marker == ENGINE:
        raise ValueError("cannot use the legacy engine for a finite-demand campaign")
    if marker == ENGINE or requested == "finite-demand":
        if (Path(campaign) / "checkpoints/main").exists():
            raise ValueError("finite-demand does not import an existing CP6 checkpoint")
        return ENGINE
    if (Path(campaign) / "finite-demand").exists():
        raise ValueError("finite namespace exists without its engine steering; refuse legacy fallback")
    return None


def require_policy(value):
    fields = {"workers", "windows", "census", "export", "storage", "max_total_rows", "max_source_bytes", "control"}
    optional = {"per_batch_work", "prepared_reduction_aggregates", "operation_limits", "max_resident_work_bytes", "max_windows_per_batch", "reuse_worker_pool", "parallel_window_preparation"}
    if not isinstance(value, dict) or not fields <= set(value) or set(value) - fields - optional:
        raise ValueError("finite policy must contain the public native policy fields and no diagnostic settings")
    if type(value["workers"]) is not int or value["workers"] <= 0 or not isinstance(value["windows"], dict):
        raise ValueError("finite policy requires positive workers and explicit window limits")
    if type(value.get("max_windows_per_batch", 1)) is not int or value.get("max_windows_per_batch", 1) <= 0:
        raise ValueError("finite max_windows_per_batch must be positive")
    if type(value.get("reuse_worker_pool", True)) is not bool:
        raise ValueError("finite reuse_worker_pool must be boolean")
    if type(value.get("parallel_window_preparation", False)) is not bool:
        raise ValueError("finite parallel_window_preparation must be boolean")
    if value.get("parallel_window_preparation", False) and not value.get("reuse_worker_pool", True):
        raise ValueError("parallel window preparation requires the persistent worker pool")
    resident = value.get("max_resident_work_bytes", 0)
    if type(resident) is not int or not 0 <= resident < 2**64:
        raise ValueError("resident Work allowance must be an unsigned platform byte count")
    if "operation_limits" in value:
        require_operation_limits(value["operation_limits"])
    return value


def require_operation_limits(value):
    """JSON shape/type validation only; native Rust owns limits and semantics.

    Missing/object-empty policy means uncapped trusted finite operations.
    Top-level null deliberately keeps source grants. Individual null counts
    disable that stop; zero remains an explicit zero allowance.
    """
    if value is None:
        return
    exact = {"max_polynomial_terms": None, "max_term_operations": None}
    schema = {
        "planning": {"max_positive_layers": None, "max_partition_checks": None},
        "transport": {
            **{name: None for name in ("max_factors", "max_relation_coefficient_entries", "max_total_power",
                "max_native_polynomial_terms", "max_native_polynomial_operations", "max_native_exponent_entries",
                "max_endpoints", "max_endpoint_power_entries", "max_retained_endpoint_key_bytes",
                "max_retained_coefficient_terms", "max_retained_coefficient_clone_owned_bytes")},
            "exact_algebra": exact,
        },
        "reduction": {
            "exact_algebra": exact,
            "indexed_algebra": {"exact_algebra": exact,
                "max_specialization_power_operations": None, "max_specialization_integer_bits": None},
            **{name: None for name in ("max_cached_integrals", "max_cached_coefficient_terms",
                "max_cached_coefficient_bytes", "max_factorization_terms")},
        },
    }

    def check(node, fields, path):
        if not isinstance(node, dict) or set(node) - fields.keys():
            raise ValueError(f"{path} must be an object containing only native operation-limit fields")
        for name, item in node.items():
            child = fields[name]
            if isinstance(child, dict):
                check(item, child, f"{path}.{name}")
            elif item is not None and (type(item) is not int or not 0 <= item < 2**64):
                raise ValueError(f"{path}.{name} must be an unsigned count or null (uncapped)")
    check(value, schema, "operation_limits")


def reject_auxiliary(campaign):
    """Publication currently has no auxiliary-root admission; fail before work."""
    paths = [Path(campaign) / "inputs/queries.json", *sorted((Path(campaign) / "amendments").glob("amendment-*.json"))]
    for path in paths:
        document = read(path)
        roles = SUPERVISOR.ROLES.query_roles(document, require_explicit=True)
        if any(role == "auxiliary" for role in roles.values()):
            raise ValueError("finite campaign auto-publication does not support auxiliary query rows; no rows were dropped")


@contextmanager
def control_lock(namespace):
    namespace = Path(namespace)
    if not namespace.exists():
        yield  # Native create owns the absent namespace; dispatcher is held.
        return
    path = namespace / "control.lock"
    descriptor = os.open(path, os.O_RDWR | os.O_NOFOLLOW)
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise ValueError("finite campaign already has an active native control lease") from None
        yield
    finally:
        os.close(descriptor)


def selected_policy_path(campaign, supplied=None):
    if supplied is not None:
        return Path(supplied).resolve()
    steering = Path(campaign) / "bin/steering.json"
    if steering.is_file():
        saved = read(steering)
        if saved.get("policy"):
            return Path(campaign) / saved["policy"]
    return None


def resolve_metadata_budget(policy, resources):
    """Resolve the automatic decoder allowance from the user's RAM budget.

    It is an allocation permission, not memory reserved by the Work cache.
    Never mutate supplied or previously frozen policy documents.
    """
    storage = policy.get("storage", {})
    if "max_transient_bytes" not in storage or storage["max_transient_bytes"] is not None:
        return policy
    ceiling = resources.get("max_memory_bytes")
    margin = resources.get("ram_guard_margin_percent", 5)
    if (type(ceiling) is not int or not 0 < ceiling < 2**64
            or type(margin) not in (int, float) or not math.isfinite(margin) or not 0 < margin < 100):
        raise ValueError("automatic metadata budget requires a positive saved/requested RAM ceiling and guard margin")
    budget = int(ceiling * (1 - margin / 100))
    if budget <= 0:
        raise ValueError("RAM allowance leaves no metadata decoding budget")
    return {**policy, "storage": {**storage, "max_transient_bytes": budget}}


@contextmanager
def inspection_policy(campaign, supplied=None):
    path = selected_policy_path(campaign, supplied)
    if path is None:
        yield None
        return
    policy = require_policy(read(path))
    if policy.get("storage", {}).get("max_transient_bytes", 0) is not None:
        yield path
        return
    saved = read(Path(campaign) / "bin/steering.json")
    resolved = resolve_metadata_budget(policy, saved.get("resources", {}))
    scratch = Path(__file__).resolve().parents[2] / "TMP"
    scratch.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="finite-inspection-policy-", dir=scratch) as directory:
        temporary = Path(directory) / "policy.json"
        write_json(temporary, resolved)
        yield temporary


def inspect(campaign, executable, output_format="json", policy_path=None):
    command = [str(executable), "finite-demand", "inspect", "--directory", str(Path(campaign) / "finite-demand"),
               "--format", output_format]
    with inspection_policy(campaign, policy_path) as selected:
        if selected is not None:
            command += ["--policy", str(selected)]
        result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise ValueError(f"native finite inspect failed (exit{result.returncode}): {result.stderr[-8192:]}")
    if output_format != "json":
        return result.stdout
    value = SUPERVISOR.ROLES.loads_document(result.stdout)
    if value.get("schema") != "rustred.finite-demand-status.v1" or value.get("engine") != ENGINE:
        raise ValueError("native finite inspect returned a different engine/schema")
    return value


def public_inspect(campaign, executable, output_format, policy_path=None):
    with inspection_policy(campaign, policy_path) as selected:
        return _public_inspect(campaign, executable, output_format, selected)


def _public_inspect(campaign, executable, output_format, policy_path=None):
    """Compose native committed status with native published/requested scope.

    Internal scope planning continues to use inspect() alone. No artifact means
    pending publication, never a fallback to the legacy CP6 inventory reader.
    """
    policy_path = selected_policy_path(campaign, policy_path)
    native = inspect(campaign, executable, policy_path=policy_path)
    pointer = campaign / "artifacts/latest.json"
    publication = None
    present = False
    try:
        metadata = pointer.lstat()
    except FileNotFoundError:
        pass
    else:
        if not stat.S_ISREG(metadata.st_mode):
            raise ValueError("finite publication pointer must be a regular file")
        parent = pointer.parent.lstat()
        if not stat.S_ISDIR(parent.st_mode):
            raise ValueError("finite publication directory must be a real directory")
        # Existing portable pointer validator; the native inspector remains the
        # source for rule/master counts and current-versus-published scope.
        if PHASES.completed_artifact(campaign) is None:
            raise ValueError("finite publication pointer has no completed artifact")
        present = True
    table = output_format == "table" or output_format == "auto" and sys.stdout.isatty()
    artifact_command = [str(executable), "artifact-inspect", "--campaign-directory", str(campaign)]
    policy_args = ["--policy", str(policy_path)] if policy_path is not None else []
    if policy_path is not None:
        artifact_command += ["--finite-policy", str(policy_path)]
    if table:
        # Do not capture/re-render native tables: retain their TTY/color behavior.
        for command in ([str(executable), "finite-demand", "inspect", "--directory",
                         str(campaign / "finite-demand"), "--format", "table", *policy_args],
                        artifact_command + ["--format", "table"] if present else None):
            if command is None:
                print("Publication: pending; no published artifact is available.", flush=True)
                continue
            result = subprocess.run(command)
            if result.returncode:
                raise ValueError(f"native finite/publication inspect failed (exit{result.returncode})")
        return
    if present:
        result = subprocess.run(artifact_command + ["--format", "json"], capture_output=True, text=True)
        if result.returncode:
            raise ValueError(f"native publication inspect failed (exit{result.returncode}): {result.stderr[-8192:]}")
        publication = SUPERVISOR.ROLES.loads_document(result.stdout)
        if not isinstance(publication, dict):
            raise ValueError("native publication inspection must be an object")
    print(json.dumps({**native, "publication": publication,
        "publication_status": "available" if present else "pending"}, indent=2))


def finite_scope_binding(plan):
    campaign = Path(plan["campaign_directory"])
    if not Path(plan["checkpoint_directory"]).exists():
        return None
    native = inspect(campaign, plan["native_executable"], policy_path=plan.get("finite_policy"))
    require_prepared_namespace(native)
    if native["source"]["status"] in ("missing", "stale") and plan.get("allow_unbound_resume"):
        return None  # Explicit source resupply; never infer absence/freshness.
    if native["source"]["status"] != "bound":
        raise ValueError("native finite source is missing/stale; resupply its unchanged source for native repair before planning")
    validate_committed_sources(campaign, native)
    components = {"engine": ENGINE, "generation_digest": native["generation"]["digest"],
        "amendments": PHASES._chain(campaign),
        "selection_sha256": PHASES.digest(campaign / "inputs/selection.json"),
        "queries_sha256": PHASES.digest(campaign / "inputs/queries.json")}
    return {"key": digest(json.dumps(components, sort_keys=True, separators=(",", ":")).encode()),
            "components": components, "generation": native["generation"]["number"], "native": native}


def require_prepared_namespace(native):
    if native.get("status") == "preparing" or native.get("source", {}).get("status") == "unadmitted":
        raise ValueError("initial finite preparation is incomplete; explicitly use saved_campaign.py run "
                         "--repair-preparation. Native permits this only before any store/checkpoint exists")


def validate_committed_sources(campaign, native):
    """Join requested files to native-inspected committed JSON snapshots exactly.

    Native inspect authenticates these snapshots. This byte comparison adds no
    hash or binary parser and cannot promote a same-count modified amendment.
    """
    source = native["source"]
    records, files = source["amendments"], source["files"]
    amendments = sorted((campaign / "amendments").glob("amendment-*.json"))
    if len(files) != 2 + len(records) or len(amendments) not in (len(records), len(records) + 1):
        raise ValueError("committed source inventory and pending amendment suffix differ")
    expected = [("selection", campaign / "inputs/selection.json"), ("queries", campaign / "inputs/queries.json")]
    expected += [("amendment", path) for path in amendments[:len(records)]]
    namespace = campaign / "finite-demand"
    for (kind, original), item in zip(expected, files, strict=True):
        relative = Path(item["path"])
        if relative.parent != Path("sources") or relative.name != f"{kind}-{item['blake3']}.json":
            raise ValueError("native source snapshot has an unexpected JSON path")
        captured = namespace / relative
        if captured.is_symlink() or not captured.resolve().is_relative_to(namespace.resolve()):
            raise ValueError("native source snapshot escaped its namespace")
        if captured.stat().st_size != item["bytes"] or original.stat().st_size != item["bytes"]:
            raise ValueError("requested source bytes differ from native committed snapshot")
        with captured.open("rb") as left, original.open("rb") as right:
            while block := left.read(1024 * 1024):
                if block != right.read(len(block)):
                    raise ValueError("requested source bytes differ from native committed snapshot")
            if right.read(1):
                raise ValueError("requested source bytes differ from native committed snapshot")


def finite_completed_request(plan, binding):
    native = binding["native"]
    committed = native["committed"]
    if not committed["required_complete"] or not committed["all_rows_exhausted"]:
        return None
    # An immutable pending amendment changes the requested scope, not CURRENT.
    if len(native["source"]["amendments"]) != len(binding["components"]["amendments"]):
        return None
    return Path(plan["source_command"])


def source_command(campaign, count, size, amendments):
    # Relative inputs resolve at the immutable bin/argv JSON's parent. The old
    # checkpoint/output are inert parser carriers; never create or execute them.
    command = ["rustred", "owner-domain-match", "--manifest", "../inputs/selection.json",
        "--queries", "../inputs/queries.json", "--owner-base", "../inputs",
        "--output", "../finite-source-unused/output.json", "--follow-successors",
        "--max-queries", str(count), "--max-query-bytes", str(size),
        "--publication-policy", "epoch", "--transfer-unreserved-lookahead", "1",
        "--route-domain-overcover", "--no-progress",
        "--resume" if amendments else "--checkpoint", "../finite-source-unused/checkpoint"]
    for path in amendments:
        command += ["--amend-queries", "../amendments/" + path.name]
    return {"command": command}


def extension(campaign, executable, rank, difference, dry_run, dispatcher_held=False, policy_path=None):
    helper = sibling("extend_rank_campaign")
    dispatcher = (nullcontext() if dispatcher_held else PRODUCTION.existing_phase_lock(campaign)
                  if dry_run else PHASES.dispatcher_lock(campaign / "master-reduction"))
    with dispatcher, control_lock(campaign / "finite-demand"):
        native = inspect(campaign, executable, policy_path=policy_path)
        require_prepared_namespace(native)
        if native["source"]["status"] != "bound":
            raise ValueError("rank-stage planning requires bound native source metadata")
        validate_committed_sources(campaign, native)
        binding = finite_scope_binding({"campaign_directory": str(campaign), "checkpoint_directory": str(campaign / "finite-demand"),
                                       "native_executable": str(executable),
                                       "finite_policy": str(policy_path) if policy_path is not None else None})
        PHASES.require_retained_refinement(campaign, binding)
        path, raw, receipt = helper.plan_from_source(campaign, rank, native["source"],
            {"generation": native["generation"]["number"], "generation_digest": native["generation"]["digest"]},
            max_power_difference=difference, finite=True)
        if not dry_run:
            if path.parent.resolve().parent != campaign or path.parent.is_symlink():
                raise ValueError("amendments directory must remain inside this campaign")
            path.parent.mkdir(exist_ok=True)
            helper.write_once(path, raw)
        return {**receipt, "status": "preview-only" if dry_run else "amendment-prepared-no-native-work"}


def immutable_json(campaign, prefix, value, write):
    raw = encoded(value)
    path = campaign / "bin" / f"{prefix}-{digest(raw)}.json"
    if write:
        path.parent.mkdir(parents=True, exist_ok=True)
        sibling("extend_rank_campaign").write_once(path, raw)
    return path


def executable(campaign, supplied):
    if supplied is not None:
        result = Path(supplied).resolve()
    else:
        receipt = read(campaign / "bin/executable.json")
        result = campaign / "bin" / receipt["file"]
        if PHASES.digest(result) != receipt["sha256"]:
            raise ValueError("frozen finite executable digest changed")
    if not result.is_file() or not os.access(result, os.X_OK):
        raise ValueError("finite campaign requires an executable native binary")
    return result


def launch_plan(args, campaign, binary, *, apply, startup_progress=None):
    saved_path = campaign / "bin/steering.json"
    saved = read(saved_path) if saved_path.is_file() else None
    if startup_progress is not None:
        startup_progress.phase("verifying campaign inputs")
    count, size, receipt = PRODUCTION.verify_inputs(campaign / "inputs")
    reject_auxiliary(campaign)
    if saved is not None and (saved.get("schema") != STEERING or saved.get("engine") != ENGINE):
        raise ValueError("invalid finite steering identity")
    policy_path = getattr(args, "finite_policy", None)
    if policy_path is None:
        if saved is None:
            raise ValueError("first finite run requires --finite-policy with explicit native grants")
        policy_path = campaign / saved["policy"]
    policy = require_policy(read(policy_path))
    # Make the native trusted-finite default explicit in each new immutable
    # launch policy, including resuming older policies which omitted this field.
    # Never mutate the caller's input file or an earlier frozen policy.
    policy = {"operation_limits": {}, **policy}
    workers = getattr(args, "workers", None)
    if workers is not None:
        policy = require_policy({**policy, "workers": workers})
    grouping = getattr(args, "windows_per_batch", None)
    if grouping is not None:
        policy = require_policy({**policy, "max_windows_per_batch": grouping})
    pool = getattr(args, "worker_pool", None)
    if pool is not None:
        if pool not in ("persistent", "per-group"):
            raise ValueError("finite worker pool must be persistent or per-group")
        policy = {**policy, "reuse_worker_pool": pool == "persistent"}
    preparation = getattr(args, "window_preparation", None)
    if preparation is not None:
        if preparation not in ("serial", "parallel"):
            raise ValueError("finite window preparation must be serial or parallel")
        policy = {**policy, "parallel_window_preparation": preparation == "parallel"}
    policy = require_policy(policy)
    if saved is not None and policy["windows"] != saved["window_policy"]:
        raise ValueError("finite window/partition policy cannot change on resume")
    resources = dict((saved or {}).get("resources", {}))
    resources["memory_policy"] = (getattr(args, "memory_policy", None)
                                  or resources.get("memory_policy", "process-limit"))
    resources.setdefault("host_memory_reserve_bytes", 0)
    for key in ("cpus", "max_memory_bytes", "host_memory_reserve_bytes", "ram_guard_margin_percent"):
        value = getattr(args, key, None)
        if value is not None:
            resources[key] = value
    if any(key not in resources for key in ("cpus", "max_memory_bytes")):
        raise ValueError("first finite run requires --cpus and --max-memory-bytes")
    resources.setdefault("ram_guard_margin_percent", 5)
    if (type(resources["max_memory_bytes"]) is not int or resources["max_memory_bytes"] <= 0
            or type(resources["host_memory_reserve_bytes"]) is not int or resources["host_memory_reserve_bytes"] < 0):
        raise ValueError("finite RSS ceiling must be positive and host reserve nonnegative")
    margin = resources["ram_guard_margin_percent"]
    if type(margin) not in (int, float) or not math.isfinite(margin) or not 0 < margin < 100:
        raise ValueError("finite RAM margin must be finite and strictly between0 and100")
    resources.update(workers=policy["workers"], swap_growth_stop_bytes_per_second=0)
    cpus = SUPERVISOR.parse_cpu_set(resources["cpus"])
    if len(cpus) != policy["workers"] or not cpus <= os.sched_getaffinity(0):
        raise ValueError("finite workers must equal the reserved permitted CPU count")
    SUPERVISOR.memory_admission_for_resources(resources, SUPERVISOR.memory_host_snapshot(resources))
    policy = resolve_metadata_budget(policy, resources)
    grace = getattr(args, "stop_grace_seconds", None)
    if grace is None:
        grace = (saved or {}).get("stop_grace_seconds", 120)
    if type(grace) not in (int, float) or not math.isfinite(grace) or grace <= 0:
        raise ValueError("stop grace must be finite and positive")
    namespace = campaign / "finite-demand"
    if getattr(args, "resume", False) and not namespace.exists():
        raise ValueError("explicit finite resume requires an existing native namespace")
    repair = getattr(args, "repair_preparation", False)
    if repair and not namespace.exists():
        raise ValueError("preparation repair requires an existing interrupted finite namespace")
    if startup_progress is not None:
        startup_progress.phase("freezing solve executable" if apply else "checking solve executable")
    if apply:
        (campaign / "bin").mkdir(parents=True, exist_ok=True)
        binary, binary_sha = PRODUCTION.copy_executable(campaign / "bin", binary)
        PRODUCTION.write_json(campaign / "bin/executable.json", {"file": binary.name, "sha256": binary_sha})
    else:
        binary_sha = PHASES.digest(binary)
    if startup_progress is not None:
        startup_progress.phase("saving launch plan")
    amendments = sorted((campaign / "amendments").glob("amendment-*.json"))
    carrier = source_command(campaign, count, size, amendments)
    source_path = immutable_json(campaign, "finite-source", carrier, apply)
    frozen_policy = immutable_json(campaign, "finite-policy", policy, apply)
    steering = {"schema": STEERING, "engine": ENGINE,
        "command_arguments": ["--executable", str(binary)], "window_policy": policy["windows"],
        "policy": str(frozen_policy.relative_to(campaign)), "resources": resources, "stop_grace_seconds": grace}
    if apply:
        write_json(saved_path, steering)
    run = campaign / "runs" / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    native = [str(binary), "finite-demand", "run", "--directory", str(namespace),
        "--source-command", str(source_path), "--policy", str(frozen_policy),
        "--output", str(run / "native-result.json"), "--events", str(run / "events.jsonl"),
        "--stop-file", str(run / "stop-request.json")]
    if namespace.exists():
        native.append("--resume")
    if repair:
        native.append("--repair-preparation")
    plan = {"engine": ENGINE, "campaign_directory": str(campaign), "checkpoint_directory": str(namespace),
        "run_directory": str(run), "native_executable": str(binary), "executable_sha256": binary_sha,
        "source_command": str(source_path), "finite_policy": str(frozen_policy),
        "native_command": native, "requested_workers": policy["workers"], "cpus": resources["cpus"],
        "supervisor_ram_policy": resources, "checkpoint_interval_seconds": 300,
        "steering_policy": steering, "stop_grace_seconds": grace,
        "allow_unbound_resume": args.action == "run",
        "repair_preparation": repair,
        "selection_sha256": receipt["selection_sha256"], "queries_sha256": receipt["queries_sha256"]}
    plan["command"] = [sys.executable, "-B", str(Path(__file__).resolve()), "--execute", str(run / "launch.json")]
    if apply:
        run.mkdir(parents=True, exist_ok=False)
        write_json(run / "launch.json", plan)
    return plan


def execute(path):
    with STARTUP.StartupProgress() as progress:
        progress.phase("checking native launch executable")
        plan = read(path)
        if plan.get("engine") != ENGINE or PHASES.digest(plan["native_executable"]) != plan["executable_sha256"]:
            raise ValueError("finite launch executable/engine binding differs")
        progress.phase("preparing native process guard")
        guard_path = Path(__file__).resolve().parents[2] / "tools/research/runtime_order/generation_guard.py"
        spec = importlib.util.spec_from_file_location("finite_campaign_guard", guard_path)
        guard = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(guard)
        for name in SUPERVISOR.DIAGNOSTIC_ONLY_ENVIRONMENT:
            os.environ.pop(name, None)
        run = Path(plan["run_directory"])
    # The guard's preexec_fn must run without a live reporter thread.
    try:
        guard.run(plan["native_command"], run / "guard", plan["supervisor_ram_policy"],
            grace_seconds=plan["stop_grace_seconds"], cooperative_stop_file=run / "stop-request.json",
            native_tty_progress=True)
    except RuntimeError:
        pass
    receipt = read(run / "guard/result.json")
    if not receipt.get("owned_group_drained") or receipt.get("stop_reason"):
        return 8
    code = receipt.get("exit_code")
    if code != 0:
        return code if type(code) is int and code > 0 else 1
    result = read(run / "native-result.json")
    if result.get("schema") != "rustred.finite-demand-run.v1" or result.get("engine") != ENGINE or result.get("complete") is not True:
        raise ValueError("native finite exit0 lacks a complete typed result")
    return 0


def public_main(args, extra):
    parser = argparse.ArgumentParser(add_help=False, allow_abbrev=False)
    parser.add_argument("--finite-policy", type=Path)
    parser.add_argument("--workers", type=int)
    parser.add_argument("--windows-per-batch", type=int)
    parser.add_argument("--worker-pool", choices=("persistent", "per-group"))
    parser.add_argument("--window-preparation", choices=("serial", "parallel"))
    parser.add_argument("--cpus")
    parser.add_argument("--max-memory-bytes", type=int)
    parser.add_argument("--host-memory-reserve-bytes", type=int)
    parser.add_argument("--memory-policy", choices=("process-limit", "host-aware"))
    parser.add_argument("--ram-guard-margin-percent", type=float)
    parser.add_argument("--stop-grace-seconds", type=float)
    parser.add_argument("--repair-preparation", action="store_true")
    finite_args = parser.parse_args(extra)
    if finite_args.workers is not None and finite_args.workers <= 0:
        raise ValueError("finite workers must be positive")
    if finite_args.windows_per_batch is not None and finite_args.windows_per_batch <= 0:
        raise ValueError("finite windows per batch must be positive")
    if args.action == "inspect" and any(value is not None and value is not False
                                        for name, value in vars(finite_args).items() if name != "finite_policy"):
        raise ValueError("finite inspect accepts only --finite-policy, not launch-policy overrides")
    for name, value in vars(finite_args).items():
        setattr(args, name, value)
    if args.repair_preparation and args.action != "run":
        raise ValueError("--repair-preparation is only an explicit finite run recovery operation")
    campaign = args.campaign.resolve()
    assert engine(campaign, getattr(args, "engine", None)) == ENGINE
    for name in ("bin", "runs", "master-reduction", "amendments", "finite-demand", "artifacts"):
        if (campaign / name).is_symlink():
            raise ValueError("finite campaign output directories must not be symlinks: " + name)
    binary = executable(campaign, args.executable)
    if args.action == "inspect":
        public_inspect(campaign, binary, args.format, policy_path=args.finite_policy)
        return 0
    if getattr(args, "deep_verification", False):
        raise ValueError("finite-demand publication has trusted generated provenance, not independent graph replay")
    if args.dry_run:
        with PRODUCTION.existing_phase_lock(campaign):
            return public_locked(args, campaign, binary, apply=False)
    # Keep the dispatcher over source planning, launch and publication. Native
    # work acquires control only after the planning lock has been dropped.
    with PHASES.dispatcher_lock(campaign / "master-reduction"):
        return public_locked(args, campaign, binary, apply=True)


def public_locked(args, campaign, binary, *, apply):
    with STARTUP.StartupProgress(enabled=apply and not args.dry_run) as progress:
        return _public_locked(args, campaign, binary, apply=apply, startup_progress=progress)


def _public_locked(args, campaign, binary, *, apply, startup_progress):
    if args.action == "extend":
        helper = sibling("extend_rank_campaign")
        difference = (None if args.unbounded_power_difference else args.max_power_difference
                      if args.max_power_difference is not None else helper.PRESERVE_DIFFERENCE)
        receipt = extension(campaign, binary, args.rank, difference, args.dry_run, dispatcher_held=True,
                            policy_path=getattr(args, "finite_policy", None))
        print(json.dumps(receipt, indent=2), flush=True)
        if args.dry_run:
            return 0
    startup_progress.phase("checking publication configuration")
    policy = PHASES.configuration(campaign, enabled=args.action == "refine", executable=binary,
        seed_depth=getattr(args, "seed_depth", None), saved_rule_assistance=getattr(args, "saved_rule_assistance", None),
        containing_sector_depth=getattr(args, "containing_sector_depth", None),
        circuit_symmetry_assistance=getattr(args, "circuit_symmetry_assistance", None),
        normalization_profile=getattr(args, "normalization_profile", None),
        collection_artifacts=getattr(args, "collection_artifact", None),
        finite_feedback=getattr(args, "finite_feedback", None),
        deep_verification=False if args.action in ("run", "extend", "publish") else None,
        max_relation_columns=getattr(args, "max_relation_columns", None),
        refinement_strategy=getattr(args, "refinement_strategy", None),
        modular_policy=getattr(args, "modular_policy", None),
        relation_batch_rows=getattr(args, "relation_batch_rows", None),
        relation_block_rows=getattr(args, "relation_block_rows", None),
        refinement_workers=getattr(args, "refinement_workers", None),
        refinement_cpus=getattr(args, "refinement_cpus", None))
    if args.action == "finalize-refinement":
        policy["operation"] = "finalize-refinement"
    startup_progress.phase("acquiring finite control lock")
    with control_lock(campaign / "finite-demand"):
        if (campaign / "finite-demand").exists() and not args.repair_preparation:
            startup_progress.phase("inspecting finite checkpoint")
            status = inspect(campaign, binary, policy_path=getattr(args, "finite_policy", None))
            require_prepared_namespace(status)
            if status["source"]["status"] != "bound" and args.action != "run":
                raise ValueError("native source metadata is not bound; run with unchanged source to repair it first")
            if status["source"]["status"] == "bound":
                startup_progress.phase("validating committed input sources")
                validate_committed_sources(campaign, status)
        plan = launch_plan(args, campaign, binary, apply=apply, startup_progress=startup_progress)
    if args.dry_run:
        print("No native work launched. Native source command: " + shlex.join(plan["native_command"]))
        return 0
    return PHASES.run(plan, policy, Path(plan["checkpoint_directory"]).exists(), sys.modules[__name__],
                      postprocess_only=args.action in ("publish", "refine", "finalize-refinement"),
                      dispatcher_held=True, startup_progress=startup_progress)


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] != "--execute":
        raise SystemExit("use saved_campaign.py for public finite campaign steering")
    raise SystemExit(execute(sys.argv[2]))
