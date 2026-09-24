#!/usr/bin/env python3
"""Copy immutable saved-owner inputs into a persistent, portable campaign folder.

No native loading, rule generation, or campaign launch. By default the supplied
query document is copied byte for byte. Optional rank/power-bounded owner
orthants are additional native query obligations, not certified coverage: every
original query is retained and no descendant is clipped. Only owner payload
paths in the copied selection are changed to relative paths in the new folder.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import time


def unsigned(value, bits, name):
    if type(value) is not int or not 0 <= value < 2 ** bits:
        raise ValueError(f"{name} must be an unsigned {bits}-bit integer")
    return value


def cli_unsigned(bits, name):
    def parse(value):
        if not value.isascii() or not value.isdecimal():
            raise argparse.ArgumentTypeError(f"{name} must be an unsigned {bits}-bit integer")
        try:
            return unsigned(int(value), bits, name)
        except ValueError as error:
            raise argparse.ArgumentTypeError(str(error)) from error
    return parse


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON field: {key}")
        result[key] = value
    return result


def plan_anchor_queries(query_bytes, masks, max_rank, max_positive_power=None,
                        positive_power_owners=None):
    """Append ordinary native obligations; do not interpret rules or certify a cover.

    Only the generated rows are planned here. Rust remains authoritative for
    admission and interpretation of all original query fields, which are kept.
    """
    unsigned(max_rank, 32, "anchor rank")
    if max_positive_power is not None:
        unsigned(max_positive_power, 64, "anchor positive power")
    if (not masks or any(not isinstance(mask, str) or not mask or set(mask) - {"0", "1"}
                         for mask in masks) or len(set(masks)) != len(masks)
            or len({len(mask) for mask in masks}) != 1):
        raise ValueError("anchor owners must be unique binary masks with one common arity")
    if positive_power_owners is not None:
        if max_positive_power is None:
            raise ValueError("anchor positive-power owners require an explicit anchor positive power")
        if (not isinstance(positive_power_owners, (list, tuple)) or not positive_power_owners
                or any(not isinstance(mask, str) or mask not in masks for mask in positive_power_owners)
                or len(set(positive_power_owners)) != len(positive_power_owners)):
            raise ValueError("anchor positive-power owners must be unique selected owner masks")
    bounded_owners = set(masks if positive_power_owners is None else positive_power_owners)
    document = json.loads(query_bytes, object_pairs_hook=unique_object)
    if (not isinstance(document, dict) or document.get("schema") != "rustred.owner-domain-queries.json.v2"
            or not isinstance(document.get("queries"), list) or not document["queries"]):
        raise ValueError("anchor planning requires a nonempty owner-domain query document")
    originals = document["queries"]
    ids = set()
    for row in originals:
        if not isinstance(row, dict):
            raise ValueError("original queries must be objects")
        query_id = row.get("id")
        if not isinstance(query_id, str) or not 1 <= len(query_id.encode("utf-8")) <= 128:
            raise ValueError("query id must contain 1..128 UTF-8 bytes")
        if query_id in ids:
            raise ValueError("query ids must be unique")
        ids.add(query_id)
        owner = row.get("owner")
        if not isinstance(owner, str) or len(owner) != len(masks[0]) or set(owner) - {"0", "1"}:
            raise ValueError("original query owner must be an arity-sized binary mask")
    prefix = "owner-anchor-"
    anchors = []
    for mask in masks:
        power = max_positive_power if mask in bounded_owners else None
        power_label = "none" if power is None else str(power)
        query_id = f"{prefix}r{max_rank}-a{power_label}-{mask}"
        if query_id in ids or len(query_id.encode("utf-8")) > 128:
            raise ValueError("generated anchor id collides with an original id or exceeds 128 bytes")
        anchors.append({"id": query_id, "owner": mask, "lower": [0] * len(mask),
                        "upper": [None] * len(mask), "max_numerator_rank": max_rank,
                        "power_bounds": {"max_positive_power": power,
                                         "min_power_difference": None, "max_power_difference": None}})
    document["queries"] = originals + anchors
    plan = {"kind": "owner_orthant_obligations", "max_numerator_rank": max_rank,
            "max_positive_power": max_positive_power, "original_query_count": len(originals),
            "anchor_query_count": len(anchors), "anchor_id_prefix": prefix,
            "owner_order": "selection", "descendant_clipping": False, "family_closure_claim": False}
    if positive_power_owners is not None:
        plan["positive_power_owners"] = [mask for mask in masks if mask in bounded_owners]
    plan["positive_power_owner_scope"] = ("none" if max_positive_power is None else
                                           "all_selected" if positive_power_owners is None else "listed")
    return (json.dumps(document, indent=2, allow_nan=False) + "\n").encode("utf-8"), plan


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def stage(manifest, queries, destination, owner_base, *, anchor_max_numerator_rank=None,
          anchor_max_positive_power=None, anchor_positive_power_owners=None):
    manifest_bytes = manifest.read_bytes()
    selection = json.loads(manifest_bytes)
    owners = selection.get("owners")
    if not isinstance(owners, list) or not owners:
        raise ValueError("selection must contain a nonempty owner list")
    masks = [row.get("mask") for row in owners]
    if any(not isinstance(mask, str) or not mask or set(mask) - {"0", "1"} for mask in masks) or len(set(masks)) != len(masks):
        raise ValueError("owner masks must be unique binary strings")
    if anchor_max_positive_power is not None and anchor_max_numerator_rank is None:
        raise ValueError("anchor positive power requires an explicit anchor rank")
    if anchor_positive_power_owners is not None and anchor_max_positive_power is None:
        raise ValueError("anchor positive-power owners require an explicit anchor positive power")
    anchor_plan = None
    if anchor_max_numerator_rank is not None:
        original_bytes = queries.read_bytes()
        augmented_bytes, anchor_plan = plan_anchor_queries(
            original_bytes, masks, anchor_max_numerator_rank, anchor_max_positive_power,
            anchor_positive_power_owners)
    destination.mkdir(parents=True, exist_ok=False)
    marker = destination / "STAGING_INCOMPLETE"
    marker.write_text("Not ready until input-receipt.json is complete.\n")
    (destination / "owners").mkdir()
    receipts = []
    for index, row in enumerate(owners):
        source = (owner_base / row["path"]).resolve()
        before = source.stat()
        if before.st_size != row["bytes"]:
            raise ValueError(f"declared payload size mismatch: {source}")
        relative = Path("owners") / f"{index:04d}-{row['mask']}.rrbin"
        target = destination / relative
        source_hash = digest(source)
        with source.open("rb") as incoming, target.open("xb") as outgoing:
            shutil.copyfileobj(incoming, outgoing, 1024 * 1024)
        after = source.stat()
        if (before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns) or digest(target) != source_hash:
            raise ValueError(f"owner changed during staging: {source}")
        target.chmod(0o444)
        receipts.append({"mask": row["mask"], "source": str(source), "path": str(relative),
                         "bytes": before.st_size, "sha256": source_hash})
        row["path"] = str(relative)
    if anchor_plan is None:
        with queries.open("rb") as incoming, (destination / "queries.json").open("xb") as outgoing:
            shutil.copyfileobj(incoming, outgoing, 1024 * 1024)
        source_query_hash = digest(queries)
        if digest(destination / "queries.json") != source_query_hash:
            raise ValueError("query source changed during staging")
    else:
        source_query_hash = hashlib.sha256(original_bytes).hexdigest()
        if digest(queries) != source_query_hash:
            raise ValueError("query source changed during staging")
        for name, contents in (("queries-original.json", original_bytes), ("queries.json", augmented_bytes)):
            with (destination / name).open("xb") as stream:
                stream.write(contents)
        (destination / "queries-original.json").chmod(0o444)
    query_hash = digest(destination / "queries.json")
    with (destination / "selection.json").open("x") as stream:
        json.dump(selection, stream, indent=2)
        stream.write("\n")
    for name in ("selection.json", "queries.json"):
        (destination / name).chmod(0o444)
    receipt = {"schema": "rustred.staged-owner-inputs.v1", "prepared_unix_time": time.time(),
               "source_manifest": str(manifest.resolve()),
               "source_manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
               "selection_sha256": digest(destination / "selection.json"),
               "source_queries": str(queries.resolve()), "source_queries_sha256": source_query_hash,
               "queries_sha256": query_hash,
               "owner_count": len(owners), "owner_bytes": sum(row["bytes"] for row in owners),
               "owners": receipts, "query_bytes_unchanged": anchor_plan is None,
               "manifest_changes": "owner payload paths only", "family_closure_claim": False}
    if anchor_plan is not None:
        receipt["anchor_plan"] = anchor_plan
        receipt["original_queries"] = {"path": "queries-original.json", "sha256": source_query_hash,
                                       "bytes": len(original_bytes)}
    with (destination / "input-receipt.json").open("x") as stream:
        json.dump(receipt, stream, indent=2)
        stream.write("\n")
    marker.unlink()
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--queries", required=True, type=Path)
    parser.add_argument("--owner-base", type=Path, default=Path.cwd())
    parser.add_argument("--destination", required=True, type=Path)
    parser.add_argument("--anchor-max-numerator-rank", type=cli_unsigned(32, "anchor rank"),
                        help="append one ordinary owner-orthant query per selected owner; disabled by default")
    parser.add_argument("--anchor-max-positive-power", type=cli_unsigned(64, "anchor positive power"),
                        help="optional positive-power bound for appended anchors; requires anchor rank")
    parser.add_argument("--anchor-positive-power-owners",
                        help="comma-separated selected masks receiving the positive-power bound; default: all selected")
    args = parser.parse_args()
    try:
        receipt = stage(args.manifest, args.queries, args.destination, args.owner_base,
                        anchor_max_numerator_rank=args.anchor_max_numerator_rank,
                        anchor_max_positive_power=args.anchor_max_positive_power,
                        anchor_positive_power_owners=(None if args.anchor_positive_power_owners is None else
                                                     [mask.strip() for mask in args.anchor_positive_power_owners.split(",")]))
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    print(json.dumps({key: value for key, value in receipt.items() if key != "owners"}, sort_keys=True))


if __name__ == "__main__":
    main()
