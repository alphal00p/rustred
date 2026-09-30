#!/usr/bin/env python3
"""Stage fresh v4 generation shards without interpreting native algebra.

Checks frozen study inputs and generation metadata; native loading and exact
reinspection remain mandatory. This is not a binary parser or a certificate.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import tomllib

PLAN_SCHEMA = "rustred.runtime-order-stage-plan.v1"
RECEIPT_SCHEMA = "rustred.runtime-order-stage-receipt.v1"
CHECKPOINT_VERSION = 4
CHECKPOINT_RECIPE = "ordinary-family-candidates-checkpoint-selection-v4"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path: Path):
    return json.loads(path.read_bytes(), object_pairs_hook=unique_pairs)


def mask(value, arity: int) -> str:
    require(isinstance(value, str) and len(value) == arity
            and set(value) <= {"0", "1"}, "invalid mask or arity")
    return value


def bit_mask(value, arity: int) -> str:
    require(isinstance(value, list) and len(value) == arity
            and all(type(bit) is bool for bit in value), "invalid Boolean sector")
    return "".join("1" if bit else "0" for bit in value)


def subset(child: str, parent: str) -> bool:
    return all(a == "0" or b == "1" for a, b in zip(child, parent))


def resolve(base: Path, path: str) -> Path:
    return (base / path).resolve()


def stage(plan_path: Path, output: Path) -> dict:
    """Validate every input before creating a new, non-overwriting directory."""
    plan_path = plan_path.resolve()
    base = plan_path.parent
    plan_bytes = plan_path.read_bytes()
    plan = json.loads(plan_bytes, object_pairs_hook=unique_pairs)
    plan_digest = hashlib.sha256(plan_bytes).hexdigest()
    require(plan.get("schema") == PLAN_SCHEMA, "unsupported stage-plan schema")
    require(not output.exists(), "output already exists; refusing overwrite")
    paths, frozen = {}, {}
    for name in ("family", "selection", "queries"):
        binding = plan[name]
        paths[name] = resolve(base, binding["path"])
        frozen[name] = paths[name].read_bytes()
        require(hashlib.sha256(frozen[name]).hexdigest() == binding["sha256"],
                f"frozen {name} bytes changed")
    selection = json.loads(frozen["selection"], object_pairs_hook=unique_pairs)
    queries = json.loads(frozen["queries"], object_pairs_hook=unique_pairs)
    family_source = frozen["family"].decode("utf-8")
    owners = selection["owners"]
    routes = selection["initial_frontier_routes"]
    require(bool(owners) and bool(routes), "empty owner or route census")
    arity = len(owners[0]["mask"])
    require(arity > 0, "empty coordinate set")
    owner_masks = [mask(owner["mask"], arity) for owner in owners]
    require(len(set(owner_masks)) == len(owners), "duplicate selected owner")
    require(all(type(owner.get("parent")) in (str, int) for owner in owners),
            "every selected owner requires an explicit generation parent")
    require(selection["owner_count"] == len(owners), "selected owner count differs")
    route_masks = [mask(route["source_mask"], arity) for route in routes]
    require(len(set(route_masks)) == len(routes), "duplicate route source")
    require(selection["route_record_count"] == len(routes), "route census count differs")
    require(all(mask(route["owner_mask"], arity) in owner_masks for route in routes),
            "route references an unselected owner")
    require(set(owner_masks) <= set(route_masks), "selected owner absent from route census")

    require(queries.get("schema") == "rustred.owner-domain-queries.json.v2",
            "unsupported query schema")
    query_ids = [query["id"] for query in queries["queries"]]
    require(bool(query_ids) and len(set(query_ids)) == len(query_ids), "empty/duplicate queries")
    roles = queries["query_roles"]
    required, auxiliary = roles["required"], roles["auxiliary"]
    require(len(set(required)) == len(required) and len(set(auxiliary)) == len(auxiliary)
            and not set(required) & set(auxiliary)
            and set(required) | set(auxiliary) == set(query_ids), "incomplete or overlapping roles")
    require(all(query["owner"] in owner_masks for query in queries["queries"]),
            "query references an unselected owner")
    expected_order = plan["integral_order"]
    require(isinstance(expected_order, str) and bool(expected_order), "missing expected integral_order")
    # The identity is emitted by the native compiler, never recreated in Python.
    # Explicit None is part of the experiment contract, not a missing default.
    expected_strategy = plan["discovery_strategy"]
    checkpoints = {}
    manifest_receipts = []
    for root_number, root in enumerate(plan["roots"]):
        require(type(root["parent"]) in (str, int), "invalid root parent identifier")
        key = str(root["parent"])
        require(key not in checkpoints, "duplicate generation root")
        root_mask = mask(root["mask"], arity)
        generation_scope = root.get("generation_scope", "root-downset")
        require(generation_scope in ("root-downset", "selected-sectors"),
                "unsupported generation scope")
        manifest_path = resolve(base, root["checkpoint"])
        report_path = resolve(base, root["report"])
        manifest = tomllib.loads(manifest_path.read_text())
        report = tomllib.loads(report_path.read_text())
        require(type(manifest.get("version")) is int and manifest["version"] == CHECKPOINT_VERSION
                and manifest.get("recipe") == CHECKPOINT_RECIPE, "requires native v4 checkpoint")
        require(manifest.get("integral_order") == expected_order, "mixed/missing checkpoint integral_order")
        require(manifest.get("permutation") is None, "legacy permutation is not part of this experiment")
        require(manifest.get("family_source") == family_source
                and manifest.get("input_format") == plan["input_format"]
                and manifest.get("family_fingerprint") == selection["family_fingerprint"],
                "checkpoint family binding differs")
        require(bit_mask(manifest["root_sector"], arity) == root_mask, "checkpoint root differs")
        require(manifest.get("exact_backend") == plan["exact_backend"]
                and manifest.get("solver_policy") == plan["solver_policy"]
                and manifest.get("discovery_strategy") == expected_strategy,
                "checkpoint generation recipe differs")
        sectors = [bit_mask(sector, arity) for sector in manifest["sectors"]]
        require(sectors == sorted(set(sectors)), "unordered/duplicate checkpoint sectors")
        if generation_scope == "selected-sectors":
            # This changes only which native solver jobs must have completed.
            # Routes and required/auxiliary queries remain the full frozen set.
            expected = {owner["mask"] for owner in owners if str(owner["parent"]) == key}
            require(bool(expected) and all(subset(support, root_mask) for support in expected),
                    "selected owner is outside its generation root")
            declared = [bit_mask(sector, arity) for sector in manifest.get("selected_sectors", [])]
            require(declared == sectors and set(sectors) == expected,
                    "checkpoint selected sectors differ from frozen owner jobs")
        else:
            require(manifest.get("selected_sectors") is None,
                    "full root mode cannot admit a selected-sector checkpoint")
            expected = {support for support in route_masks if subset(support, root_mask)}
            require(bool(expected) and set(sectors) == expected,
                    "checkpoint does not cover full root route census")
        for ordinal in range(len(sectors)):
            shard = manifest_path.parent / f"sector-{ordinal}.rrbin"
            require(shard.is_file() and shard.stat().st_size > 0, f"missing/empty generated shard {shard}")
        require(report.get("schema") == "rustred.family-candidates-output.toml.v1"
                and report.get("status") == "uncertified-candidates", "generation did not finish normally")
        require(report.get("integral_order") == expected_order
                and report.get("exact_backend") == plan["exact_backend"]
                and report.get("discovery_strategy") == expected_strategy
                and report.get("family_fingerprint") == selection["family_fingerprint"]
                and report.get("arity") == arity
                and bit_mask(report["root_sector"], arity) == root_mask
                and report.get("solved_sectors") == len(sectors), "generation report binding differs")
        require(report.get("generation_scope") == generation_scope,
                "generation report scope differs")
        if generation_scope == "selected-sectors":
            reported = [bit_mask(sector, arity) for sector in report.get("selected_sectors", [])]
            require(reported == sectors, "generation report selected sectors differ")
        else:
            require(report.get("selected_sectors") is None,
                    "full root report cannot declare selected sectors")
        relative = f"provenance/root-{root_number:04d}.toml"
        report_relative = f"provenance/root-{root_number:04d}-report.toml"
        checkpoints[key] = (manifest_path, {support: n for n, support in enumerate(sectors)}, relative)
        manifest_receipts.append({"parent": root["parent"], "mask": root_mask,
                                  "checkpoint": relative, "report": report_relative,
                                  "checkpoint_sha256": sha(manifest_path),
                                  "report_sha256": sha(report_path), "sectors": len(sectors),
                                  "generation_scope": generation_scope})
    require(set(checkpoints) == {str(owner["parent"]) for owner in owners},
            "generation roots differ from selected-owner parents")
    jobs = []
    for number, owner in enumerate(owners):
        manifest_path, ordinals, manifest_relative = checkpoints[str(owner["parent"])]
        require(owner["mask"] in ordinals, "selected owner missing from its parent checkpoint")
        ordinal = ordinals[owner["mask"]]
        source = manifest_path.parent / f"sector-{ordinal}.rrbin"
        jobs.append((number, owner, source, ordinal, sha(source), source.stat().st_size,
                     manifest_relative))

    output.mkdir(parents=True, exist_ok=False)
    (output / "owners").mkdir()
    (output / "provenance").mkdir()
    staged_owners = []
    for number, original, source, ordinal, digest, size, manifest_relative in jobs:
        relative = f"owners/{number:04d}-{original['mask']}.rrbin"
        shutil.copyfile(source, output / relative)
        require(sha(output / relative) == digest and (output / relative).stat().st_size == size,
                "owner changed during copy")
        owner = copy.deepcopy(original)
        owner.update(path=relative, bytes=size, sha256=digest, native_ordinal=ordinal,
                     manifest=manifest_relative)
        staged_owners.append(owner)
    for root, receipt in zip(plan["roots"], manifest_receipts):
        for name in ("checkpoint", "report"):
            shutil.copyfile(resolve(base, root[name]), output / receipt[name])
            require(sha(output / receipt[name]) == receipt[f"{name}_sha256"],
                    f"{name} changed during copy")
    shutil.copyfile(paths["queries"], output / "queries.json")
    require(sha(output / "queries.json") == plan["queries"]["sha256"], "query changed during copy")
    staged = copy.deepcopy(selection)
    staged.update(owners=staged_owners, total_bundle_bytes=sum(owner["bytes"] for owner in staged_owners),
                  authority="Fresh v4 generation metadata staged; native admission and cold verification required",
                  recursive_coverage_established=False,
                  receipts={"stage_plan_sha256": plan_digest,
                            "source_selection_sha256": plan["selection"]["sha256"]})
    require(staged["initial_frontier_routes"] == routes, "route inventory changed during staging")
    (output / "selection.json").write_text(json.dumps(staged, indent=2) + "\n")
    receipt = {"schema": RECEIPT_SCHEMA, "status": "STAGED_NATIVE_ADMISSION_AND_COLD_REQUIRED",
               "integral_order": expected_order, "discovery_strategy": expected_strategy,
               "owners": len(staged_owners), "routes": len(routes), "queries": len(query_ids),
               "required": len(required), "auxiliary": len(auxiliary), "roots": manifest_receipts,
               "query_sha256": sha(output / "queries.json"),
               "selection_sha256": sha(output / "selection.json"),
               "payloads": [{key: owner[key] for key in ("mask", "parent", "native_ordinal", "path", "bytes", "sha256")}
                            for owner in staged_owners],
               "native_payload_admission": False, "source_replay_claim": False, "closure_claim": False}
    (output / "input-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        print(json.dumps(stage(args.plan, args.output), indent=2))
    except (ValueError, KeyError, TypeError, OSError) as error:
        parser.exit(2, f"runtime-order staging rejected: {error}\n")


if __name__ == "__main__":
    main()
