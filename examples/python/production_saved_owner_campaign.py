#!/usr/bin/env python3
"""Prepare or manually launch the persistent saved-owner production campaign.

By default this verifies staged input and freezes the supplied executable, then
prints a command. Only --start launches the solver. No algebra lives in Python.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import sys
import tempfile

RAM_POLICY_OPTIONS = ("max_memory_bytes", "ram_guard_margin_percent")


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
    query_path = directory / "queries.json"
    document = json.loads(query_path.read_text())
    if document.get("schema") != "rustred.owner-domain-queries.json.v2" or not isinstance(document.get("queries"), list) or not document["queries"]:
        raise ValueError("staged query document is not a nonempty owner-domain query set")
    return len(document["queries"]), query_path.stat().st_size, receipt


def frozen_policy(campaign, args, executable, inputs, count, size):
    """Persist original steering; only per-resume supervisor RAM may differ."""
    path = campaign / "bin" / "steering.json"
    names = ("workers", "cpus", "checkpoint_interval_seconds", "max_memory_bytes",
             "ram_guard_margin_percent", "apply_subdivision_axis", "apply_subdivision_cut",
             "apply_cell_refinement_max_cardinality")
    if path.exists():
        policy = json.loads(path.read_text())
        if policy.get("schema") != "rustred.production-steering.v1":
            raise ValueError("unknown frozen steering policy")
        if args.publication_policy is not None:
            command = policy["command_arguments"]
            if command.count("--publication-policy") != 1:
                raise ValueError("frozen steering must contain exactly one --publication-policy")
            if command[command.index("--publication-policy") + 1] != args.publication_policy:
                raise ValueError("--publication-policy differs from frozen policy; use a new campaign directory")
        for name in names:
            supplied = getattr(args, name, None)
            # Older frozen policies omitted this opt-in and therefore mean Off.
            if supplied is not None and supplied != policy["options"].get(name):
                if args.resume and name in RAM_POLICY_OPTIONS:
                    continue
                raise ValueError(f"--{name.replace('_', '-')} differs from frozen policy; use a new campaign directory")
        return policy
    if args.resume:
        raise ValueError("resume requires the original frozen steering.json; refusing to guess native policy")
    options = {name: getattr(args, name, None) for name in names}
    defaults = {"workers": min(50, len(os.sched_getaffinity(0))),
                "checkpoint_interval_seconds": 3600, "max_memory_bytes": 500_000_000_000,
                "ram_guard_margin_percent": 5.0}
    for name, default in defaults.items():
        if options[name] is None:
            options[name] = default
    cpus = (set(map(int, options["cpus"].split(","))) if options["cpus"] else
            set(sorted(os.sched_getaffinity(0))[:options["workers"]]))
    if len(cpus) != options["workers"] or not cpus <= os.sched_getaffinity(0):
        raise ValueError("CPU affinity must contain exactly the requested number of permitted CPUs")
    options["cpus"] = ",".join(map(str, sorted(cpus)))
    command = ["--executable", str(executable), "--manifest", str(inputs / "selection.json"),
               "--queries", str(inputs / "queries.json"), "--owner-base", str(inputs),
               "--workers", str(options["workers"]), "--cpus", options["cpus"],
               "--max-memory-bytes", str(options["max_memory_bytes"]),
               "--ram-guard-margin-percent", str(options["ram_guard_margin_percent"]),
               "--unbounded-work", "--max-queries", str(count), "--max-query-bytes", str(size),
               "--bounded-refinement-axes", "finite-axes", "--max-guard-univariate-degree", "64",
               "--publication-policy", args.publication_policy or "ordered", "--route-domain-overcover",
               "--transfer-unreserved-lookahead", "256", "--reuse-initial-d-bands",
               "--checkpoint-interval-seconds", str(options["checkpoint_interval_seconds"])]
    if options["apply_subdivision_axis"] is not None:
        command += ["--apply-subdivision-axis", str(options["apply_subdivision_axis"]),
                    "--apply-subdivision-cut", str(options["apply_subdivision_cut"])]
    if options["apply_cell_refinement_max_cardinality"] is not None:
        command += ["--apply-cell-refinement-max-cardinality",
                    str(options["apply_cell_refinement_max_cardinality"])]
    policy = {"schema": "rustred.production-steering.v1", "options": options,
              "command_arguments": command}
    write_json(path, policy)
    path.chmod(0o444)
    return policy


def effective_supervisor_policy(policy, args):
    """Overlay resume-only RAM settings without rewriting frozen solver policy."""
    options = dict(policy["options"])
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
    parser.add_argument("--start", action="store_true", help="manually launch after preparation")
    parser.add_argument("--resume", action="store_true", help="continue the latest native checkpoint with the frozen executable")
    parser.add_argument("--workers", type=int, help="initial default: at most 50 permitted CPUs; frozen for resume")
    parser.add_argument("--cpus", help="optional explicit affinity; exactly --workers CPU IDs")
    parser.add_argument("--run-directory", type=Path)
    parser.add_argument("--publication-policy", choices=("ordered", "ready"),
                        help="initial default: ordered; ready is experimental; frozen for resume")
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
    cardinalities = args.apply_cell_refinement_max_cardinality
    if cardinalities is not None and len(cardinalities) != 1:
        parser.error("--apply-cell-refinement-max-cardinality may be supplied only once")
    args.apply_cell_refinement_max_cardinality = cardinalities[0] if cardinalities else None
    if ((args.workers is not None and not 1 <= args.workers <= 50) or
            (args.checkpoint_interval_seconds is not None and args.checkpoint_interval_seconds <= 0)):
        parser.error("workers must be in 1..50 and checkpoint interval must be positive")
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
    plan = {"command": command, "campaign_directory": str(campaign), "run_directory": str(run),
            "checkpoint_directory": str(checkpoint), "executable_sha256": executable_hash,
            "selection_sha256": receipt["selection_sha256"], "queries_sha256": receipt["queries_sha256"],
            "anchor_plan": receipt.get("anchor_plan"),
            "requested_workers": options["workers"], "hard_timeout_seconds": None,
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
