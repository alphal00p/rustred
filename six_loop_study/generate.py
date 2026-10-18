#!/usr/bin/env python3
"""Prepare/run independent native candidate generation; never run a closure walk.

This study adapter does no algebra or topology classification. Native RustRed
owns generation, serialization and completed-sector checkpoint validation.
"""
import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import shlex
import shutil
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/research/runtime_order"))
import generation_scheduler as scheduler
import shared_generation_scheduler as shared_scheduler
from generation_monitor import Dashboard

SCHEMA = "rustred.independent-generation-study.v1"
TOPOLOGY_SCHEMA = "six-loop-study.topologies.v1"
USIZE_MAX = (1 << 64) - 1
DEFAULT_DISCOVERY = {
    "version": 1,
    "sectors": {"kind": "active-first"},
    "rows": {"kind": "features", "priorities": [
        {"feature": {"kind": "terms"}, "descending": False},
        {"feature": {"kind": "coefficient-monomials"}, "descending": False},
    ]},
}


def active_sector_limit(value):
    if value == "all":
        return "all"
    try:
        number = int(value)
    except (ValueError, TypeError):
        raise argparse.ArgumentTypeError("active-sector limit must be a positive integer or 'all'") from None
    if number <= 0:
        raise argparse.ArgumentTypeError("active-sector limit must be positive or 'all'")
    return number


def resume_resources(plan, parent_slots=None, sector_limit=None):
    if parent_slots is None and sector_limit is None:
        return plan
    if plan.get("execution_mode") != "shared":
        raise ValueError("runtime parent/sector overrides require shared execution")
    # These grants do not enter the native sector checkpoint identity. Keep
    # the original frozen plan intact and record actual grants in each attempt.
    plan = dict(plan, resources=dict(plan["resources"]))
    if parent_slots is not None:
        plan["resources"]["generation_jobs"] = parent_slots
    if sector_limit is not None:
        plan["resources"]["max_active_sectors"] = None if sector_limit == "all" else sector_limit
    plan["allocation"] = shared_scheduler.schedule(plan["resources"])
    return plan


def read(path):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key: {key}")
            result[key] = value
        return result
    return json.loads(Path(path).read_bytes(), object_pairs_hook=unique)


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write(path, value):
    """Atomic small metadata publication; no generated output is overwritten."""
    path = Path(path)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", dir=path.parent,
                prefix=f".{path.name}.", suffix=".part", delete=False) as stream:
            temporary = Path(stream.name)
            json.dump(value, stream, indent=2, allow_nan=False)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def local_file(base, name):
    path = (base / name).resolve(strict=True)
    if Path(name).is_absolute() or base.resolve() not in path.parents or not path.is_file():
        raise ValueError(f"manifest path must name a regular file within its directory: {name}")
    return path


def manifest_entries(path):
    path = Path(path).resolve(strict=True)
    value = read(path)
    if value.get("schema") != TOPOLOGY_SCHEMA:
        raise ValueError("unsupported topology manifest")
    entries = value.get("entries")
    if not isinstance(entries, list) or not entries:
        raise ValueError("topology manifest has no generation entries")
    ids = set()
    checked = []
    for entry in entries:
        key = entry["id"]
        if (not isinstance(key, str) or not key or Path(key).name != key
                or key in (".", "..") or key in ids):
            raise ValueError("generation IDs must be unique filename components")
        ids.add(key)
        loops = entry["loop_count"]
        if type(loops) is not int or loops <= 0:
            raise ValueError("loop_count must be positive")
        arity = loops * (loops + 1) // 2
        physical, auxiliary = entry["physical_propagators"], entry["auxiliary_slots"]
        if any(not isinstance(group, list) for group in (physical, auxiliary)):
            raise ValueError("physical_propagators and auxiliary_slots must be coordinate lists")
        coordinates = physical + auxiliary
        if (not physical or any(type(axis) is not int for axis in coordinates)
                or len(set(coordinates)) != len(coordinates)
                or set(coordinates) != set(range(arity))):
            raise ValueError("physical/auxiliary slots must exactly partition the scalar-product basis")
        selected = entry.get("selected_sectors")
        if selected is not None:
            if (not isinstance(selected, list) or not selected
                    or any(not isinstance(mask, str) or len(mask) != arity
                        or set(mask) - {"0", "1"} or "1" not in mask
                        or any(mask[axis] == "1" for axis in auxiliary) for mask in selected)
                    or len(set(selected)) != len(selected)):
                raise ValueError("selected_sectors must be unique nonzero masks inside the physical root")
        family = local_file(path.parent, entry["input"])
        graph = local_file(path.parent, entry["dot"])
        # The generator proves the graph/routing correspondence; this cheap
        # independent shape check prevents an accidentally wrong file pairing.
        parsed = tomllib.loads(family.read_text())
        definition = parsed.get("family", {})
        if (len(definition.get("loop_momenta", [])) != loops
                or definition.get("external_momenta", [])
                or len(definition.get("denominators", [])) != arity):
            raise ValueError(f"family dimension disagrees with manifest: {key}")
        checked.append({**entry, "arity": arity,
                        "family_source": family, "graph_source": graph})
    return value, checked


@contextmanager
def exclusive(directory):
    with (directory / "generation.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise ValueError("another process owns this generation study") from error
        yield


def prepare(args):
    directory = args.directory.resolve()
    if directory.exists():
        raise ValueError("output already exists; use --resume, or choose a new directory")
    if args.manifest is None or args.executable is None:
        raise ValueError("first preparation requires --manifest and --executable")
    if args.workers is None or args.cpus is None or args.max_memory_bytes is None:
        raise ValueError("first preparation requires --workers, --cpus and --max-memory-bytes")
    margin = 5 if args.ram_margin_percent is None else args.ram_margin_percent
    rank = 0 if args.rank is None else args.rank
    depth = 0 if args.numerical_depth is None else args.numerical_depth
    if (args.max_memory_bytes <= 0 or not math.isfinite(margin)
            or not 0 < margin < 100):
        raise ValueError("RAM ceiling must be positive; margin must be between 0 and 100 percent")
    if any(value < 0 or value > (1 << 32) - 1 for value in (rank, depth)):
        raise ValueError("rank and numerical depth must fit unsigned 32-bit integers")
    original, entries = manifest_entries(args.manifest)
    execution_mode = getattr(args, "execution_mode", None) or "shared"
    max_active = getattr(args, "max_active_sectors", None)
    if max_active == "all":
        max_active = None
    if execution_mode == "isolated" and max_active is not None:
        raise ValueError("--max-active-sectors applies to the shared pool")
    resources = dict(workers=args.workers, cpus=args.cpus,
        generation_jobs=(len(entries) if execution_mode == "shared" else 1) if args.jobs is None else args.jobs,
        max_memory_bytes=args.max_memory_bytes, memory_policy="process-limit",
        host_memory_reserve_bytes=0, ram_guard_margin_percent=margin,
        swap_growth_stop_bytes_per_second=0)
    if execution_mode == "shared":
        resources.update(max_active_sectors=max_active)
    allocation = (shared_scheduler if execution_mode == "shared" else scheduler).schedule(resources)
    executable = args.executable.resolve(strict=True)
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ValueError("--executable must be an executable regular file")
    strategy = read(args.discovery_strategy) if args.discovery_strategy else DEFAULT_DISCOVERY
    # Native payload permissions track user RAM, not arbitrary history counts.
    # They do not preallocate this amount or replace the aggregate RSS guard.
    payload_bytes = int(args.max_memory_bytes * (1 - margin / 100))
    if payload_bytes <= 0 or payload_bytes > USIZE_MAX:
        raise ValueError("RAM allowance is outside native 64-bit byte representation")
    directory.mkdir(parents=True)
    (directory / "inputs").mkdir()
    (directory / "inputs/topology").mkdir()
    (directory / "bin").mkdir()
    shutil.copy2(executable, directory / "bin/rustred")
    write(directory / "inputs/topology/manifest.json", original)
    write(directory / "inputs/discovery.json", strategy)
    bound = {"bin/rustred": sha(directory / "bin/rustred"),
             "inputs/topology/manifest.json": sha(directory / "inputs/topology/manifest.json"),
             "inputs/discovery.json": sha(directory / "inputs/discovery.json")}
    owners = []
    for entry in entries:
        key = entry["id"]
        family, graph = f"inputs/topology/{entry['input']}", f"inputs/topology/{entry['dot']}"
        (directory / family).parent.mkdir(parents=True, exist_ok=True)
        (directory / graph).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(entry["family_source"], directory / family)
        shutil.copyfile(entry["graph_source"], directory / graph)
        bound[family], bound[graph] = sha(directory / family), sha(directory / graph)
        (directory / "generation" / key).mkdir(parents=True)
        owners.append(dict(id=key, input=family, dot=graph, loop_count=entry["loop_count"],
            arity=entry["arity"], physical_propagators=entry["physical_propagators"],
            auxiliary_slots=entry["auxiliary_slots"],
            selected_sectors=None if args.full_downsets else entry.get("selected_sectors")))
    for field in ("topology_census", "coverage_witnesses", "contraction_census"):
        name = original.get(field)
        if name is not None:
            source = local_file(args.manifest.resolve().parent, name)
            target = directory / "inputs/topology" / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            bound[str(target.relative_to(directory))] = sha(target)
    plan = dict(schema=SCHEMA, rank=rank, finite_case_policy=args.finite_case_policy or "search",
        numerical_depth=depth, exact_backend=args.exact_backend or "sparse-factorized",
        resources=resources, allocation=allocation, payload_bytes=payload_bytes,
        bound_files=bound, owners=owners, family_closure_claim=False,
        checkpoint="completed sectors only; unfinished sectors restart",
        generation_scope="manifest-selected representatives unless a full downset is requested",
        full_downsets=args.full_downsets, root_zero_census=True,
        cross_parent_rule_reuse=False, execution_mode=execution_mode)
    write(directory / "generation.json", plan)
    return plan


def validate(directory, plan):
    if plan.get("schema") != SCHEMA:
        raise ValueError("unsupported generation study plan")
    for name, expected in plan["bound_files"].items():
        if sha(local_file(directory, name)) != expected:
            raise ValueError(f"frozen input or executable changed: {name}")
    engine = shared_scheduler if plan.get("execution_mode") == "shared" else scheduler
    if engine.schedule(plan["resources"]) != plan["allocation"]:
        raise ValueError("frozen CPU allocation changed")


def command(directory, plan, owner):
    run = directory / "generation" / owner["id"]
    values = [str(directory / "bin/rustred"), "family-candidates",
        "--input", str(directory / owner["input"]), "--input-format", "toml",
        "--output", str(run / "candidates.rrbin"),
        "--report-output", str(run / "generation.toml"),
        "--checkpoint-dir", str(run / "sectors"), "--checkpoint-max-bytes", str(USIZE_MAX),
        "--progress", "--progress-json", str(run / "native-progress.json"),
        "--n-cores", str(plan["resources"]["workers"] if plan.get("execution_mode") == "shared" else plan["allocation"]["workers_per_job"]),
        "--max-numerator-rank", str(plan["rank"]),
        "--finite-case-policy", plan["finite_case_policy"],
        "--numerical-depth", str(plan["numerical_depth"]),
        "--exact-backend", plan["exact_backend"],
        "--discovery-strategy", str(directory / "inputs/discovery.json")]
    if plan["root_zero_census"]:
        values.append("--root-zero-census")
    if owner["auxiliary_slots"]:
        values += ["--nonpositive-indices", ",".join(map(str, owner["auxiliary_slots"]))]
    if owner.get("selected_sectors") is not None:
        values += ["--selected-sectors", ",".join(owner["selected_sectors"])]
    count_flags = ["--case-max-work-items", "--case-max-terms-per-conjunction",
        "--case-max-normalizations", "--case-max-factorizations", "--bundle-max-entries",
        "--bundle-max-polynomial-terms", "--bundle-max-term-operations"]
    if plan["finite_case_policy"] == "retain-rank-finite":
        count_flags += ["--finite-max-visited-points", "--finite-max-retained-terminals"]
    for flag in count_flags:
        values += [flag, str(USIZE_MAX)]
    for flag in ("--bundle-max-bytes", "--bundle-max-coefficient-bytes",
                 "--bundle-max-total-coefficient-bytes"):
        values += [flag, str(plan["payload_bytes"])]
    if (run / "sectors/checkpoint.toml").is_file():
        values += ["--resume", "--force"]
    elif (run / "sectors").exists():
        raise ValueError(f"incomplete checkpoint setup at {run / 'sectors'}; inspect before retrying")
    return values


def checked_report(directory, plan, owner):
    run = directory / "generation" / owner["id"]
    report = tomllib.loads((run / "generation.toml").read_text())
    root = [axis in owner["physical_propagators"] for axis in range(owner["arity"])]
    if (report.get("schema") != "rustred.family-candidates-output.toml.v1"
            or report.get("status") != "uncertified-candidates"
            or report.get("root_sector") != root
            or report.get("arity") != owner["arity"]
            or report.get("max_numerator_rank") != plan["rank"]
            or report.get("generation_scope") != ("root-downset" if owner.get("selected_sectors") is None
                                                    else "selected-sectors")
            or report.get("numerical_depth") != plan["numerical_depth"]
            or report.get("exact_backend") != plan["exact_backend"]
            or report.get("finite_case_policy") != plan["finite_case_policy"]
            or report.get("discovery_strategy") != read(directory / "inputs/discovery.json")):
        raise ValueError(f"native report differs from requested generation scope: {owner['id']}")
    def masks(rows):
        if rows is None:
            return None
        if (not isinstance(rows, list) or any(not isinstance(row, list) or len(row) != owner["arity"]
                or any(type(bit) is not bool for bit in row) for row in rows)):
            raise ValueError("native selection has an invalid mask shape")
        return sorted("".join("1" if bit else "0" for bit in row) for row in rows)
    selected = owner.get("selected_sectors")
    expected_selection = None if selected is None else sorted(selected)
    if masks(report.get("selected_sectors")) != expected_selection:
        raise ValueError("native report selected-sector scope differs from frozen input")
    checkpoint = tomllib.loads((run / "sectors/checkpoint.toml").read_text())
    if (checkpoint.get("root_zero_census", False) != plan["root_zero_census"]
            or checkpoint.get("root_sector") != root
            or checkpoint.get("family_source") != (directory / owner["input"]).read_text()
            or checkpoint.get("family_fingerprint") != report.get("family_fingerprint")
            or masks(checkpoint.get("selected_sectors")) != expected_selection):
        raise ValueError(f"checkpoint provenance differs from frozen input: {owner['id']}")
    return report


def completed(directory, plan, owner):
    run = directory / "generation" / owner["id"]
    receipt = run / "completed.json"
    if not receipt.exists():
        return None
    value = read(receipt)
    if (value.get("schema") != "rustred.generation-receipt.v1"
            or set(value.get("outputs", {})) != {
                "candidates.rrbin", "generation.toml", "sectors/checkpoint.toml"}):
        raise ValueError(f"invalid completed-output receipt: {receipt}")
    for name, expected in value["outputs"].items():
        if sha(local_file(run, name)) != expected:
            raise ValueError(f"completed output changed: {run / name}")
    report = checked_report(directory, plan, owner)
    if any(value.get(key) != report[key] for key in ("generated_rules", "finite_residuals", "solved_sectors")):
        raise ValueError(f"completed counts differ from native report: {receipt}")
    return value


def record_completed(directory, plan, owner):
    run = directory / "generation" / owner["id"]
    report = checked_report(directory, plan, owner)
    outputs = ("candidates.rrbin", "generation.toml", "sectors/checkpoint.toml")
    value = dict(schema="rustred.generation-receipt.v1",
        outputs={name: sha(run / name) for name in outputs},
        generated_rules=report["generated_rules"], finite_residuals=report["finite_residuals"],
        solved_sectors=report["solved_sectors"], total_seconds=report["total_us"] / 1e6,
        authority="successful native generation; no independent source replay or closure proof")
    write(run / "completed.json", value)
    return value


def execute(directory, plan, dashboard=True):
    validate(directory, plan)
    with exclusive(directory):
        jobs, retained = [], []
        owners_by_id = {owner["id"]: owner for owner in plan["owners"]}
        for ordinal, owner in enumerate(plan["owners"], 1):
            metadata = dict(parent=ordinal, owner_id=owner["id"],
                root="".join("1" if axis in owner["physical_propagators"] else "0"
                             for axis in range(owner["arity"])),
                selected_sectors=owner.get("selected_sectors") or [],
                generation_scope="root-downset" if owner.get("selected_sectors") is None else "selected-sectors")
            if completed(directory, plan, owner) is not None:
                retained.append(dict(id=owner["id"], state="retained", metadata=metadata))
                continue
            run = directory / "generation" / owner["id"]
            jobs.append(scheduler.Job(owner["id"], tuple(command(directory, plan, owner)), metadata,
                native_progress=run / "native-progress.json", checkpoint=run / "sectors"))
        if jobs:
            stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S.%fZ")
            attempt = directory / "attempts" / stamp
            shared = plan.get("execution_mode") == "shared"
            layout = (f"one shared {plan['resources']['workers']}-worker pool; "
                f"active-sector limit {plan['resources'].get('max_active_sectors') or 'all'}" if shared else
                f"{plan['allocation']['jobs']} slots × {plan['allocation']['workers_per_job']} workers")
            print(f"Generation: {len(jobs)} pending parents; {layout}.\n"
                  f"Live snapshots/events: {attempt}\n"
                  "Ctrl+C preserves completed sectors; an active sector restarts on resume.", flush=True)
            full = any(owner.get("selected_sectors") is None for owner in plan["owners"])
            if full and dashboard:
                print("Full-downset job counts are determined by native zero-sector preparation; "
                      "using plain generation progress instead of a misleading aggregate sector denominator. "
                      "Structured native progress is retained.", flush=True)
            dashboard = dashboard and not full
            with Dashboard(directory / "bin/rustred", attempt, plan["resources"], dashboard) as view:
                last_plain = -30.0
                def observe(event):
                    nonlocal last_plain
                    view.observe(event)
                    if dashboard:
                        return
                    snapshot = event["snapshot"]
                    elapsed = snapshot["elapsed_seconds"]
                    if event["event"] == "sample" and elapsed-last_plain < 30:
                        return
                    last_plain = elapsed
                    aggregate = snapshot.get("aggregate") or {}
                    cores, rss = aggregate.get("observed_cores"), aggregate.get("sampled_rss_bytes")
                    print(f"Generation {elapsed:.0f}s · {snapshot['state']} · "
                          f"cores {'sampling' if cores is None else f'{cores:.2f}'} / {plan['resources']['workers']} · "
                          f"RSS {'sampling' if rss is None else f'{rss/1e9:.2f} GB'}", flush=True)
                    for row in snapshot["jobs"]:
                        if row["state"] not in ("running", "failed", "interrupted"):
                            continue
                        native = (row.get("native_progress") or {}).get("snapshot") or {}
                        counts = native.get("counts") or {}
                        print(f"  {row['id']}: {row['state']} · "
                              f"generated {counts.get('generated', '?')} sectors · "
                              f"checkpoint files {row.get('checkpoint_files_observed', '?')}", flush=True)
                complete = lambda job: record_completed(directory, plan, owners_by_id[job.id])
                if shared:
                    shared_scheduler.run(jobs, attempt, plan["resources"], directory / "bin/rustred",
                        complete, retained=retained, observer=observe)
                else:
                    scheduler.run(jobs, attempt, plan["resources"], complete,
                        retained=retained, observer=observe)
        reports = {owner["id"]: completed(directory, plan, owner) for owner in plan["owners"]}
        if any(value is None for value in reports.values()):
            raise ValueError("generation returned without every parent completed")
        summary = dict(schema=SCHEMA, status="generated-uncertified-candidates", rank=plan["rank"],
            parents=len(reports), generated_rules=sum(row["generated_rules"] for row in reports.values()),
            finite_residuals=sum(row["finite_residuals"] for row in reports.values()),
            solved_sectors=sum(row["solved_sectors"] for row in reports.values()),
            parents_report=reports, family_closure_claim=False,
            master_independence_claim=False, cross_parent_deduplicated=False)
        write(directory / "summary.json", summary)
        print(json.dumps(summary, indent=2), flush=True)
        return summary


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--workers", type=int)
    parser.add_argument("--cpus")
    parser.add_argument("--jobs", type=int, help="concurrent parent coordinators [shared default: all parents]")
    parser.add_argument("--execution-mode", choices=("shared", "isolated"), help="one shared pool or separate fixed pools [default: shared]")
    parser.add_argument("--max-active-sectors", type=active_sector_limit, metavar="N|all",
                        help="global admitted-sector limit for RAM control; may override on resume [default: all]")
    parser.add_argument("--max-memory-bytes", type=int)
    parser.add_argument("--ram-margin-percent", type=float, help="RAM stop margin [default: 5]")
    parser.add_argument("--rank", type=int, help="entry negative-index degree [default: 0]")
    parser.add_argument("--numerical-depth", type=int, help="finite-case search depth [default: 0]")
    parser.add_argument("--finite-case-policy", choices=("search", "retain-rank-finite"),
                        help="finite-case treatment [default: search]")
    parser.add_argument("--exact-backend", choices=("sparse", "sparse-factorized",
        "sparse-target-factorized", "semi-numerical"), help="exact backend [default: sparse-factorized]")
    parser.add_argument("--discovery-strategy", type=Path)
    parser.add_argument("--full-downsets", action="store_true",
                        help="diagnostic: ignore representative selection and generate every parent subsector")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--start", action="store_true", help="explicitly launch native generation")
    parser.add_argument("--no-dashboard", action="store_true")
    args = parser.parse_args(argv)
    directory = args.directory.resolve()
    restart = shlex.join([sys.executable, "-B", str(Path(__file__).resolve()),
                         "--directory", str(directory), "--resume", "--start"])
    try:
        if args.resume:
            if args.full_downsets or any(value is not None for value in (args.manifest, args.executable, args.workers,
                    args.cpus, args.max_memory_bytes, args.discovery_strategy, args.rank,
                    args.numerical_depth, args.finite_case_policy, args.exact_backend,
                    args.ram_margin_percent, args.execution_mode)):
                raise ValueError("resume uses frozen inputs/resources; omit preparation options")
            plan = read(directory / "generation.json")
            validate(directory, plan)
            plan = resume_resources(plan, args.jobs, args.max_active_sectors)
        else:
            plan = prepare(args)
        if not args.start:
            print(json.dumps(dict(status="PREPARED_NO_NATIVE_GENERATION", parents=len(plan["owners"]),
                rank=plan["rank"], allocation=plan["allocation"], resume_command=restart,
                selected_sector_jobs=(sum(len(owner["selected_sectors"]) for owner in plan["owners"])
                    if all(owner.get("selected_sectors") is not None for owner in plan["owners"]) else None),
                family_closure_claim=False), indent=2))
            return 0
        execute(directory, plan, dashboard=not args.no_dashboard)
        return 0
    except (OSError, ValueError, RuntimeError, KeyError) as error:
        print(f"Generation stopped: {error}\nCompleted sectors remain in {directory}.\n"
              f"Resume the same rank with:\n{restart}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
