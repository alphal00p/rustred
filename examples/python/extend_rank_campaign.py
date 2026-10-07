#!/usr/bin/env python3
"""Append a required rank stage to a stopped CP6 campaign; launch only with --start.

The original request, helpers, rules, checkpoint records and pending work are
immutable. New cumulative R<=N queries come from the original query attachment,
not from widening the restricted R0 file. This is not a closure certificate.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile

from prepare_rank_campaign import (PRODUCTION, STAGE, RECEIPT_NAME, SOURCE_QUERIES_NAME,
                                   plan_rank_queries, power_difference, cli_power_difference)


SCHEMA = "rustred.owner-domain-scope-extension.json.v1"
PROVENANCE_SCHEMA = "rustred.rank-scope-extension.v1"
MAX_BYTES = 16 * 1024 * 1024
HEX = re.compile(r"[0-9a-f]{64}\Z")
PRESERVE_DIFFERENCE = object()


def encoded(value):
    return (json.dumps(value, indent=2, allow_nan=False) + "\n").encode()


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read_document(path):
    with Path(path).open("rb") as stream:
        raw = stream.read(MAX_BYTES + 1)
    if len(raw) > MAX_BYTES:
        raise ValueError(f"JSON document exceeds {MAX_BYTES} bytes: {path}")
    value = STAGE.ROLES.loads_document(raw)
    if not isinstance(value, dict):
        raise ValueError(f"JSON document must be an object: {path}")
    return value, raw


def checkpoint_snapshot(checkpoint):
    """Observe bounded CP6 JSON, not the ledger; native resume authenticates it."""
    document, manifest_bytes = read_document(checkpoint / "latest.json")
    manifest = document.get("manifest")
    if (not isinstance(manifest, dict) or manifest.get("format") != "RUSTRED-WALK-CP6"
            or type(manifest.get("schema")) is not int or manifest["schema"] not in (1, 2, 3)
            or manifest.get("resumable") is not True
            or not PRODUCTION.natural(manifest.get("generation"))):
        raise ValueError("rank continuation requires a resumable CP6 latest checkpoint")
    files = manifest.get("files")
    if not isinstance(files, list) or not all(isinstance(item, dict) for item in files):
        raise ValueError("invalid CP6 file inventory")
    references = [item for item in files if item.get("key") == "meta"]
    if len(references) != 1:
        raise ValueError("CP6 must reference exactly one scalar metadata file")
    reference = references[0]
    name = reference.get("file")
    if not isinstance(name, str) or Path(name).name != name or name in ("", ".", ".."):
        raise ValueError("invalid CP6 metadata file name")
    path = checkpoint / name
    if path.resolve().parent != checkpoint.resolve():
        raise ValueError("CP6 metadata must remain inside its checkpoint")
    metadata, raw = read_document(path)
    if type(reference.get("bytes")) is not int or reference["bytes"] != len(raw):
        raise ValueError("CP6 metadata byte count differs from manifest")
    if (metadata.get("schema") != 5 or not HEX.fullmatch(str(metadata.get("request", "")))
            or metadata.get("initial_admission") != "complete"
            or metadata.get("processed_queries") != metadata.get("total_queries")
            or metadata.get("engine_certification_void") is not False):
        raise ValueError("CP6 needs a complete original admission and intact certification authority")
    amendments = metadata.get("amendments")
    if not isinstance(amendments, list) or not all(isinstance(item, dict) for item in amendments):
        raise ValueError("invalid saved amendment inventory")
    return metadata, {"generation": manifest["generation"],
                      "manifest_sha256": sha(manifest_bytes), "metadata_sha256": sha(raw)}


def stage_document(source, rank, sequence, parent, selection_sha256, previous_rank,
                   difference, previous_difference):
    scoped, scope = plan_rank_queries(source, rank, max_power_difference=difference)
    document = json.loads(scoped)
    mapping = []
    for index, row in enumerate(document["queries"]):
        original = row["id"]
        # Bound the ID independently of arbitrary original Unicode ID length.
        row["id"] = f"rank{rank}-d{difference if difference is not None else 'all'}-{index:05d}-{sha(original.encode())[:24]}"
        mapping.append({"source_id": original, "id": row["id"]})
    return {"schema": SCHEMA, "sequence": sequence, "parent": parent,
            "queries": document["queries"],
            "query_roles": {"required": [row["id"] for row in document["queries"]], "auxiliary": []},
            "provenance": {"schema": PROVENANCE_SCHEMA,
                           "source_queries_sha256": scope["source_queries_sha256"],
                           "selection_sha256": selection_sha256,
                           "previous_max_numerator_rank": previous_rank,
                           "max_numerator_rank": rank,
                           "previous_max_power_difference": previous_difference,
                           "max_power_difference": difference,
                           "omitted_disjoint_power_difference_query_ids": scope[
                               "omitted_disjoint_power_difference_query_ids"],
                           "source_query_ids": mapping,
                           "coordinate_positive_power_and_minimum_difference_bounds_unchanged": True,
                           "helpers_unchanged": True, "descendant_clipping": False,
                           "closure_claim": False}}


def validate_extension(previous_rank, rank, previous_difference, difference):
    STAGE.unsigned(rank, 32, "extended input rank")
    if difference is not None:
        power_difference(difference)
    if rank < previous_rank:
        raise ValueError("cannot lower the previous rank cap")
    if difference is not None and (previous_difference is None or difference < previous_difference):
        raise ValueError("cannot lower the previous power difference cap")
    if rank == previous_rank and difference == previous_difference:
        raise ValueError("stage extension must raise rank or power difference, not repeat a stage")


def plan(campaign, rank, *, max_power_difference=PRESERVE_DIFFERENCE):
    """Read-only plan, with the native checkpoint lock held by the caller."""
    STAGE.unsigned(rank, 32, "extended input rank")
    inputs = campaign / "inputs"
    _, _, inputs_receipt = PRODUCTION.verify_inputs(inputs)
    attachments = {item["name"]: inputs / item["path"]
                   for item in inputs_receipt.get("attachments", [])}
    if not {RECEIPT_NAME, SOURCE_QUERIES_NAME}.issubset(attachments):
        raise ValueError("campaign must originate from prepare_rank_campaign.py")
    base, _ = read_document(attachments[RECEIPT_NAME])
    _, source = read_document(attachments[SOURCE_QUERIES_NAME])
    if base.get("schema") != "rustred.rank-scoped-starting-queries.v1":
        raise ValueError("unsupported initial rank receipt")
    original_rank = base.get("max_numerator_rank")
    original_difference = base.get("max_power_difference")
    expected_base, _ = plan_rank_queries(source, original_rank,
                                        include_auxiliary=base.get("include_auxiliary"),
                                        max_power_difference=original_difference)
    if (sha(source) != base.get("source_queries_sha256")
            or sha(expected_base) != base.get("queries_sha256")
            or sha(expected_base) != inputs_receipt["queries_sha256"]
            or base.get("source_selection_sha256") != inputs_receipt["selection_sha256"]):
        raise ValueError("rank-source attachment, original rank receipt or frozen inputs differ")
    base_document = json.loads(expected_base)
    base_roles = STAGE.ROLES.query_roles(base_document, require_explicit=True)
    metadata, snapshot = checkpoint_snapshot(campaign / "checkpoints/main")
    if metadata.get("total_queries") != len(base_document["queries"]):
        raise ValueError("checkpoint original query inventory differs from frozen inputs")
    records = metadata["amendments"]
    paths = sorted((campaign / "amendments").glob("amendment-*.json"))
    if len(paths) not in (len(records), len(records) + 1):
        raise ValueError("all recorded amendments and at most one pending stage must be present")
    seen = set(base_roles)
    previous_rank, parent = original_rank, metadata["request"]
    previous_difference = original_difference
    required = sum(role == "required" for role in base_roles.values())
    pending = None
    for index, path in enumerate(paths):
        if path.name != f"amendment-{index + 1:04d}.json":
            raise ValueError("amendment filenames must be a consecutive canonical chain")
        document, raw = read_document(path)
        provenance = document.get("provenance")
        if not isinstance(provenance, dict):
            raise ValueError("rank continuation requires rank-stage provenance for every amendment")
        cap = provenance.get("max_numerator_rank")
        difference = provenance.get("max_power_difference")
        STAGE.unsigned(cap, 32, "previous stage rank")
        validate_extension(previous_rank, cap, previous_difference, difference)
        expected = stage_document(source, cap, index + 1, parent,
                                  inputs_receipt["selection_sha256"], previous_rank,
                                  difference, previous_difference)
        if document != expected:
            raise ValueError("existing amendment differs from exact required rank-stage geometry/provenance")
        ids = expected["query_roles"]["required"]
        if seen.intersection(ids):
            raise ValueError("rank-stage query IDs collide with earlier immutable queries")
        if index == len(records):
            pending = (path, raw, expected)
            break
        record = records[index]
        if (record.get("sequence") != index + 1 or record.get("parent") != parent
                or record.get("queries") != len(ids)
                or record.get("first_input") != len(seen)
                or not HEX.fullmatch(str(record.get("digest", "")))):
            raise ValueError("checkpoint amendment cursor differs from supplied stage chain")
        parent, previous_rank, previous_difference = record["digest"], cap, difference
        seen.update(ids)
        required += len(ids)
    difference = previous_difference if max_power_difference is PRESERVE_DIFFERENCE else max_power_difference
    validate_extension(previous_rank, rank, previous_difference, difference)
    if pending is not None and (pending[2]["provenance"]["max_numerator_rank"] != rank
                                or pending[2]["provenance"]["max_power_difference"] != difference):
        raise ValueError("resume the pending rank stage before preparing another stage")
    sequence = len(records) + 1
    document = stage_document(source, rank, sequence, parent,
                              inputs_receipt["selection_sha256"], previous_rank,
                              difference, previous_difference)
    if seen.intersection(document["query_roles"]["required"]):
        raise ValueError("rank-stage query IDs collide with earlier immutable queries")
    path = campaign / "amendments" / f"amendment-{sequence:04d}.json"
    raw = pending[1] if pending is not None else encoded(document)
    # Every completed new scope must publish an artifact. Older frozen Python
    # snapshots only emitted CP6; retain their native engine and use this
    # publisher-aware launcher without rewriting the snapshots.
    launcher = Path(PRODUCTION.__file__).resolve()
    if not (campaign / "bin/steering.json").is_file():
        raise ValueError("freeze the campaign executable and steering before extending it")
    command = [sys.executable, "-B", str(launcher), "--campaign-directory", str(campaign), "--resume"]
    receipt = {"schema": "rustred.rank-stage-plan.v1", "campaign": str(campaign),
               "checkpoint": str(campaign / "checkpoints/main"), **snapshot,
               "sequence": sequence, "rank": rank, "previous_rank": previous_rank,
               "max_power_difference": difference,
               "previous_max_power_difference": previous_difference,
               "amendment": str(path), "amendment_sha256": sha(raw),
               "base_required_queries": sum(role == "required" for role in base_roles.values()),
               "new_required_queries": len(document["queries"]),
               "cumulative_required_queries": required + len(document["queries"]),
               "unchanged_auxiliary_queries": sum(role == "auxiliary" for role in base_roles.values()),
               "original_root_prefix": metadata.get("p0"),
               "checkpoint_work_retained": "records, dependency edges, closed anchors and pending obligations",
               "checkpoint_authentication": "native resume checks complete byte-digest chain and request binding",
               "closure_claim": False, "native_work_started": False,
               "resume_command": command, "start_command": [*command, "--start"]}
    return path, raw, receipt


def write_once(path, raw):
    """Refuse replacement, including symlinks; allow an identical prepared file."""
    if path.is_symlink():
        raise ValueError(f"refusing symlink output: {path}")
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".rank-stage-", delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        # Publish a complete file atomically, but never replace an existing one.
        os.link(temporary, path)
        descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    except FileExistsError:
        if path.read_bytes() != raw:
            raise ValueError(f"refusing to replace immutable file: {path}") from None
    finally:
        if temporary is not None:
            temporary.unlink()


def extend(campaign, rank, *, max_power_difference=PRESERVE_DIFFERENCE, dry_run=False):
    campaign = Path(campaign).resolve(strict=True)
    checkpoint = campaign / "checkpoints/main"
    if not (checkpoint / "checkpoint.lock").is_file():
        raise ValueError("a saved native checkpoint with checkpoint.lock is required")
    if PRODUCTION.campaign_run_liveness(campaign):
        raise ValueError("campaign is still running; request an orderly save and wait for its exit")
    with PRODUCTION.checkpoint_lock(checkpoint), PRODUCTION.existing_phase_lock(campaign):
        if PRODUCTION.campaign_run_liveness(campaign):
            raise ValueError("campaign started while acquiring checkpoint lock; wait for an orderly exit")
        path, raw, receipt = plan(campaign, rank, max_power_difference=max_power_difference)
        if not dry_run:
            directory = path.parent
            if directory.resolve().parent != campaign:
                raise ValueError("amendments directory must remain inside this campaign")
            directory.mkdir(exist_ok=True)
            write_once(path, raw)
            # The immutable amendment itself is the rank receipt; the printed
            # launch plan observes the current saved generation on every call.
        return {**receipt, "status": "preview-only" if dry_run else "amendment-prepared-no-native-work"}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--campaign-directory", type=Path, required=True)
    parser.add_argument("--extend-rank", type=STAGE.cli_unsigned(32, "extended input rank"), required=True)
    difference = parser.add_mutually_exclusive_group()
    difference.add_argument("--max-power-difference", type=cli_power_difference,
                            default=PRESERVE_DIFFERENCE,
                            help="new D=A-R cap; omit to preserve the previous stage's cap")
    difference.add_argument("--unbounded-power-difference", action="store_true",
                            help="remove the stage D cap (original per-query bounds still apply)")
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--dry-run", action="store_true", help="read-only preview; do not write the amendment")
    modes.add_argument("--start", action="store_true", help="prepare and explicitly start the frozen resume launcher")
    args = parser.parse_args(argv)
    try:
        receipt = extend(args.campaign_directory, args.extend_rank, dry_run=args.dry_run,
                         max_power_difference=None if args.unbounded_power_difference
                         else args.max_power_difference)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    receipt["launch_requested"] = args.start
    print(json.dumps(receipt, indent=2), flush=True)
    if args.start:
        return subprocess.call(receipt["start_command"])
    if args.dry_run:
        print("Preview only: no amendment was written. Run this helper without --dry-run before resuming.")
    else:
        print("Resume when ready: " + shlex.join(receipt["start_command"]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
