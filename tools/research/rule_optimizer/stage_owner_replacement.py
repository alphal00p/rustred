#!/usr/bin/env python3
"""Stage a checked owner/overlay replacement without loading or launching native code.

The source campaign stays read-only. All queries (including roles), routes,
other owners, overlays and attachments are preserved. Payload hashes are input
identity checks, not mathematical validation: use native export/rebind receipts
and cold controls to justify the supplied replacement. By default print a plan;
--stage creates only fresh portable inputs, never a binary, steering or process.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


EXAMPLES = Path(__file__).resolve().parents[3] / "examples" / "python"
PRODUCTION = module("replacement_production", EXAMPLES / "production_saved_owner_campaign.py")
STAGER = module("replacement_stager", EXAMPLES / "stage_saved_owner_campaign.py")


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def bound_file(path, expected=None):
    path = Path(path).resolve(strict=True)
    if not path.is_file():
        raise ValueError(f"not a regular file: {path}")
    identity = {"path": str(path), "bytes": path.stat().st_size, "sha256": digest(path)}
    if expected is not None and identity["sha256"] != expected:
        raise ValueError(f"SHA256 mismatch: {path}")
    return identity


def prepare(source_campaign, destination, owner_mask, owner_payload, owner_sha256,
            overlay_payload, overlay_sha256, selection_sha256, queries_sha256,
            *, evidence=(), execute=False):
    source = Path(source_campaign).resolve(strict=True)
    destination_path = Path(destination)
    if destination_path.is_symlink():
        raise ValueError("destination campaign must not exist")
    destination = destination_path.resolve()
    if (source == destination or source in destination.parents
            or destination in source.parents):
        raise ValueError("source and destination must be disjoint campaigns")
    if destination.exists():
        raise ValueError("destination campaign must not exist")
    inputs = source / "inputs"
    manifest = bound_file(inputs / "selection.json", selection_sha256)
    queries = bound_file(inputs / "queries.json", queries_sha256)
    count, _, original_receipt = PRODUCTION.verify_inputs(inputs)
    selection = json.loads((inputs / "selection.json").read_bytes(),
                           object_pairs_hook=STAGER.unique_object)
    document = json.loads((inputs / "queries.json").read_bytes(),
                          object_pairs_hook=STAGER.unique_object)
    roles = STAGER.ROLES.query_roles(document, require_explicit=True)
    for declared, retained in zip(selection["owners"], original_receipt["owners"], strict=True):
        if any(declared.get(key) != retained[key] for key in ("mask", "path", "bytes")):
            raise ValueError("source owner inventory differs from its receipt")
        if declared.get("sha256", retained["sha256"]) != retained["sha256"]:
            raise ValueError("source owner hash differs from its receipt")
    owners = [i for i, row in enumerate(selection["owners"]) if row["mask"] == owner_mask]
    overlays = [i for i, row in enumerate(selection.get("domain_rule_overlays", []))
                if row["owner_mask"] == owner_mask]
    if len(owners) != 1 or len(overlays) != 1:
        raise ValueError("replacement requires exactly one selected owner and same-owner overlay")
    if any(row["owner_mask"] == owner_mask
           for row in selection.get("preferred_owner_programs", [])):
        raise ValueError("same-owner preferred program needs a separate checked rebind")
    owner = bound_file(owner_payload, owner_sha256)
    overlay = bound_file(overlay_payload, overlay_sha256)
    evidence_files = [bound_file(path) for path in evidence]
    amended = copy.deepcopy(selection)
    for field in ("owners", "domain_rule_overlays", "preferred_owner_programs"):
        for row in amended.get(field, []):
            row["path"] = str((inputs / row["path"]).resolve(strict=True))
    owner_index, overlay_index = owners[0], overlays[0]
    old_owner = copy.deepcopy(selection["owners"][owner_index])
    old_overlay = copy.deepcopy(selection["domain_rule_overlays"][overlay_index])
    amended["owners"][owner_index].update(owner)
    amended["domain_rule_overlays"][overlay_index].update(
        path=overlay["path"], bytes=overlay["bytes"])
    if "sha256" in old_overlay:
        amended["domain_rule_overlays"][overlay_index]["sha256"] = overlay["sha256"]
    if "total_bundle_bytes" in amended:
        # The selection's bundle total counts owners, not repair overlays.
        if selection["total_bundle_bytes"] != sum(row["bytes"] for row in selection["owners"]):
            raise ValueError("unexpected source total_bundle_bytes convention")
        amended["total_bundle_bytes"] = sum(row["bytes"] for row in amended["owners"])
    plan = {
        "schema": "rustred.owner-replacement-staging.v1", "input_only": True,
        "native_admission_performed": False, "campaign_started": False,
        "family_closure_claim": False, "performance_claim": False,
        "source_campaign": str(source), "destination_campaign": str(destination),
        "source_selection": manifest, "source_queries": queries,
        "source_receipt": bound_file(inputs / "input-receipt.json"),
        "owner_mask": owner_mask, "old_owner": old_owner, "new_owner": owner,
        "old_overlay": old_overlay, "new_overlay": overlay,
        "owner_count": len(selection["owners"]),
        "route_count": len(selection.get("initial_frontier_routes", [])),
        "overlay_count": len(selection.get("domain_rule_overlays", [])),
        "query_count": count,
        "query_roles": {role: sum(value == role for value in roles.values())
                        for role in ("required", "auxiliary")},
        "evidence": evidence_files, "utility": bound_file(Path(__file__)),
        "stager": bound_file(EXAMPLES / "stage_saved_owner_campaign.py"),
        "staged": False,
    }
    if not execute:
        return plan
    # Keep the exact amended source manifest as provenance; the generic stager
    # relocates payload references to portable relative paths in inputs/.
    destination.mkdir(parents=True, exist_ok=False)
    marker = destination / "STAGING_INCOMPLETE"
    marker.write_text("No launch: replacement staging has not completed.\n")
    amended_path = destination / "replacement-source-selection.json"
    amended_path.write_text(json.dumps(amended, indent=2) + "\n")
    amended_path.chmod(0o444)
    attachments = [inputs / row["path"] for row in original_receipt.get("attachments", [])]
    staged = STAGER.stage(amended_path, inputs / "queries.json", destination / "inputs",
                          inputs, query_order="preserve", attachments=attachments)
    if PRODUCTION.verify_inputs(inputs)[2] != original_receipt:
        raise ValueError("source receipt changed during staging")
    if bound_file(owner_payload, owner_sha256) != owner or bound_file(overlay_payload, overlay_sha256) != overlay:
        raise ValueError("replacement changed during staging")
    if (staged["queries_sha256"] != queries_sha256 or not staged["query_bytes_unchanged"]
            or PRODUCTION.verify_inputs(destination / "inputs")[0] != count):
        raise ValueError("staged query identity changed")
    expected = copy.deepcopy(selection)
    expected["owners"][owner_index].update(bytes=owner["bytes"], sha256=owner["sha256"])
    expected["domain_rule_overlays"][overlay_index]["bytes"] = overlay["bytes"]
    if "sha256" in old_overlay:
        expected["domain_rule_overlays"][overlay_index]["sha256"] = overlay["sha256"]
    if "total_bundle_bytes" in expected:
        expected["total_bundle_bytes"] = amended["total_bundle_bytes"]
    actual = json.loads((destination / "inputs" / "selection.json").read_bytes())
    # Relative filenames may be normalized, but no other inventory field changes.
    for field in ("owners", "domain_rule_overlays", "preferred_owner_programs"):
        for row, old in zip(actual.get(field, []), expected.get(field, []), strict=True):
            row["path"] = old["path"]
    if actual != expected:
        raise ValueError("unexpected selection change beyond two payload replacements")
    expected_owner_hashes = [row["sha256"] for row in original_receipt["owners"]]
    expected_owner_hashes[owner_index] = owner_sha256
    expected_overlay_hashes = [row["sha256"] for row in original_receipt["domain_rule_overlays"]]
    expected_overlay_hashes[overlay_index] = overlay_sha256
    if ([row["sha256"] for row in staged["owners"]] != expected_owner_hashes
            or [row["sha256"] for row in staged["domain_rule_overlays"]] != expected_overlay_hashes):
        raise ValueError("staged payload identity differs from declared replacement")
    plan.update(staged=True, staged_receipt=bound_file(destination / "inputs" / "input-receipt.json"),
                staged_selection=bound_file(destination / "inputs" / "selection.json"))
    if evidence_files:
        (destination / "evidence").mkdir()
        for index, binding in enumerate(evidence_files):
            relative = Path("evidence") / f"{index:02d}-{Path(binding['path']).name}"
            size, sha = STAGER.copy_read_only(Path(binding["path"]), destination / relative)
            if size != binding["bytes"] or sha != binding["sha256"]:
                raise ValueError("evidence changed during staging")
            binding["staged_path"] = str(relative)
    path = destination / "replacement-staging.json"
    path.write_text(json.dumps(plan, indent=2) + "\n")
    path.chmod(0o444)
    marker.unlink()
    return plan


def main():
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    for name in ("source-campaign", "destination", "owner-payload", "overlay-payload"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("owner-mask", "owner-sha256", "overlay-sha256", "selection-sha256", "queries-sha256"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--evidence", type=Path, action="append", default=[])
    parser.add_argument("--stage", action="store_true")
    args = vars(parser.parse_args())
    args["execute"] = args.pop("stage")
    print(json.dumps(prepare(**args), indent=2))


if __name__ == "__main__":
    main()
