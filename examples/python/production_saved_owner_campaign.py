#!/usr/bin/env python3
"""Prepare or manually launch the persistent saved-owner production campaign.

By default this verifies staged input and freezes the supplied executable, then
prints a command. Only --start launches the solver. No algebra lives in Python.
--prepare-from copies verified immutable inputs into a new, disjoint campaign;
its default helper-first query order is only an admission-order heuristic.
With --queries it stages a verified replacement query document instead, and
--attach copies planner receipts read-only beside the inputs. The frozen
steering (schema v2) fixes workers, CPUs, checkpoint interval, RAM policy,
publication policy, transfer lookahead and inspection workers; only the RAM
options may be overridden per resume.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import shutil
import sys
import tempfile

RAM_POLICY_OPTIONS = ("max_memory_bytes", "ram_guard_margin_percent")
STEERING_SCHEMA = "rustred.production-steering.v2"
STEERING_SCHEMAS = ("rustred.production-steering.v1", STEERING_SCHEMA)
FROZEN_OPTIONS = ("workers", "cpus", "checkpoint_interval_seconds", "max_memory_bytes",
                  "ram_guard_margin_percent", "apply_subdivision_axis", "apply_subdivision_cut",
                  "apply_cell_refinement_max_cardinality", "publication_policy",
                  "transfer_unreserved_lookahead", "inspection_workers")
DEFAULT_PUBLICATION_POLICY = "ready"
QUERY_SCHEMA = "rustred.owner-domain-queries.json.v2"
QUERY_ROW_FIELDS = frozenset({"id", "owner", "lower", "upper", "max_numerator_rank", "power_bounds"})
ENTRY_PLAN_RECEIPT_NAME = "entry-plan-receipt.json"
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
            raise ValueError("campaign already has a different frozen executable; use a new campaign directory")
        return target.resolve(), receipt["sha256"]
    if source is None:
        raise ValueError("first preparation requires --executable; resume retains the frozen binary")
    if not source.is_file() or not os.access(source, os.X_OK):
        raise ValueError("supplied executable must be an executable file")
    source_hash = digest(source)
    target = directory / ("rustred-" + source_hash)
    with source.open("rb") as incoming, target.open("xb") as outgoing:
        shutil.copyfileobj(incoming, outgoing, 1024 * 1024)
        outgoing.flush()
        os.fsync(outgoing.fileno())
    if digest(target) != source_hash or digest(source) != source_hash:
        raise ValueError("executable changed during freezing")
    target.chmod(0o555)
    with target.open("rb") as stream:
        os.fsync(stream.fileno())
    write_json(receipt_path, {"sha256": source_hash, "file": target.name, "source": str(source.resolve())})
    return target.resolve(), source_hash


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
            or verify_inputs(source_inputs)[2] != original
            or verify_inputs(destination / "inputs")[0] != count):
        raise ValueError("source input identity changed during fresh preparation")
    return staged


def frozen_options(policy):
    """Frozen options with v1 defaults for options a v1 steering file never recorded."""
    options = dict(policy["options"])
    command = policy.get("command_arguments", [])

    def flag_value(flag):
        return command[command.index(flag) + 1] if command.count(flag) == 1 else None

    if "publication_policy" not in options:
        options["publication_policy"] = flag_value("--publication-policy") or "ordered"
    if "transfer_unreserved_lookahead" not in options:
        lookahead = flag_value("--transfer-unreserved-lookahead")
        options["transfer_unreserved_lookahead"] = 256 if lookahead is None else int(lookahead)
    if "inspection_workers" not in options:
        inspectors = flag_value("--inspection-workers")
        options["inspection_workers"] = None if inspectors is None else int(inspectors)
    # Older frozen policies omitted this opt-in and therefore mean Off.
    options.setdefault("apply_cell_refinement_max_cardinality", None)
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
    return command


def frozen_policy(campaign, args, executable, inputs, count, size):
    """Persist original steering; only per-resume supervisor RAM may differ."""
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
            if supplied is not None and supplied != frozen.get(name):
                if getattr(args, "resume", False) and name in RAM_POLICY_OPTIONS:
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
                "transfer_unreserved_lookahead": 256}
    for name, default in defaults.items():
        if options[name] is None:
            options[name] = default
    if not 1 <= options["workers"] <= MAX_WORKERS:
        raise ValueError(f"workers must be in 1..{MAX_WORKERS}")
    cpus = (SUPERVISOR.parse_cpu_set(options["cpus"]) if options["cpus"] else
            set(sorted(affinity)[:options["workers"]]))
    if len(cpus) != options["workers"] or not cpus <= affinity:
        raise ValueError("CPU affinity must contain exactly the requested number of permitted CPUs")
    options["cpus"] = SUPERVISOR.format_cpu_set(cpus)
    if options["publication_policy"] not in ("ordered", "ready"):
        raise ValueError("publication policy must be ordered or ready")
    if options["publication_policy"] != "ordered" and options["apply_subdivision_axis"] is not None:
        raise ValueError("physical subdivision requires --publication-policy ordered")
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


def effective_supervisor_policy(policy, args):
    """Overlay resume-only RAM settings without rewriting frozen solver policy."""
    options = frozen_options(policy)
    command = list(policy["command_arguments"])
    overrides = {}
    if args.resume:
        for name in RAM_POLICY_OPTIONS:
            supplied = getattr(args, name)
            if supplied is not None and supplied != options[name]:
                overrides[name] = supplied
                options[name] = supplied
                flag = "--" + name.replace("_", "-")
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
    parser.add_argument("--resume", action="store_true", help="continue the latest native checkpoint with the frozen executable")
    parser.add_argument("--workers", type=int, help=f"initial default: at most 50 permitted CPUs (cap {MAX_WORKERS}); frozen for resume")
    parser.add_argument("--cpus", help="optional explicit affinity: comma list or ranges (128-177, 0-3,8); exactly --workers IDs")
    parser.add_argument("--run-directory", type=Path)
    parser.add_argument("--publication-policy", choices=("ordered", "ready"),
                        help=f"initial default: {DEFAULT_PUBLICATION_POLICY}; ordered remains selectable; frozen for resume")
    parser.add_argument("--transfer-unreserved-lookahead", type=int,
                        help="initial default: 256 logical dispatch lookahead; frozen for resume")
    parser.add_argument("--inspection-workers", type=int,
                        help="explicit native inspectors (default: native split); frozen for resume")
    parser.add_argument("--checkpoint-interval-seconds", type=int, help="initial default: 3600")
    parser.add_argument("--max-memory-bytes", type=int,
                        help="positive requested RAM ceiling; initial default: 500000000000; may override per resume")
    parser.add_argument("--ram-guard-margin-percent", type=float,
                        help="initial default: 5 (save+stop at 95%%); may override per resume")
    parser.add_argument("--apply-subdivision-axis", type=int)
    parser.add_argument("--apply-subdivision-cut", type=int)
    parser.add_argument("--apply-cell-refinement-max-cardinality", type=application_cardinality, action="append",
                        help="opt-in singleton refinement eligibility; positive cardinality, default off, unchanged by unbounded work; frozen for resume")
    parser.add_argument("--json", action="store_true", help="print the prepared command as JSON")
    args = parser.parse_args(argv)
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
    if (args.apply_subdivision_axis is None) != (args.apply_subdivision_cut is None):
        parser.error("subdivision requires both axis and cut")
    if args.publication_policy == "ready" and args.apply_subdivision_axis is not None:
        parser.error("ready publication cannot be combined with physical subdivision")
    if any(value is not None and value < 0 for value in (args.apply_subdivision_axis, args.apply_subdivision_cut)):
        parser.error("subdivision axis and cut must be nonnegative")
    campaign = args.campaign_directory.resolve()
    inputs = campaign / "inputs"
    try:
        if args.prepare_from is not None:
            if args.executable is None or not args.executable.is_file() or not os.access(args.executable, os.X_OK):
                raise ValueError("fresh preparation requires an executable --executable")
            default_order = "preserve" if args.queries is not None else "helpers-first"
            prepare_from(args.prepare_from, args.campaign_directory, args.query_order or default_order,
                         queries_override=args.queries, attachments=args.attach)
        count, size, receipt = verify_inputs(inputs)
        executable, executable_hash = freeze_executable(campaign, args.executable)
        policy = frozen_policy(campaign, args, executable, inputs, count, size)
        command_arguments, options, ram_overrides = effective_supervisor_policy(policy, args)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    run = args.run_directory.resolve() if args.run_directory else campaign / "runs" / timestamp
    checkpoint = campaign / "checkpoints" / "main"
    supervisor = Path(__file__).with_name("shared_owner_campaign.py").resolve()
    command = [sys.executable, str(supervisor), *command_arguments,
               "--run-directory", str(run), "--resume" if args.resume else "--checkpoint", str(checkpoint)]
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
            "transfer_unreserved_lookahead": options["transfer_unreserved_lookahead"],
            "inspection_workers": options["inspection_workers"],
            "requested_hard_memory_bytes": options["max_memory_bytes"],
            "ram_guard_margin_percent": options["ram_guard_margin_percent"],
            "supervisor_ram_policy": {name: options[name] for name in RAM_POLICY_OPTIONS},
            "supervisor_ram_overrides": ram_overrides,
            "supervisor_ram_override_scope": "this_invocation_only; omitted_values_use_original_frozen_policy",
            "checkpoint_interval_seconds": options["checkpoint_interval_seconds"],
            "steering_policy": policy, "steering_policy_sha256": digest(campaign / "bin" / "steering.json"),
            "unbounded_cumulative_work": True, "scratch_and_algebra_admission_remain_bounded": True,
            "family_closure_claim": False, "launch_requested": args.start}
    if not args.start:
        print(json.dumps(plan, indent=2) if args.json else shlex.join(command))
        return 0
    checkpoint.parent.mkdir(parents=True, exist_ok=True)
    sync_directory(campaign)
    write_json(campaign / "active-run.json", plan)
    os.execv(command[0], command)


if __name__ == "__main__":
    raise SystemExit(main())
