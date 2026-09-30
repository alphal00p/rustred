#!/usr/bin/env python3
"""User-launched generation → selected-owner admission → guarded owner walk.

Preparation is metadata only. --start is explicit; --resume reuses only saved
completed sectors / the native walking checkpoint. No old-owner fallback.
"""
import argparse
import copy
from contextlib import contextmanager
from datetime import datetime, timezone
import fcntl
import json
import math
import os
from pathlib import Path
import shutil
import shlex
import subprocess
import sys

from generation_guard import ROOT, SUPERVISOR, module, run as guarded_run
from prepare_selected import SCHEMA, prepare, read, sha, write
import generation_scheduler
from generation_monitor import Dashboard
import stage as candidate_stage

PRODUCTION = module("selected_pipeline_production", ROOT / "examples/python/production_saved_owner_campaign.py")
SAVED_STAGE = module("selected_pipeline_saved_stage", ROOT / "examples/python/stage_saved_owner_campaign.py")


@contextmanager
def exclusive(directory):
    with (directory / "pipeline.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError("another preparation/generation process owns this pipeline") from error
        yield


def validate(directory, plan):
    if plan.get("schema") != SCHEMA:
        raise ValueError("unsupported pipeline state")
    for name, digest in plan["bound_files"].items():
        if sha(directory / name) != digest:
            raise ValueError(f"bound pipeline input changed: {name}")
    for binary in plan["binaries"].values():
        if sha(binary["path"]) != binary["sha256"]:
            raise ValueError("bound native executable changed")
    actual = generation_scheduler.schedule(plan["resources"])
    if plan.get("generation_schedule", actual) != actual:
        raise ValueError("frozen generation schedule differs from its resource budget")
    for group in plan["recipe"]["groups"]:
        command = read(directory / f"commands/parent-{group['parent']}.json")
        if command[command.index("--n-cores")+1] != str(actual["workers_per_job"]):
            raise ValueError("native generation command differs from frozen per-job worker budget")


def attempt_path(directory, name):
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
    return directory / "attempts" / f"{stamp}-{name}"


def generation_complete(run):
    receipt = run / "completed.json"
    if not receipt.exists():
        return False
    for name, digest in read(receipt)["outputs"].items():
        if sha(run / name) != digest:
            raise ValueError(f"completed native output changed: {run / name}")
    return True


def check_staged(directory):
    receipt = read(directory / "input-receipt.json")
    if sha(directory / "selection.json") != receipt["selection_sha256"] or sha(directory / "queries.json") != receipt["query_sha256"]:
        raise ValueError("staged manifest/query changed")
    for row in receipt["payloads"]:
        if sha(directory / row["path"]) != row["sha256"]:
            raise ValueError("staged owner changed")
    return receipt


def walk_options(plan):
    r = plan["resources"]
    return ["--executable", plan["binaries"]["cli"]["path"], "--workers", str(r["workers"]),
            "--cpus", r["cpus"], "--inspection-workers", str(max(1, r["workers"]-1)),
            "--max-memory-bytes", str(r["max_memory_bytes"]),
            "--host-memory-reserve-bytes", str(r["host_memory_reserve_bytes"]),
            "--ram-guard-margin-percent", str(r["ram_guard_margin_percent"]),
            "--checkpoint-interval-seconds", "3600", *plan["recipe"]["walk_options"]]


def preflight_walk(directory, plan):
    """Reuse production's policy validators before any expensive native work."""
    parser = argparse.ArgumentParser(allow_abbrev=False, exit_on_error=False)
    domain = SUPERVISOR.DOMAIN
    choices = {"publication-policy": ("ordered", "ready", "epoch"),
               "epoch-inspector-lookup": domain.EPOCH_INSPECTOR_LOOKUP_MODES,
               "epoch-dispatch": domain.EPOCH_DISPATCH_POLICIES,
               "g2-residual-anchors": SUPERVISOR.G2_RESIDUAL_MODES,
               "frontier-policy": domain.FRONTIER_POLICIES}
    for key, values in choices.items():
        parser.add_argument("--"+key, choices=values, action=domain.StoreOnce)
    parser.add_argument("--transfer-unreserved-lookahead", type=domain.positive, action=domain.StoreOnce)
    parser.add_argument("--epoch-rolling", action=domain.StoreTrueOnce, nargs=0, default=None)
    rescue = parser.add_mutually_exclusive_group()
    rescue.add_argument("--auto-rescue", dest="auto_rescue", action="store_true", default=None)
    rescue.add_argument("--no-auto-rescue", dest="auto_rescue", action="store_false")
    domain.add_epoch_batch_arguments(parser)
    domain.add_epoch_preparation_arguments(parser)
    try:
        supplied, unknown = parser.parse_known_args(plan["recipe"]["walk_options"])
    except (argparse.ArgumentError, SystemExit) as error:
        raise ValueError(f"invalid walk recipe: {error}") from error
    if unknown:
        raise ValueError(f"walk recipe cannot override resource, input or lifecycle options: {unknown}")
    values = {name: None for name in PRODUCTION.FROZEN_OPTIONS}
    values.update(vars(supplied))
    values.update({name: plan["resources"][name] for name in ("workers", "cpus", "max_memory_bytes",
                  "host_memory_reserve_bytes", "ram_guard_margin_percent")})
    values.update(inspection_workers=max(1, plan["resources"]["workers"]-1), checkpoint_interval_seconds=3600)
    preview = directory / "walk-preview"
    (preview / "bin").mkdir(parents=True, exist_ok=True)
    values["resume"] = (preview / "bin/steering.json").exists()
    PRODUCTION.frozen_policy(preview, argparse.Namespace(**values), Path(plan["binaries"]["cli"]["path"]),
        directory / "shared", sum(plan["counts"][k] for k in ("required", "auxiliary")),
        (directory / "shared/queries.json").stat().st_size)


def generation_jobs(directory, plan):
    """Completed receipts alone skip work; checkpoint files only permit resume."""
    jobs, retained = [], []
    for group in plan["recipe"]["groups"]:
        parent = str(group["parent"])
        run = directory / f"generation/parent-{parent}"
        if generation_complete(run):
            print(f"Parent {parent}: completed native output retained", flush=True)
            retained.append(dict(id=f"parent-{parent}", state="retained", metadata=group))
            continue
        command = read(directory / f"commands/parent-{parent}.json")
        if (run / "sectors/checkpoint.toml").is_file():
            # Only this pipeline's exact bound bundle/report destinations
            # may be reassembled after an interrupted final write.
            command += ["--resume", "--force"]
            print(f"Parent {parent}: resume completed sectors; unfinished sector restarts", flush=True)
        elif (run / "sectors").exists():
            raise ValueError(f"checkpoint directory has no manifest: {run / 'sectors'}; inspect rather than overwrite")
        print(f"Parent {parent}: {len(group['selected_sectors'])} selected owner jobs", flush=True)
        jobs.append(generation_scheduler.Job(f"parent-{parent}", tuple(command), group,
            native_progress=run / "native-progress.json", checkpoint=run / "sectors"))
    return jobs, retained


def complete_generation(job):
    run = Path(job.command[job.command.index("--output")+1]).parent
    outputs = ["candidates.rrbin", "generation.toml", "sectors/checkpoint.toml"]
    # Receipt publication is atomic: an interrupted hash/write does not turn a
    # restartable completed-sector checkpoint into an unusable partial marker.
    temporary = run / ".completed.json.part"
    payload = dict(outputs={name: sha(run / name) for name in outputs},
        source="successful native invocation; strict selected scope validated during staging")
    temporary.write_text(json.dumps(payload, indent=2) + "\n")
    os.replace(temporary, run / "completed.json")


def run_generations(directory, plan):
    jobs, retained = generation_jobs(directory, plan)
    if not jobs:
        return
    allocation = generation_scheduler.schedule(plan["resources"])
    if allocation["jobs"] == 1:
        # Retain the existing default lifecycle/command surface. The opt-in
        # multi-parent path never runs these per-child RAM guards concurrently.
        for job in jobs:
            guarded_run(list(job.command), attempt_path(directory, "generate-"+job.id), plan["resources"])
            complete_generation(job)
        return
    checked = subprocess.run([plan["binaries"]["cli"]["path"], "--help"], check=False,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=10,
        env=generation_scheduler.native_environment())
    if checked.returncode != 0 or "--progress-json" not in checked.stdout + checked.stderr:
        raise ValueError("parallel generation requires a native build advertising --progress-json; no generation started")
    attempt = attempt_path(directory, "generate-parallel")
    print(f"Parallel generation: {allocation['jobs']} slots x {allocation['workers_per_job']} workers; "
          f"one aggregate RAM guard; snapshots/events in {attempt}", flush=True)
    enabled = plan.get("presentation", {}).get("generation_dashboard", True)
    with Dashboard(plan["binaries"]["cli"]["path"], attempt, plan["resources"], enabled) as dashboard:
        generation_scheduler.run(jobs, attempt, plan["resources"], complete_generation,
                                 retained=retained, observer=dashboard.observe)


def execute(directory, plan):
    validate(directory, plan)
    with exclusive(directory):
        preflight_walk(directory, plan)
        run_generations(directory, plan)
        # Every original parent must be present; racing completions never alter
        # input order, roles, routes or the deterministic staging barrier.
        if any(not generation_complete(directory / f"generation/parent-{group['parent']}")
               for group in plan["recipe"]["groups"]):
            raise ValueError("generation barrier has a missing completed parent")
        staged = directory / "generated-inputs"
        if not staged.exists():
            temporary = attempt_path(directory, "partial-candidate-staging")
            temporary.parent.mkdir(parents=True, exist_ok=True)
            candidate_stage.stage(directory / "stage-plan.json", temporary)
            temporary.rename(staged)
        receipt = check_staged(staged)
        if {key: receipt[key] for key in plan["counts"]} != plan["counts"]:
            raise ValueError("staged scope counts changed")
        admission = directory / "native-admitted.json"
        if not admission.exists():
            command = [plan["binaries"]["inspector"]["path"], "--selection", str(staged / "selection.json"),
                       "--owner-base", str(staged), "--expected-order", plan["recipe"]["integral_order"]]
            attempt = attempt_path(directory, "admit")
            guarded_run(command, attempt, plan["resources"])
            result = read(attempt / "stdout")
            if result.get("status") != "NATIVE_OWNER_BINDING_ADMITTED":
                raise ValueError("native inspector did not admit the owner bindings")
            write(admission, dict(result=result, staged_receipt_sha256=sha(staged / "input-receipt.json"),
                                  inspector_sha256=plan["binaries"]["inspector"]["sha256"]))
        elif read(admission)["staged_receipt_sha256"] != sha(staged / "input-receipt.json"):
            raise ValueError("native admission is not bound to this staged input")
        inputs = directory / "inputs"
        if not inputs.exists():
            temporary = attempt_path(directory, "partial-input-staging")
            temporary.parent.mkdir(parents=True, exist_ok=True)
            SAVED_STAGE.stage(staged / "selection.json", staged / "queries.json", temporary, staged,
                              query_order="preserve")
            shutil.copytree(staged / "provenance", temporary / "provenance")
            PRODUCTION.verify_inputs(temporary)
            temporary.rename(inputs)
        PRODUCTION.verify_inputs(inputs)
        # Freeze through the existing public launcher, including the entire
        # Epoch policy. This prints its exact JSON plan but never starts yet.
        if not (directory / "bin/steering.json").exists():
            PRODUCTION.main(["--campaign-directory", str(directory), "--json", *walk_options(plan)])
    # The public supervisor owns subsequent walking, RAM stops and checkpoints.
    # Native checkpoint locks also reject concurrent resumes of the same walk.
    restart = (directory / "checkpoints/main/latest.json").exists()
    command = [sys.executable, str(ROOT / "examples/python/production_saved_owner_campaign.py"),
               "--campaign-directory", str(directory), "--start"]
    if restart:
        command.append("--resume")
    # Always overlay the resolved invocation policy. A temporary higher limit
    # used before the first walk freeze must not become a permanent default
    # for later plain pipeline resumes. The production launcher changes no
    # saved mathematical/scheduling identity for these three guard controls.
    for name in ("max_memory_bytes", "host_memory_reserve_bytes", "ram_guard_margin_percent"):
        command += ["--" + name.replace("_", "-"), str(plan["resources"][name])]
    print("All selected owners admitted; transferring to the existing campaign dashboard.", flush=True)
    os.execv(command[0], command)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--recipe", type=Path)
    parser.add_argument("--selection", type=Path)
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--inspector", type=Path)
    parser.add_argument("--workers", type=int)
    parser.add_argument("--generation-jobs", type=int,
        help="opt-in independent parent slots; divides --workers exactly (default: 1)")
    parser.add_argument("--no-generation-dashboard", action="store_true",
        help="disable only the optional read-only viewer for parallel generation (also valid on resume)")
    parser.add_argument("--cpus")
    parser.add_argument("--max-memory-bytes", type=int)
    parser.add_argument("--host-memory-reserve-bytes", type=int)
    parser.add_argument("--checkpoint-max-bytes", type=int)
    parser.add_argument("--ram-guard-margin-percent", type=float)
    parser.add_argument("--start", action="store_true")
    parser.add_argument("--resume", action="store_true")
    args = parser.parse_args(argv)
    directory = args.directory.resolve()
    try:
        settings = ("recipe", "selection", "executable", "inspector", "workers", "generation_jobs", "cpus", "max_memory_bytes",
                    "host_memory_reserve_bytes", "checkpoint_max_bytes", "ram_guard_margin_percent")
        if args.resume:
            ram_names = {"max_memory_bytes", "host_memory_reserve_bytes", "ram_guard_margin_percent"}
            if any(getattr(args, key) is not None for key in settings if key not in ram_names):
                raise ValueError("--resume freezes source/workers/CPUs; only RAM policy may be overridden")
            plan = read(directory / "pipeline.json")
            validate(directory, plan)
            plan = copy.deepcopy(plan)
            plan["ram_overrides"] = {name: getattr(args, name) for name in ram_names if getattr(args, name) is not None}
            for name, value in plan["ram_overrides"].items():
                if (not math.isfinite(value) or value <= 0
                        or name == "ram_guard_margin_percent" and value >= 100):
                    raise ValueError("RAM limits must be positive and margin strictly between zero and 100")
                plan["resources"][name] = value
        else:
            if any(getattr(args, key) is None for key in settings if key not in ("ram_guard_margin_percent", "generation_jobs")):
                raise ValueError("fresh preparation requires recipe, selection, executables and all resource arguments")
            margin = 5.0 if args.ram_guard_margin_percent is None else args.ram_guard_margin_percent
            if not math.isfinite(margin) or not 0 < margin < 100:
                raise ValueError("RAM margin must be finite and strictly between zero and 100")
            for key in ("workers", "max_memory_bytes", "host_memory_reserve_bytes", "checkpoint_max_bytes"):
                if getattr(args, key) <= 0:
                    raise ValueError(f"{key} must be positive")
            cpus = SUPERVISOR.parse_cpu_set(args.cpus)
            if len(cpus) != args.workers or not cpus <= os.sched_getaffinity(0):
                raise ValueError("provide exactly workers available CPU IDs")
            resources = {key: getattr(args, key) for key in ("workers", "cpus", "max_memory_bytes",
                         "host_memory_reserve_bytes", "checkpoint_max_bytes")}
            resources["ram_guard_margin_percent"] = margin
            resources["generation_jobs"] = 1 if args.generation_jobs is None else args.generation_jobs
            plan = prepare(args.recipe, args.selection, directory, args.executable, args.inspector, resources)
            preflight_walk(directory, plan)
        # Presentation is per invocation, not native policy/checkpoint identity.
        plan["presentation"] = dict(generation_dashboard=not args.no_generation_dashboard)
        if args.start:
            execute(directory, plan)
        else:
            print(json.dumps(dict(status="METADATA_ONLY_NO_NATIVE_INVOCATION", counts=plan["counts"],
                command=[sys.executable, str(Path(__file__).resolve()), "--directory", str(directory), "--resume", "--start"],
                payloads="UNGENERATED until all native phases complete", family_closure_claim=False), indent=2))
        return 0
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, KeyboardInterrupt) as error:
        restart = shlex.join([sys.executable, str(Path(__file__).resolve()), "--directory", str(directory), "--resume", "--start"])
        parser.exit(130 if isinstance(error, KeyboardInterrupt) else 2,
                    f"selected-owner pipeline stopped: {error}\nCompleted native checkpoints remain in {directory}.\n"
                    f"After resolving the cause, restart with: {restart}\n")


if __name__ == "__main__":
    raise SystemExit(main())
