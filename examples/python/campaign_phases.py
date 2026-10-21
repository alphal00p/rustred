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
import stat
import shutil
import subprocess
import sys
import tempfile
import time

SCHEMA = "rustred.campaign-phases.v1"
NORMALIZATION_PROFILES = {"conservative": "conservative-v1", "standard": "standard-v1"}
COLLECTION_STRATEGY = "full-u-diagonal-v1"
FINITE_FEEDBACK_RECIPE = "finite-row-feedback-v1"
PUBLICATION_VERIFICATION_RECIPE = "publication-verification-v1"
REFINEMENT_STRATEGIES = ("exact", "modular-circuits")
MODULAR_SEARCH_RECIPE = "terminal-modular-circuits-v1"
FRESH_SEARCH_RECIPE = "relations-only-v1"
NUMERATOR_SYMMETRY_RECIPE = "verified-denominator-preserving-numerator-maps-v1"


def read_json(path, max_bytes=None):
    if max_bytes is not None and (type(max_bytes) is not int or max_bytes < 0):
        raise ValueError("metadata byte allowance must be nonnegative or None")
    with Path(path).open("rb") as stream:
        raw = stream.read() if max_bytes is None else stream.read(max_bytes + 1)
    if max_bytes is not None and len(raw) > max_bytes:
        raise ValueError(f"campaign metadata exceeds requested {max_bytes}-byte allowance: {path}")
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
                  collection_artifacts=None, finite_feedback=None, deep_verification=None,
                  max_relation_columns=None, relation_batch_rows=None, relation_block_rows=None,
                  refinement_workers=None, refinement_cpus=None, refinement_strategy=None,
                  modular_policy=None, fresh_search=False, numerator_symmetry_assistance=None):
    """Read-only configuration. Only this invocation can request refinement.

    Historical ``enabled`` policies retain executable/search preferences, never
    authority to refine an ordinary solve or extension automatically.
    """
    path = Path(campaign) / "master-reduction" / "policy.json"
    existing = read_json(path) if path.is_file() else None
    if type(fresh_search) is not bool or (fresh_search and not enabled):
        raise ValueError("fresh search requires explicit refinement and a boolean request")
    # A new round retains operational grants, not the preceding search recipe.
    # This request is deliberately never saved as authority to reset again.
    preferences = ({key: value for key, value in (existing or {}).items() if key in (
        "executable", "max_relation_columns", "refinement_workers", "refinement_cpus",
        "relation_batch_rows", "relation_block_rows", "deep_verification")}
        if fresh_search else (existing or {}))
    if refinement_strategy is not None and (refinement_strategy not in REFINEMENT_STRATEGIES
                                             or not enabled):
        raise ValueError("master refinement strategy requires explicit refinement and exact or modular-circuits")
    strategy = refinement_strategy or preferences.get("refinement_strategy", "exact")
    if strategy not in REFINEMENT_STRATEGIES:
        raise ValueError("invalid persisted master refinement strategy")
    if modular_policy is not None and (not enabled or strategy != "modular-circuits"):
        raise ValueError("modular policy requires explicit modular-circuits refinement")
    modular = (read_json(modular_policy) if modular_policy is not None
               else preferences.get("modular_policy"))
    if modular is not None and not isinstance(modular, dict):
        raise ValueError("invalid persisted modular refinement policy")
    if max_relation_columns is not None and (type(max_relation_columns) is not int or max_relation_columns < 0):
        raise ValueError("master max relation columns must be nonnegative (0 means unlimited)")
    if max_relation_columns is not None and not enabled:
        raise ValueError("master max relation columns requires explicit --refine-masters")
    if normalization_profile is not None and (not isinstance(normalization_profile, str)
                                               or normalization_profile not in NORMALIZATION_PROFILES):
        raise ValueError("master normalization profile must be conservative or standard")
    if seed_depth is not None and (type(seed_depth) is not int or seed_depth < 0):
        raise ValueError("master seed depth must be nonnegative")
    if saved_rule_assistance is not None and type(saved_rule_assistance) is not bool:
        raise ValueError("master saved-rule assistance must be boolean")
    if circuit_symmetry_assistance is not None and type(circuit_symmetry_assistance) is not bool:
        raise ValueError("master circuit-symmetry assistance must be boolean")
    if numerator_symmetry_assistance is not None and type(numerator_symmetry_assistance) is not bool:
        raise ValueError("master numerator-symmetry assistance must be boolean")
    if finite_feedback is not None and type(finite_feedback) is not bool:
        raise ValueError("master finite feedback must be boolean")
    if deep_verification is not None and type(deep_verification) is not bool:
        raise ValueError("publication deep verification must be boolean")
    if deep_verification is not None and enabled:
        raise ValueError("deep verification applies to publication, not artifact refinement")
    if containing_sector_depth is not None and (type(containing_sector_depth) is not int or containing_sector_depth < 0):
        raise ValueError("master containing-sector depth must be nonnegative")
    if existing is not None:
        if (existing.get("schema") != SCHEMA or existing.get("enabled") is not True
                or type(existing.get("seed_depth")) is not int or existing["seed_depth"] < 0
                or type(existing.get("saved_rule_assistance", False)) is not bool
                or type(existing.get("circuit_symmetry_assistance", False)) is not bool
                or type(existing.get("numerator_symmetry_assistance", False)) is not bool
                or type(existing.get("finite_feedback", True)) is not bool
                or type(existing.get("deep_verification", False)) is not bool
                or existing.get("normalization_profile") not in (None, *NORMALIZATION_PROFILES)
                or type(existing.get("containing_sector_depth", 0)) is not int
                or existing.get("containing_sector_depth", 0) < 0):
            raise ValueError("invalid persisted master-reduction policy")
        cap = existing.get("max_relation_columns")
        if cap is not None and (type(cap) is not int or cap < 0):
            raise ValueError("invalid persisted master relation column budget")
        if enabled and not fresh_search and seed_depth is not None and seed_depth < existing["seed_depth"]:
            raise ValueError("cannot lower the previous master seed depth")
        if (enabled and not fresh_search and containing_sector_depth is not None
                and containing_sector_depth < existing.get("containing_sector_depth", 0)):
            raise ValueError("cannot lower the previous master containing-sector depth")
    if seed_depth is not None and not enabled:
        raise ValueError("master seed depth requires explicit --refine-masters")
    if saved_rule_assistance is not None and not enabled:
        raise ValueError("master saved-rule assistance requires explicit --refine-masters")
    if circuit_symmetry_assistance is not None and not enabled:
        raise ValueError("master circuit-symmetry assistance requires explicit --refine-masters")
    if numerator_symmetry_assistance is not None and not enabled:
        raise ValueError("master numerator-symmetry assistance requires explicit --refine-masters")
    if finite_feedback is not None and not enabled:
        raise ValueError("master finite feedback requires explicit --refine-masters")
    if containing_sector_depth is not None and not enabled:
        raise ValueError("master containing-sector depth requires explicit --refine-masters")
    if normalization_profile is not None and not enabled:
        raise ValueError("master normalization profile requires explicit --refine-masters")
    if collection_artifacts is not None and not enabled:
        raise ValueError("master collection artifacts require explicit --refine-masters")
    members = (preferences.get("collection_artifacts", []) if collection_artifacts is None
               else [str(Path(path).resolve()) for path in collection_artifacts])
    if not isinstance(members, list) or any(not isinstance(path, str) for path in members):
        raise ValueError("invalid persisted master collection artifacts")
    policy = {**preferences, "schema": SCHEMA, "enabled": True,
            "operation": "refine" if enabled else "publish",
            "fresh_search": fresh_search,
            "max_relation_columns": (preferences.get("max_relation_columns")
                                     if max_relation_columns is None else max_relation_columns),
            "seed_depth": preferences.get("seed_depth", 0) if seed_depth is None else seed_depth,
            "saved_rule_assistance": (preferences.get("saved_rule_assistance", False)
                                      if saved_rule_assistance is None else saved_rule_assistance),
            "circuit_symmetry_assistance": (preferences.get("circuit_symmetry_assistance", False)
                                            if circuit_symmetry_assistance is None else circuit_symmetry_assistance),
            "numerator_symmetry_assistance": (preferences.get("numerator_symmetry_assistance", False)
                                              if numerator_symmetry_assistance is None else numerator_symmetry_assistance),
            "finite_feedback": (preferences.get("finite_feedback", True)
                                if finite_feedback is None else finite_feedback),
            "deep_verification": (preferences.get("deep_verification", False)
                                  if deep_verification is None else deep_verification),
            "containing_sector_depth": (preferences.get("containing_sector_depth", 0)
                                        if containing_sector_depth is None else containing_sector_depth),
            "terminal_policy": "bounded exact search; finite nonminimal basis permitted",
            "master_minimality_claim": False}
    policy["collection_strategy"] = COLLECTION_STRATEGY
    policy["refinement_strategy"] = strategy
    policy["retry_modular_proposals"] = modular_policy is not None
    if strategy == "modular-circuits":
        policy["terminal_policy"] = "heuristic modular source discovery; exact accepted terminal identities; no minimality claim"
    if modular is not None:
        policy["modular_policy"] = modular
    if enabled and strategy == "modular-circuits" and policy["numerator_symmetry_assistance"]:
        raise ValueError("numerator-symmetry assistance requires exact refinement; disable it or choose --refinement-strategy exact")
    if enabled and strategy == "modular-circuits" and (policy["saved_rule_assistance"]
                                                      or policy["circuit_symmetry_assistance"]):
        raise ValueError("modular-circuits currently requires ordinary sources without saved-rule or circuit-symmetry assistance")
    if strategy == "modular-circuits" and (relation_batch_rows is not None or relation_block_rows is not None):
        raise ValueError("relation batch/block rows configure exact refinement; use --modular-policy batch_rows/max_candidates for modular-circuits")
    if refinement_workers is not None and (not enabled or type(refinement_workers) is not int or refinement_workers <= 0):
        raise ValueError("refinement workers require explicit refinement and a positive count")
    if refinement_cpus is not None and (not enabled or not isinstance(refinement_cpus,str) or not refinement_cpus):
        raise ValueError("refinement CPU affinity requires explicit refinement and a nonempty CPU set")
    for name,value in (("refinement_workers",refinement_workers),("refinement_cpus",refinement_cpus)):
        value=value if value is not None else preferences.get(name)
        if value is not None:
            if ((name=="refinement_workers" and (type(value) is not int or value<=0)) or
                    (name=="refinement_cpus" and (not isinstance(value,str) or not value))):
                raise ValueError("invalid saved refinement worker/CPU resource choice")
            policy[name]=value
    for name, value in (("relation_batch_rows",relation_batch_rows),("relation_block_rows",relation_block_rows)):
        if value is not None and (type(value) is not int or value <= 0 or not enabled):
            raise ValueError("relation batch/block rows require explicit refinement and positive sizes")
        value = value if value is not None else preferences.get(name)
        if value is not None:
            if type(value) is not int or value <= 0: raise ValueError("invalid saved relation batch/block rows")
            policy[name] = value
    policy["finite_feedback_recipe"] = FINITE_FEEDBACK_RECIPE
    policy["publication_verification_recipe"] = PUBLICATION_VERIFICATION_RECIPE
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
    left_engine = left.get("components", {}).get("engine")
    right_engine = right.get("components", {}).get("engine")
    if left_engine != right_engine:
        return False
    if left_engine == "rustred.finite-demand.v1":
        fields = ("engine", "generation_digest", "amendments", "selection_sha256", "queries_sha256")
        return all(left.get("components", {}).get(key) == right.get("components", {}).get(key) for key in fields)
    fields = ("checkpoint_manifest_sha256", "amendments", "selection_sha256", "queries_sha256")
    return all(left.get("components", {}).get(key) == right.get("components", {}).get(key) for key in fields)


def unfinished_refinement(campaign, binding, include_uncheckpointed_fresh=False):
    """Locate the current scope's retained finite cursor without altering it."""
    campaign = Path(campaign).resolve()
    path = campaign / "master-reduction/active-phase.json"
    if not path.is_file():
        return None
    active = read_json(path)
    if active.get("operation") != "refine":
        return None
    source = Path(active["directory"]).resolve()
    if not source.is_relative_to(campaign / "master-reduction/scopes"):
        raise ValueError("refinement checkpoint must remain inside the campaign scopes directory")
    if not (source / "latest.json").is_file():
        if include_uncheckpointed_fresh:
            steering = read_json(source / "steering-binding.json")
            if (steering.get("fresh_search") is True
                    and steering.get("fresh_search_recipe") == FRESH_SEARCH_RECIPE
                    and same_input_scope(steering.get("scope_binding", {}), binding)):
                # A selected round may stop during native initialization, before
                # its first checkpoint. Retain the mode, never invent proofs.
                return {"directory": source, "native": {}, "steering": steering,
                        "active": active, "manifest_sha256": None}
        return None
    native = read_json(source / "latest.json")
    if native.get("status") in ("completed_nonminimal", "published_unrefined"):
        return None
    steering = read_json(source / "steering-binding.json")
    if not same_input_scope(steering.get("scope_binding", {}), binding):
        return None
    return {"directory": source, "native": native, "steering": steering, "active": active,
            "manifest_sha256": digest(source / "latest.json")}


def require_retained_refinement(campaign, binding=None, action="extending"):
    """Called while extension holds both the checkpoint and dispatcher locks."""
    campaign = Path(campaign)
    if binding is None:
        binding = scope_binding(campaign, campaign / "checkpoints/main")
    pending = unfinished_refinement(campaign, binding)
    if pending is None:
        return
    previous = completed_artifact(campaign)
    retained = (previous or {}).get("refinement", {})
    if (previous and same_input_scope(previous.get("scope_binding", {}), binding)
            and retained.get("source_directory") == str(pending["directory"].relative_to(campaign.resolve()))
            and retained.get("source_manifest_sha256") == pending["manifest_sha256"]):
        completed_path = campaign / "master-reduction/completed-phase.json"
        if completed_path.is_file():
            completed = read_json(completed_path)
            if (Path(completed.get("directory", "")).resolve() == previous["resolved_directory"]
                    and same_input_scope(completed.get("scope_binding", {}), binding)):
                return
    raise ValueError(f"current scope has unfinished refinement; retain its proved maps before {action}: "
                     f"saved_campaign.py finalize-refinement --campaign {campaign}")


def finalize_refinement(plan, policy, binding, request, driver):
    campaign = Path(plan["campaign_directory"])
    pending = unfinished_refinement(campaign, binding)
    if pending is None:
        raise ValueError("no unfinished refinement checkpoint for the current scope")
    if pending["native"].get("status") not in ("paused", "failed"):
        raise ValueError("finalize-refinement requires a stopped paused or failed native refinement checkpoint")
    source, steering, native = pending["directory"], pending["steering"], pending["native"]
    refinement = {
        "source_directory": str(source.relative_to(campaign)),
        "source_manifest_sha256": pending["manifest_sha256"],
        "source_artifact": steering["source"],
        "refinement_strategy": steering.get("refinement_strategy", "exact"),
        **({"fresh_search": True, "fresh_search_recipe": FRESH_SEARCH_RECIPE}
           if steering.get("fresh_search", False) else {}),
        "collection_strategy": COLLECTION_STRATEGY,
        "collection_inputs": steering.get("collection_inputs", []),
        "finite_feedback": native.get("finite_feedback", steering.get("finite_feedback", False)),
        "finite_feedback_recipe": native.get("finite_feedback_recipe", FINITE_FEEDBACK_RECIPE),
        "seed_depth": native.get("seed_depth", steering.get("seed_depth", 0)),
        "containing_sector_depth": native.get("containing_sector_depth", steering.get("containing_sector_depth", 0)),
        "saved_rule_assistance": steering.get("saved_rule_assistance", False),
        "circuit_symmetry_assistance": steering.get("circuit_symmetry_assistance", False),
        **({"numerator_symmetry_assistance": True}
           if steering.get("numerator_symmetry_assistance", False) else {}),
        **numerator_symmetry_metadata(native)}
    key = hashlib.sha256(json.dumps({"partial_refinement": str(source),
                                    "source_manifest_sha256": pending["manifest_sha256"]},
                                   sort_keys=True).encode()).hexdigest()
    target = campaign / "master-reduction/scopes" / key
    previous = completed_artifact(campaign)
    if (previous and previous["resolved_directory"] == target
            and same_input_scope(previous.get("scope_binding", {}), binding)
            and previous.get("refinement", {}).get("source_manifest_sha256") == pending["manifest_sha256"]):
        # Repair the second pointer after an interrupted pointer update without
        # replaying the export or changing the retained source cursor.
        driver.write_json(campaign / "master-reduction/completed-phase.json", {
            "schema": SCHEMA, "directory": str(target), "scope_binding": binding,
            "finite_search_complete": False, "refinement_complete": False,
            "completed_unix_time": time.time(), "master_minimality_claim": False})
        print("Proved refinement maps already published; finite search remains incomplete.", flush=True)
        return 0
    return phase_two(plan, {**policy, "partial_source": str(source), "partial_refinement": refinement},
                     request, binding, target, driver)


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
    assurance = verification_metadata(native)
    if ("independently_verified" in record
            and record["independently_verified"] != assurance["independently_verified"]):
        raise ValueError("saved artifact pointer verification assurance differs from native artifact")
    numerator = numerator_symmetry_metadata(native)
    for key, value in numerator.items():
        if key in record and record[key] != value:
            raise ValueError("saved artifact pointer numerator-symmetry provenance differs from native artifact")
    return {**record, **profile, **assurance, **numerator, "resolved_directory": directory}


def verification_metadata(report):
    """Transport native assurance; complete inventory alone is not new deep proof."""
    inventory = report.get("inventory", {})
    deep = report.get("deep_verification")
    if deep is not None and type(deep) is not bool:
        raise ValueError("invalid native publication verification mode")
    verified = inventory.get("independently_verified")
    if verified is None:
        # Historical publications always ran the independent verifier.
        verified = deep is not False and inventory.get("complete") is True
    if type(verified) is not bool:
        raise ValueError("invalid native publication verification assurance")
    return {"deep_verification": deep if deep is not None else verified,
            "publication_verification_mode": "independent_replay" if (deep is True or verified) else "inventory",
            "independently_verified": verified}


def normalization_metadata(report):
    """Read launch policy, not mathematical proof: native authenticates limits."""
    profile = report.get("normalization_profile", "conservative-v1")
    if profile not in NORMALIZATION_PROFILES.values():
        raise ValueError("unknown native normalization profile")
    if "normalization_limits" in report and "normalization_profile" not in report:
        raise ValueError("native normalization limits have no versioned profile")
    return {"normalization_profile": profile,
            **({"normalization_limits": report["normalization_limits"]} if "normalization_limits" in report else {})}


def numerator_symmetry_metadata(report):
    """Transport native proof provenance; never infer new provider authority."""
    result = {}
    if "numerator_symmetry_assistance" in report:
        if type(report["numerator_symmetry_assistance"]) is not bool:
            raise ValueError("invalid native numerator-symmetry assistance mode")
        result["numerator_symmetry_assistance"] = report["numerator_symmetry_assistance"]
    certificate = report.get("numerator_symmetry_certificate")
    if certificate is not None:
        if not isinstance(certificate, dict) or certificate.get("recipe") != NUMERATOR_SYMMETRY_RECIPE:
            raise ValueError("invalid native numerator-symmetry certificate descriptor")
        result["numerator_symmetry_certificate"] = certificate
    authority = report.get("inherited_equation_authority")
    if authority is not None:
        if not isinstance(authority, dict) or type(authority.get("numerator_symmetry_assistance", False)) is not bool:
            raise ValueError("invalid inherited numerator-symmetry authority")
        result["inherited_equation_authority"] = authority
    return result


def publish_pointer(campaign, directory, binding, operation, driver, refinement=None):
    # Called under the dispatcher lock, only after native success. An interrupted
    # refinement leaves the last valid publication visible and untouched.
    (Path(campaign) / "artifacts").mkdir(exist_ok=True)
    native = read_json(Path(directory) / "artifact.json")
    partial = (native.get("finite_search_complete") is False
               and isinstance(native.get("partial_refinement_source"), dict))
    driver.write_json(Path(campaign) / "artifacts/latest.json", {
        "schema": "rustred.saved-artifact-pointer.v1",
        "directory": str(Path(directory).resolve().relative_to(Path(campaign).resolve())),
        "scope_binding": binding, "operation": operation,
        "status": "published_partial_refinement" if partial else
                  "published_unrefined" if operation == "publish" else "completed_nonminimal",
        "completed_unix_time": time.time(), "master_minimality_claim": False,
        **normalization_metadata(native), **verification_metadata(native),
        **numerator_symmetry_metadata(native),
        **({"refinement_strategy": "modular-circuits",
            "finite_search_complete": native.get("finite_search_complete") is True,
            "modular_search_complete": native.get("modular_search_complete") is True,
            "refinement_complete": native.get("refinement_complete") is True}
           if native.get("refinement_strategy") == "modular-circuits" else {}),
        **({"finite_search_complete": False, "refinement_complete": False,
            "partial_refinement_source": native.get("partial_refinement_source")} if partial else {}),
        **({"refinement": refinement} if refinement is not None else {})})


def _arguments(command, option):
    return [command[index + 1] for index, value in enumerate(command[:-1]) if value == option]


def completed_request(campaign, checkpoint, binding, driver):
    """Find a drained native receipt bound to this exact CP6 and amendment chain.

    This admits native inventory extraction and optional independent replay.
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
    descriptor = os.open(directory / "dispatcher.lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    with os.fdopen(descriptor, "a+b") as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError("phase dispatcher lock must be a regular file")
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
    finalizing = operation == "finalize-refinement"
    publishing = operation != "refine"
    phase = "Partial refinement publication" if finalizing else "Artifact publication" if publishing else "Master refinement"
    if not publishing:
        # Refinement resources are independent of the saved closure engine.
        # Never rewrite its frozen worker/affinity policy just to refine masters.
        plan = {**plan,"requested_workers":policy.get("refinement_workers",plan["requested_workers"]),
                "cpus":policy.get("refinement_cpus",plan["cpus"])}
    supervisor = driver.SUPERVISOR
    telemetry, dashboard = supervisor.MONITOR.TELEMETRY, supervisor.MONITOR.DASHBOARD
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    attempt = directory / "runs" / timestamp
    finite_output_path(plan, attempt)
    attempt.mkdir(parents=True)
    events, stop_file = attempt / "events.jsonl", attempt / "stop-request.json"
    executable = driver.steering_executable(plan["steering_policy"])[1]
    if policy.get("executable"):
        executable = Path(plan["campaign_directory"]) / "master-reduction" / policy["executable"]["path"]
    command = [str(executable), "walk-finalize-refinement" if finalizing else
               "walk-publish" if publishing else "walk-master-reduce"]
    if finalizing:
        command += ["--refinement", str(policy["partial_source"])]
    elif publishing:
        command += ["--command", str(request)]
        if plan.get("engine") == "rustred.finite-demand.v1":
            command += ["--finite-directory", plan["checkpoint_directory"],
                        "--finite-policy", plan["finite_policy"]]
        else:
            command += ["--checkpoint", plan["checkpoint_directory"]]
        command.append("--deep-verification" if policy.get("deep_verification", False) else "--no-deep-verification")
    else:
        command += ["--artifact", str(policy["source_artifact"])]
    if plan.get("engine") == "rustred.finite-demand.v1" and (finalizing or not publishing):
        # The same caller-selected IO policy governs publication, refinement,
        # and cold reads; artifact metadata cannot silently enlarge it.
        command += ["--finite-policy", plan["finite_policy"]]
    command += ["--directory", str(directory),
               "--events", str(events), "--stop-file", str(stop_file),
               "--checkpoint-interval-seconds", str(plan["checkpoint_interval_seconds"]),
               "--threads", str(plan["requested_workers"])]
    if not publishing:
        command += ["--seed-depth", str(policy["seed_depth"])]
        if policy.get("fresh_search", False):
            command.append("--fresh-search")
        strategy = policy.get("refinement_strategy", "exact")
        if strategy != "exact":
            command += ["--refinement-strategy", strategy]
            if policy.get("modular_policy") is not None:
                modular_path = attempt / "modular-policy.json"
                driver.write_json(modular_path, policy["modular_policy"])
                command += ["--modular-policy", str(modular_path)]
        for name in ("relation_batch_rows", "relation_block_rows"):
            if strategy == "exact" and policy.get(name) is not None:
                command += ["--" + name.replace("_", "-"), str(policy[name])]
        if policy.get("max_relation_columns") is not None:
            command += ["--max-relation-columns", str(policy["max_relation_columns"])]
        command.append("--finite-feedback" if policy.get("finite_feedback", True) else "--no-finite-feedback")
        for member in policy.get("collection_artifacts", []):
            command += ["--collection-artifact", member]
        if policy.get("containing_sector_depth", 0) or policy.get("fresh_search", False):
            command += ["--containing-sector-depth", str(policy["containing_sector_depth"])]
        if policy.get("saved_rule_assistance", False):
            command.append("--saved-rule-assistance")
        if policy.get("circuit_symmetry_assistance", False):
            command.append("--circuit-symmetry-assistance")
        if policy.get("numerator_symmetry_assistance", False):
            command.append("--numerator-symmetry-assistance")
        profile = policy.get("effective_normalization_profile", "conservative-v1")
        if profile != "conservative-v1" or policy.get("normalization_profile") is not None:
            command += ["--normalization-profile", profile.removesuffix("-v1")]
    if (directory / "latest.json").is_file():
        snapshot = read_json(directory / "latest.json")
        if finalizing:
            pass  # Native authenticates the retained source generation.
        elif publishing:
            if snapshot.get("deep_verification", True) != policy.get("deep_verification", False):
                raise ValueError("cannot change deep verification on resume; start a new publication phase")
        else:
            if snapshot.get("refinement_strategy", "exact") != strategy:
                raise ValueError("cannot change refinement strategy on resume; start a new phase directory")
            if snapshot.get("numerator_symmetry_assistance", False) != policy.get("numerator_symmetry_assistance", False):
                raise ValueError("cannot change numerator-symmetry assistance on resume; start a new phase directory")
            if (snapshot.get("fresh_search", False) != policy.get("fresh_search", False)
                    or (policy.get("fresh_search", False)
                        and snapshot.get("fresh_search_recipe") != FRESH_SEARCH_RECIPE)):
                raise ValueError("cannot change fresh-search mode on resume; start a new phase directory")
            pinned = normalization_metadata(snapshot)["normalization_profile"]
            if pinned != profile:
                raise ValueError("cannot change normalization profile on resume; start a new phase directory")
            if (snapshot.get("finite_feedback", False) != policy.get("finite_feedback", True)
                    or (snapshot.get("finite_feedback", False)
                        and snapshot.get("finite_feedback_recipe") != FINITE_FEEDBACK_RECIPE)):
                raise ValueError("cannot change finite feedback on resume; start a new phase directory")
        command.append("--resume")
    elif publishing and not finalizing:
        previous_path = directory.parent.parent / "completed-phase.json"
        if policy.get("previous_artifact"):
            command += ["--previous-artifact", policy["previous_artifact"]]
        elif previous_path.is_file():
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
    admission_host = supervisor.memory_host_snapshot(options)
    hard, soft, reserve = supervisor.memory_admission_for_resources(options, admission_host)
    guard = supervisor.RamGuard(hard, soft, reserve,
        options.get("swap_growth_stop_bytes_per_second") or 0,
        options.get("swap_growth_stop_seconds") or supervisor.DEFAULT_SWAP_GROWTH_STOP_SECONDS)
    environment = dict(os.environ, SYMBOLICA_HIDE_BANNER="1")
    for name in supervisor.INNER_POOLS:
        environment[name] = "1"  # Native --threads owns the outer compute budget.
    for name in supervisor.DIAGNOSTIC_ONLY_ENVIRONMENT:
        environment.pop(name, None)
    driver.write_json(attempt / "request.json", {"command": command, "cpus": sorted(cpus),
                      "scope_binding": binding, "phase": phase, "operation": operation,
                      "memory": supervisor.memory_admission_record(options["max_memory_bytes"], admission_host,
                          reserve, hard, soft, options.get("memory_policy", "host-aware"))})
    if not finalizing:
        driver.write_json(directory.parent.parent / "active-phase.json", {
            "schema": SCHEMA, "phase": phase, "operation": operation, "scope_binding": binding["key"],
            **({"normalization_profile": profile} if not publishing else {}),
            **({"deep_verification": policy.get("deep_verification", False)} if publishing else {}),
            "directory": str(directory), "run_directory": str(attempt), "request": str(request)})
    presenter = dashboard.Presenter()
    stream = telemetry.TelemetryStream(attempt / "telemetry.jsonl")
    tail = supervisor.MONITOR.EventTail(events)
    latest = {"phase": phase, "operation": operation,
              "stage": "rebinding retained refinement" if finalizing else
                       ("independent_graph_replay" if policy.get("deep_verification", False) else "inventory_census")
                       if publishing else "loading published artifact", "status": "running",
              "seed_depth": policy.get("effective_seed_depth", policy["seed_depth"]), "scope_binding": binding["key"],
              "containing_sector_depth": (policy.get("effective_containing_sector_depth",
                                                    policy.get("containing_sector_depth", 0)) if not publishing else 0),
              "saved_rule_assistance": not publishing and policy.get("saved_rule_assistance", False),
              "circuit_symmetry_assistance": not publishing and policy.get("circuit_symmetry_assistance", False),
              "numerator_symmetry_assistance": not publishing and policy.get("numerator_symmetry_assistance", False)}
    latest["finite_feedback"] = not publishing and policy.get("finite_feedback", True)
    if not publishing:
        latest["normalization_profile"] = profile
        latest["refinement_strategy"] = strategy
        if policy.get("fresh_search", False):
            latest.update(fresh_search=True, fresh_search_recipe=FRESH_SEARCH_RECIPE,
                          stage="loading retained proved maps for a fresh search")
    elif not finalizing:
        latest["deep_verification"] = policy.get("deep_verification", False)
        latest["publication_verification_mode"] = "independent_replay" if latest["deep_verification"] else "inventory"
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
                        host = supervisor.memory_host_snapshot(options)
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
                                      "host_available_bytes": host.get("host_available_bytes", host.get("available_bytes"))}
                    previous_time = now
                    publish("stopping" if stop_reason else "running")
                    time.sleep(.5)
                status = child.wait()
        tail.poll()
        # The CLI also uses exit 4 for input errors. Only the native event
        # attesting a cooperative pause justifies displaying a resumable pause.
        native_status = status
        stopped_completion = (plan.get("engine") == "rustred.finite-demand.v1"
                              and status == 0 and stop_reason is not None)
        if stopped_completion:
            status = 8  # Keep any durable artifact, but do not promote pointers after a stop.
        paused = stopped_completion or (status == 4 and latest.get("status") == "paused")
        finished_state = "published_partial_refinement" if finalizing else "published_unrefined" if publishing else "completed_nonminimal"
        publish(finished_state if status == 0 else "paused" if paused else "failed", force=True)
        driver.write_json(attempt / "supervisor-result.json", {"exit_status": status, "stop_reason": stop_reason,
                          "native_exit_status": native_status,
                          "phase": phase, "operation": operation, "scope_binding": binding["key"],
                          "peak_observed_rss_bytes": peak, "elapsed_seconds": time.monotonic() - started})
        if status == 0:
            if not (directory / "artifact.json").is_file():
                raise ValueError("native success did not produce its completed artifact.json")
            native_artifact = read_json(directory / "artifact.json")
            native_profile = normalization_metadata(native_artifact)
            native_assurance = verification_metadata(native_artifact)
            if publishing and not finalizing and (native_assurance["deep_verification"] != policy.get("deep_verification", False)
                               or (policy.get("deep_verification", False) and not native_assurance["independently_verified"])):
                raise ValueError("native artifact verification assurance differs from requested publication")
            if not publishing and native_profile["normalization_profile"] != profile:
                raise ValueError("native artifact normalization profile differs from requested refinement")
            if not publishing and native_artifact.get("refinement_strategy", "exact") != strategy:
                raise ValueError("native artifact strategy differs from requested refinement")
            if not publishing and native_artifact.get("numerator_symmetry_assistance", False) != policy.get("numerator_symmetry_assistance", False):
                raise ValueError("native artifact numerator-symmetry mode differs from requested refinement")
            if not publishing and (native_artifact.get("fresh_search", False) != policy.get("fresh_search", False)
                    or (policy.get("fresh_search", False)
                        and native_artifact.get("fresh_search_recipe") != FRESH_SEARCH_RECIPE)):
                raise ValueError("native artifact fresh-search mode differs from requested refinement")
            if not publishing and (native_artifact.get("finite_feedback") != policy.get("finite_feedback", True)
                                   or native_artifact.get("finite_feedback_recipe") != FINITE_FEEDBACK_RECIPE):
                raise ValueError("native artifact finite feedback recipe differs from requested refinement")
            if finalizing and (native_artifact.get("finite_search_complete") is not False
                               or native_artifact.get("refinement_complete") is not False
                               or not native_artifact.get("partial_refinement_source")):
                raise ValueError("native partial export must attest incomplete refinement and its source")
            if finalizing and digest(Path(policy["partial_source"]) / "latest.json") != policy["partial_refinement"]["source_manifest_sha256"]:
                raise ValueError("refinement source changed during partial export; artifact retained but pointer not updated")
            current = phase_scope_binding(plan, driver)
            if not same_input_scope(current, binding):
                raise ValueError("campaign scope changed during postprocessing; artifact retained but latest pointer not updated")
            previous = completed_artifact(plan["campaign_directory"])
            keep_refined = publishing and not finalizing and previous and same_input_scope(previous.get("scope_binding", {}), binding) \
                and previous.get("operation") == "refine" and not policy.get("previous_artifact")
            if not keep_refined:
                refinement = None if publishing else {
                        **native_profile,
                        **numerator_symmetry_metadata(native_artifact),
                        "refinement_strategy": policy.get("refinement_strategy", "exact"),
                        **({"fresh_search": True, "fresh_search_recipe": FRESH_SEARCH_RECIPE}
                           if policy.get("fresh_search", False) else {}),
                        "modular_search_complete": native_artifact.get("modular_search_complete", False),
                        "collection_strategy": COLLECTION_STRATEGY,
                        "finite_feedback": policy.get("finite_feedback", True),
                        "finite_feedback_recipe": FINITE_FEEDBACK_RECIPE,
                        "collection_inputs": policy.get("collection_inputs", []),
                        "seed_depth": latest["seed_depth"],
                        "containing_sector_depth": latest["containing_sector_depth"],
                        "circuit_symmetry_assistance": policy.get("circuit_symmetry_assistance", False),
                        "saved_rule_assistance": policy.get("saved_rule_assistance", False), "source_artifact": str(
                            Path(policy["source_artifact"]).relative_to(Path(plan["campaign_directory"])))}
                pointer_operation = operation
                if finalizing:
                    refinement = {**policy["partial_refinement"], **native_profile,
                                  "finite_search_complete": False, "refinement_complete": False}
                    pointer_operation = "refine"
                if publishing and policy.get("preserved_refinement"):
                    refinement = policy["preserved_refinement"]
                    pointer_operation = "refine"
                publish_pointer(plan["campaign_directory"], directory, binding, pointer_operation, driver, refinement)
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
                **({"finite_search_complete": False, "refinement_complete": False} if finalizing else {}),
                "completed_unix_time": time.time(), "master_minimality_claim": False})
            # Older native events omit the inventory even though artifact.json
            # contains it. Refresh the final dashboard and persistent status
            # from that already loaded metadata; never replay owner payloads.
            counts = telemetry.publication_rule_counts(native_artifact)
            latest.update(counts)
            latest.update(numerator_symmetry_metadata(native_artifact))
            for name in ("numerator_symmetry_preparation", "numerator_symmetry_reduction"):
                if isinstance(native_artifact.get(name), dict):
                    latest[name] = native_artifact[name]
            publish(finished_state, force=True)
            print("Proved refinement maps published; finite search remains incomplete and its cursor is retained." if finalizing else
                  "Scoped artifact published; master refinement was not requested." if publishing else
                  "Modular discovery pass completed; accepted maps are exactly verified; exhaustive exact search remains incomplete."
                  if policy.get("refinement_strategy", "exact") == "modular-circuits" else
                  "Explicit master refinement completed (bounded search, not a minimality proof).", flush=True)
            for label, key in (("Published executable rule records", "published_rule_records"),
                               ("Encountered cover rule records", "observed_cover_rules")):
                value = counts[key]
                print(f"{label}: {value:,}" if value is not None else f"{label}: unknown", flush=True)
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
                                + ("--master-numerator-symmetry-assistance "
                                   if policy.get("numerator_symmetry_assistance", False)
                                   else "--no-master-numerator-symmetry-assistance ")
                                + ("--master-finite-feedback " if policy.get("finite_feedback", True)
                                   else "--no-master-finite-feedback ")
                                + ("--master-refinement-strategy modular-circuits "
                                   if policy.get("refinement_strategy", "exact") == "modular-circuits" else "")
                                + "--master-normalization-profile " + profile.removesuffix("-v1") + " ") if not publishing else (
                                    "--deep-verification " if policy.get("deep_verification", False) else "--no-deep-verification ")
            if finalizing:
                print(f"Resume export with: saved_campaign.py finalize-refinement --campaign {plan['campaign_directory']}", flush=True)
            elif plan.get("engine") == "rustred.finite-demand.v1":
                print(f"Resume with: saved_campaign.py {'publish' if publishing else 'refine'} "
                      f"--campaign {plan['campaign_directory']}", flush=True)
            else:
                print(f"Resume with: {sys.executable} {Path(driver.__file__).resolve()} --campaign-directory "
                      f"{plan['campaign_directory']} --resume {refinement_flags}--start", flush=True)
            if not paused:
                print(f"Native error details: {attempt / 'stderr.log'}", file=sys.stderr)
        return status if status >= 0 else 128 - status
    finally:
        for sig, handler in handlers.items():
            signal.signal(sig, handler)


def finite_output_path(plan, path):
    """Finite phase output ancestors cannot redirect writes outside this campaign."""
    if plan.get("engine") != "rustred.finite-demand.v1":
        return
    campaign = Path(plan["campaign_directory"]).resolve()
    candidate = Path(path).absolute()
    try:
        parts = candidate.relative_to(campaign).parts
    except ValueError:
        raise ValueError("finite phase output must remain inside its campaign") from None
    current = campaign
    for index, part in enumerate(parts):
        if part in (".", ".."):
            raise ValueError("finite phase output must be a canonical campaign path")
        current /= part
        if current.is_symlink() or not current.resolve().is_relative_to(campaign):
            raise ValueError("finite phase output has a symlink/escaping ancestor: " + str(current))
        if current.exists() and index + 1 < len(parts) and not current.is_dir():
            raise ValueError("finite phase output ancestor is not a directory")


def phase_scope_binding(plan, driver):
    if plan.get("engine") == "rustred.finite-demand.v1":
        return driver.finite_scope_binding(plan)
    checkpoint = Path(plan["checkpoint_directory"])
    return scope_binding(plan["campaign_directory"], checkpoint) if (checkpoint / "latest.json").is_file() else None


def phase_completed_request(plan, binding, driver):
    if plan.get("engine") == "rustred.finite-demand.v1":
        return driver.finite_completed_request(plan, binding)
    return completed_request(plan["campaign_directory"], plan["checkpoint_directory"], binding, driver)


@contextmanager
def phase_source_lock(plan, directory, driver, held, dispatcher_held=False):
    if plan.get("engine") == "rustred.finite-demand.v1":
        for path in (directory / "dispatcher.lock", directory / "bin", directory / "scopes",
                     directory / "policy.json", directory / "active-phase.json", directory / "completed-phase.json",
                     Path(plan["campaign_directory"]) / "artifacts/latest.json"):
            finite_output_path(plan, path)
        # Global finite order: dispatcher -> control -> native StoreWriter.
        # Drop control before launching a native child, retaining dispatcher.
        if not dispatcher_held:
            held.enter_context(dispatcher_lock(directory))
        with driver.control_lock(Path(plan["checkpoint_directory"])):
            yield
    else:
        # Preserve the legacy CP6 acquisition order and behavior.
        with driver.checkpoint_lock(Path(plan["checkpoint_directory"])):
            held.enter_context(dispatcher_lock(directory))
            yield


def run(plan, policy, resume, driver, postprocess_only=False, dispatcher_held=False, startup_progress=None):
    campaign, checkpoint = Path(plan["campaign_directory"]), Path(plan["checkpoint_directory"])
    directory = campaign / "master-reduction"
    fresh_requested = policy.get("fresh_search", False)
    with ExitStack() as held:
        if startup_progress is not None:
            held.callback(startup_progress.stop)
            startup_progress.phase("checking campaign phase locks")
        # Same acquisition order as the rank-extension helper. Creating the
        # first dispatcher.lock is serialized against an existing CP6 lock,
        # closing the first-opt-in race with an amendment publication. Release
        # CP6 before starting a native walk; retain the phase lock throughout.
        with phase_source_lock(plan, directory, driver, held, dispatcher_held):
            evidence = driver.campaign_run_liveness(campaign)
            if evidence:
                raise ValueError("campaign is still running: " + "; ".join(evidence))
            if fresh_requested:
                if policy.get("operation") != "refine":
                    raise ValueError("fresh search requires explicit refinement")
                fresh_binding = phase_scope_binding(plan, driver)
                if fresh_binding is None:
                    raise ValueError("fresh search requires the completed current input scope")
                fresh_pending = unfinished_refinement(campaign, fresh_binding)
                if fresh_pending and fresh_pending["native"].get("status") not in ("paused", "failed"):
                    raise ValueError("fresh search requires a stopped refinement; finalize-refinement retains its proved maps")
                require_retained_refinement(campaign, fresh_binding, "starting a fresh search")
            if plan.get("engine") == "rustred.finite-demand.v1" and policy.get("executable"):
                finite_output_path(plan, directory / policy["executable"]["path"])
            if startup_progress is not None:
                startup_progress.phase("freezing publication executable")
            freeze_master_executable(directory, policy, driver)
            driver.write_json(directory / "policy.json", {key: value for key, value in policy.items()
                              if key not in ("operation", "source_artifact", "previous_artifact", "preserved_refinement",
                                             "fresh_search", "fresh_search_recipe")})
            if startup_progress is not None:
                startup_progress.phase("validating current input scope")
            binding = (None if plan.get("engine") == "rustred.finite-demand.v1" and plan.get("repair_preparation")
                       else phase_scope_binding(plan, driver))
            request = phase_completed_request(plan, binding, driver) if (resume or postprocess_only) and binding else None
        operation = policy.get("operation", "publish")
        if operation == "finalize-refinement":
            if not binding or request is None:
                raise ValueError("finalize-refinement requires the completed current input scope")
            if startup_progress is not None:
                startup_progress.stop()
            return finalize_refinement(plan, policy, binding, request, driver)
        if startup_progress is not None:
            startup_progress.phase("checking existing publication")
        if operation == "refine":
            previous = completed_artifact(campaign)
            if not previous or not binding or not same_input_scope(previous.get("scope_binding", {}), binding):
                raise ValueError("refine requires a completed artifact for the current scope; run or publish it first")
            refinement = previous.get("refinement", {})
            pending = unfinished_refinement(campaign, binding, include_uncheckpointed_fresh=True)
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
            strategy = policy.get("refinement_strategy", "exact")
            searched = (refinement.get("modular_search_complete", False) if strategy == "modular-circuits"
                        else previous.get("finite_search_complete", True))
            if (previous.get("operation") == "refine"
                    and not fresh_requested
                    and not (pending and pending["steering"].get("fresh_search", False))
                    and searched
                    and not policy.get("retry_modular_proposals", False)
                    and refinement.get("refinement_strategy", "exact") == strategy
                    and refinement.get("collection_strategy") == COLLECTION_STRATEGY
                    and refinement.get("finite_feedback", False) == policy.get("finite_feedback", True)
                    and (not policy.get("finite_feedback", True)
                         or refinement.get("finite_feedback_recipe") == FINITE_FEEDBACK_RECIPE)
                    and (not collection_inputs or refinement.get("collection_inputs", []) == collection_inputs)
                    and refinement.get("seed_depth", -1) >= policy["seed_depth"]
                    and refinement.get("containing_sector_depth", 0) >= policy.get("containing_sector_depth", 0)
                    and refinement.get("circuit_symmetry_assistance", False) == policy.get("circuit_symmetry_assistance", False)
                    and refinement.get("numerator_symmetry_assistance", False) == policy.get("numerator_symmetry_assistance", False)
                    and previous["normalization_profile"] == effective_profile
                    and refinement.get("saved_rule_assistance", False) == policy.get("saved_rule_assistance", False)):
                if startup_progress is not None:
                    startup_progress.stop()
                print("Requested refinement already completed: " + str(previous["resolved_directory"]), flush=True)
                return 0
            policy = {**policy, "source_artifact": str(previous["resolved_directory"]),
                      "effective_normalization_profile": effective_profile,
                      "effective_seed_depth": (policy["seed_depth"] if fresh_requested else
                                               max(policy["seed_depth"], refinement.get("seed_depth", 0))),
                      "effective_containing_sector_depth": (policy.get("containing_sector_depth", 0)
                          if fresh_requested else max(policy.get("containing_sector_depth", 0),
                                                      refinement.get("containing_sector_depth", 0)))}
        elif binding:
            previous = completed_artifact(campaign)
            same_scope = previous and same_input_scope(previous.get("scope_binding", {}), binding)
            if same_scope and policy.get("deep_verification", False) and not previous["independently_verified"]:
                policy = {**policy, "previous_artifact": str(previous["resolved_directory"]),
                          "preserved_refinement": previous.get("refinement") if previous.get("operation") == "refine" else None}
            elif same_scope:
                if startup_progress is not None:
                    startup_progress.stop()
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
            if startup_progress is not None:
                startup_progress.stop()
            status, stopped = phase_one(plan["command"])
            if stopped:
                # Native phase one may have finished just before Ctrl+C.
                return 8 if plan.get("engine") == "rustred.finite-demand.v1" else 4
            if plan.get("engine") == "rustred.finite-demand.v1" and status != 0:
                return status  # Even a complete prior CURRENT cannot turn refusal into publication.
            if status not in (0, 4):
                return status
            if startup_progress is not None:
                startup_progress.phase("validating completed input scope")
            binding = phase_scope_binding(plan, driver)
            request = phase_completed_request(plan, binding, driver) if binding else None
            if request is None:
                if startup_progress is not None:
                    startup_progress.stop()
                print("IBP campaign remains paused/incomplete; no final artifact was published.", flush=True)
                return status if status else 4
        if startup_progress is not None:
            startup_progress.phase("preparing publication launch")
        if plan.get("engine") == "rustred.finite-demand.v1" and operation == "publish":
            previous = completed_artifact(campaign)
            if previous and previous.get("operation") == "refine":
                # Exact proved maps survive append-only input scope. Native
                # publication authenticates program/family and retained rows.
                policy = {**policy, "previous_artifact": str(previous["resolved_directory"]),
                          "preserved_refinement": previous.get("refinement")}
        identity = {"scope": binding["key"], "operation": operation,
                    "seed_depth": policy["seed_depth"] if operation == "refine" else 0,
                    "source": str(Path(policy["source_artifact"]).relative_to(campaign)) if policy.get("source_artifact") else None,
                    "executable": policy.get("executable", {}).get("sha256", plan.get("executable_sha256"))}
        if operation == "publish":
            identity["deep_verification"] = policy.get("deep_verification", False)
            identity["publication_verification_recipe"] = PUBLICATION_VERIFICATION_RECIPE
            identity["previous_artifact"] = (str(Path(policy["previous_artifact"]).relative_to(campaign))
                                             if policy.get("previous_artifact") else None)
        if operation == "refine":
            identity["collection_strategy"] = COLLECTION_STRATEGY
            if fresh_requested:
                identity.update(fresh_search=True, fresh_search_recipe=FRESH_SEARCH_RECIPE)
            if policy.get("refinement_strategy", "exact") != "exact":
                identity["refinement_strategy"] = policy["refinement_strategy"]
                identity["modular_search_recipe"] = MODULAR_SEARCH_RECIPE
            identity["collection_inputs"] = policy.get("collection_inputs", [])
            identity["finite_feedback"] = policy.get("finite_feedback", True)
            identity["finite_feedback_recipe"] = FINITE_FEEDBACK_RECIPE
        if operation == "refine" and policy.get("saved_rule_assistance", False):
            # Preserve legacy ordinary checkpoint paths. Assisted work has a
            # distinct identity and can never resume an ordinary row cursor.
            identity["saved_rule_assistance"] = True
        if operation == "refine" and policy.get("circuit_symmetry_assistance", False):
            identity["circuit_symmetry_assistance"] = True
        if operation == "refine" and policy.get("numerator_symmetry_assistance", False):
            identity["numerator_symmetry_assistance"] = True
        if operation == "refine" and policy["effective_normalization_profile"] != "conservative-v1":
            # Legacy conservative phases keep their original checkpoint path.
            identity["normalization_profile"] = policy["effective_normalization_profile"]
        if operation == "refine" and policy.get("containing_sector_depth", 0):
            # Zero retains ordinary legacy paths; different source strategies
            # must not resume one another's native cursors.
            identity["containing_sector_depth"] = policy["containing_sector_depth"]
        phase_key = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
        target = directory / "scopes" / phase_key
        if operation == "refine":
            if pending and not fresh_requested:
                saved_identity = {key: value for key, value in pending["steering"].items()
                                  if key not in ("scope_binding", "executable")}
                candidate_identity = dict(identity)
                if pending["steering"].get("fresh_search", False):
                    candidate_identity.update(fresh_search=True, fresh_search_recipe=FRESH_SEARCH_RECIPE)
                if refinement.get("source_directory") == str(pending["directory"].relative_to(campaign)):
                    candidate_identity["source"] = pending["steering"]["source"]
                requested_identity = {key: value for key, value in candidate_identity.items() if key != "executable"}
                if saved_identity == requested_identity:
                    # Executable and resource budgets are operational choices.
                    # Native resume authenticates the mathematical checkpoint.
                    target = pending["directory"]
                    identity = candidate_identity
                    policy = {**policy, "source_artifact": str(campaign / identity["source"])}
                    if identity.get("fresh_search", False):
                        policy.update(fresh_search=True, effective_seed_depth=policy["seed_depth"],
                                      effective_containing_sector_depth=policy.get("containing_sector_depth", 0))
                # A changed search recipe starts from the latest partial artifact
                # so its already proved maps survive the new finite search.
        finite_output_path(plan, target / "steering-binding.json")
        target.mkdir(parents=True, exist_ok=True)
        driver.write_json(target / "steering-binding.json", {"scope_binding": binding, **identity})
        if startup_progress is not None:
            startup_progress.stop()
        return phase_two(plan, policy, request, binding, target, driver)
