#!/usr/bin/env python3
"""Prepare selected-owner generation metadata; no native execution or algebra."""
import copy
import hashlib
import json
import os
from pathlib import Path

from generation_scheduler import schedule

SCHEMA = "rustred.selected-owner-pipeline.v1"
GRAPH_FIELDS = ("mask", "native_sector", "published_sector", "representative", "owner_to_representative")


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
    with Path(path).open("x") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def checked_file(base, row):
    path = (base / row["path"]).resolve(strict=True)
    if sha(path) != row["sha256"]:
        raise ValueError(f"frozen input changed: {path}")
    return path


def prepare(recipe_path, selection_path, directory, cli, inspector, resources):
    """Keep graph/query scope, remove old payload claims, bind native commands."""
    recipe_path, selection_path = Path(recipe_path).resolve(), Path(selection_path).resolve()
    directory = Path(directory).resolve()
    if directory.exists() or directory.is_symlink():
        raise ValueError("preparation requires a new directory; use --resume for an existing pipeline")
    if selection_path == directory or directory in selection_path.parents:
        raise ValueError("input selection cannot be inside the new output")
    recipe = read(recipe_path)
    generation_schedule = schedule(resources)
    if recipe.get("schema") != SCHEMA:
        raise ValueError("unsupported selected-owner pipeline recipe")
    if recipe.get("integral_order") != "rustred.spired-uncut-sector-order.v1":
        raise ValueError("this staging recipe currently supports the native legacy integral order only")
    if sha(selection_path) != recipe["source_selection_sha256"]:
        raise ValueError("source selection differs from the frozen recipe")
    family = checked_file(recipe_path.parent, recipe["family"])
    queries_path = checked_file(recipe_path.parent, recipe["queries"])
    original, queries = read(selection_path), read(queries_path)
    owners, routes = original["owners"], original["initial_frontier_routes"]
    masks = [row["mask"] for row in owners]
    if not masks or len(set(masks)) != len(masks) or len({len(m) for m in masks}) != 1:
        raise ValueError("owner masks must be unique with one arity")
    if any(set(m) - {"0", "1"} for m in masks):
        raise ValueError("invalid owner mask")
    if queries["queries"] != read(selection_path.parent / "queries.json")["queries"]:
        raise ValueError("query geometries/order differ from the frozen original")
    roles = queries["query_roles"]
    ids = [q["id"] for q in queries["queries"]]
    if (len(set(ids)) != len(ids) or set(roles["required"]) & set(roles["auxiliary"])
            or len(set(roles["required"])) != len(roles["required"])
            or len(set(roles["auxiliary"])) != len(roles["auxiliary"])
            or set(roles["required"]) | set(roles["auxiliary"]) != set(ids)
            or any(q["owner"] not in masks for q in queries["queries"])):
        raise ValueError("query roles must form an explicit complete partition of the frozen scope")
    counts = dict(owners=len(owners), routes=len(routes), required=len(roles["required"]),
                  auxiliary=len(roles["auxiliary"]))
    if counts != recipe["expected_counts"]:
        raise ValueError("scope counts differ from recipe")
    groups = {str(g["parent"]): g for g in recipe["groups"]}
    if len(groups) != len(recipe["groups"]):
        raise ValueError("duplicate parent")
    assigned = [m for g in groups.values() for m in g["selected_sectors"]]
    if len(assigned) != len(set(assigned)) or set(assigned) != set(masks):
        raise ValueError("selected jobs must cover every owner exactly once")
    overrides = recipe.get("missing_parent_assignments", {})
    fresh = []
    for number, owner in enumerate(owners):
        parent = owner.get("parent")
        if parent is None:
            if owner["mask"] not in overrides:
                raise ValueError(f"missing explicit parent for {owner['mask']}")
            parent = overrides[owner["mask"]]
            saved = "".join("1" if bit else "0" for bit in owner["saved_root"])
            if saved != groups[str(parent)]["root"]:
                raise ValueError("declared missing-parent recovery disagrees with original saved root")
        elif owner["mask"] in overrides:
            raise ValueError("parent override must not replace an existing parent")
        group = groups[str(parent)]
        if owner["mask"] not in group["selected_sectors"]:
            raise ValueError("owner assigned to the wrong explicit parent")
        root = group["root"]
        if len(root) != len(owner["mask"]) or set(root) - {"0", "1"} or any(
                bit == "1" and top != "1" for bit, top in zip(owner["mask"], root)):
            raise ValueError("selected owner is outside its parent")
        row = {key: copy.deepcopy(owner[key]) for key in GRAPH_FIELDS if key in owner}
        row.update(parent=parent, path=f"UNGENERATED/{number:04d}-{owner['mask']}.rrbin")
        fresh.append(row)
    binary = {}
    for name, path in (("cli", cli), ("inspector", inspector)):
        path = Path(path).resolve(strict=True)
        if not path.is_file() or not os.access(path, os.X_OK):
            raise ValueError(f"{name} must be an executable file")
        binary[name] = dict(path=str(path), sha256=sha(path))
    selection = dict(authority="Fresh metadata only: payloads UNGENERATED", family_fingerprint=original["family_fingerprint"],
                     owners=fresh, owner_count=len(fresh), initial_frontier_routes=copy.deepcopy(routes),
                     route_record_count=len(routes), load_limits=copy.deepcopy(original["load_limits"]),
                     recursive_coverage_established=False)
    reserved = {"--input", "--input-format", "--nonpositive-indices", "--selected-sectors", "--n-cores",
                "--exact-backend", "--discovery-strategy", "--checkpoint-dir", "--checkpoint-max-bytes",
                "--output", "--report-output", "--resume", "--permutation", "--integral-order", "--progress-json"}
    if any(arg.split("=", 1)[0] in reserved for arg in recipe["generation_options"]):
        raise ValueError("recipe generation_options overrides a bound identity/output option")
    directory.mkdir(parents=True)
    (directory / "shared").mkdir()
    (directory / "commands").mkdir()
    for name, source in (("family.toml", family), ("queries.json", queries_path)):
        (directory / "shared" / name).write_bytes(source.read_bytes())
    write(directory / "shared/selection.json", selection)
    write(directory / "shared/discovery.json", recipe["discovery_strategy"])
    stage = dict(schema="rustred.runtime-order-stage-plan.v1", input_format="toml",
                 integral_order=recipe["integral_order"], discovery_strategy=recipe["discovery_strategy"],
                 exact_backend=recipe["exact_backend"], solver_policy=recipe["solver_policy"], roots=[])
    for key, filename in (("family", "family.toml"), ("queries", "queries.json"), ("selection", "selection.json")):
        stage[key] = dict(path=f"shared/{filename}", sha256=sha(directory / "shared" / filename))
    for parent, group in groups.items():
        run = directory / f"generation/parent-{parent}"
        run.mkdir(parents=True)
        command = [binary["cli"]["path"], "family-candidates", "--input", str(directory / "shared/family.toml"),
                   "--input-format", "toml", "--n-cores", str(generation_schedule["workers_per_job"]),
                   "--exact-backend", recipe["exact_backend"],
                   "--selected-sectors", ",".join(sorted(group["selected_sectors"])),
                   "--checkpoint-dir", str(run / "sectors"), "--checkpoint-max-bytes", str(resources["checkpoint_max_bytes"]),
                   "--output", str(run / "candidates.rrbin"), "--report-output", str(run / "generation.toml"), "--progress"]
        inactive = [str(i) for i, bit in enumerate(group["root"]) if bit == "0"]
        if inactive:
            command += ["--nonpositive-indices", ",".join(inactive)]
        if recipe["discovery_strategy"] is not None:
            command += ["--discovery-strategy", str(directory / "shared/discovery.json")]
        if generation_schedule["jobs"] > 1:
            command += ["--progress-json", str(run / "native-progress.json")]
        command += recipe["generation_options"]
        write(directory / f"commands/parent-{parent}.json", command)
        stage["roots"].append(dict(parent=group["parent"], mask=group["root"], generation_scope="selected-sectors",
            checkpoint=f"generation/parent-{parent}/sectors/checkpoint.toml", report=f"generation/parent-{parent}/generation.toml"))
    write(directory / "stage-plan.json", stage)
    plan = dict(schema=SCHEMA, counts=counts, recipe=recipe, resources=resources, binaries=binary,
                generation_schedule=generation_schedule,
                source_selection=str(selection_path), source_selection_sha256=sha(selection_path),
                query_sha256=sha(queries_path), original_inputs_modified=False, payloads_reused=False,
                generation_checkpoint="completed sectors only; no in-sector resume", family_closure_claim=False)
    plan["bound_files"] = {str(path.relative_to(directory)): sha(path)
        for path in [directory / "stage-plan.json", *sorted((directory / "shared").iterdir()),
                     *sorted((directory / "commands").iterdir())]}
    write(directory / "pipeline.json", plan)
    return plan
