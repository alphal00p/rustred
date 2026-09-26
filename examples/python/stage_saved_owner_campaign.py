#!/usr/bin/env python3
"""Copy immutable saved-owner inputs into a persistent, portable campaign folder.

No native loading, rule generation, or campaign launch. By default the supplied
query document is copied byte for byte. Optional rank/power-bounded owner
orthants are additional native query obligations, not certified coverage: every
original query is retained and no descendant is clipped. Only owner payload
paths in the copied selection are changed to relative paths in the new folder.
Optional helpers-first ordering changes only query order, not query objects or
native coverage authority; the owner-anchor- ID prefix is merely a heuristic.
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


def plan_query_order(query_bytes, query_order):
    """Reorder metadata only; native admission remains the coverage authority."""
    if query_order == "preserve":
        return query_bytes, None
    if query_order != "helpers-first":
        raise ValueError("query order must be preserve or helpers-first")
    document = json.loads(query_bytes, object_pairs_hook=unique_object)
    if (not isinstance(document, dict) or document.get("schema") != "rustred.owner-domain-queries.json.v2"
            or not isinstance(document.get("queries"), list) or not document["queries"]):
        raise ValueError("query ordering requires a nonempty owner-domain query document")
    rows = document["queries"]
    owner_order, ids = {}, set()
    for row in rows:
        if (not isinstance(row, dict) or not isinstance(row.get("id"), str)
                or not row["id"] or not isinstance(row.get("owner"), str) or not row["owner"]):
            raise ValueError("query ordering requires objects with nonempty string IDs and owners")
        if row["id"] in ids:
            raise ValueError("query ids must be unique")
        ids.add(row["id"])
        owner_order.setdefault(row["owner"], len(owner_order))

    def key(row):
        helper = row["id"].startswith("owner-anchor-")
        rank = row.get("max_numerator_rank")
        # Null is an unbounded rank. Invalid/missing ranks stay stable at the
        # end of the helper class; Rust, not this heuristic, validates them.
        rank_key = ((0, 0) if "max_numerator_rank" in row and rank is None else
                    (1, -rank) if type(rank) is int and 0 <= rank < 2 ** 32 else (2, 0))
        return (owner_order[row["owner"]], 0 if helper else 1, rank_key if helper else (0, 0))

    ordered = sorted(rows, key=key)  # Stable within equal helper ranks and original classes.
    document["queries"] = ordered
    plan = {"mode": query_order, "owner_order": "first_seen", "helper_id_prefix": "owner-anchor-",
            "helper_rank_order": "unbounded_then_descending_u32_then_invalid; stable_ties",
            "query_count": len(rows), "helper_query_count": sum(row["id"].startswith("owner-anchor-") for row in rows),
            "changed": [row["id"] for row in rows] != [row["id"] for row in ordered],
            "query_objects_unchanged": True, "coverage_authority": False,
            "strict_bottom_up_evaluation": False, "family_closure_claim": False}
    return (json.dumps(document, indent=2, allow_nan=False) + "\n").encode("utf-8"), plan


RESERVED_INPUT_NAMES = frozenset({"selection.json", "queries.json", "queries-original.json",
                                  "input-receipt.json", "STAGING_INCOMPLETE", "owners"})


def check_attachments(attachments):
    """Attachments are opaque receipts copied beside the inputs; names must be unique and safe."""
    paths = [Path(item) for item in attachments]
    names = set()
    for path in paths:
        if not path.is_file():
            raise ValueError(f"attachment is not a file: {path}")
        name = path.name
        if (name in RESERVED_INPUT_NAMES or name.startswith(".") or "/" in name
                or not name.isprintable() or len(name.encode("utf-8")) > 255):
            raise ValueError(f"attachment name is reserved or unsafe: {name}")
        if name in names:
            raise ValueError(f"duplicate attachment name: {name}")
        names.add(name)
    return paths


def copy_read_only(source, target):
    """Copy with a change-under-us check; returns (bytes, sha256)."""
    before = source.stat()
    source_hash = digest(source)
    with source.open("rb") as incoming, target.open("xb") as outgoing:
        shutil.copyfileobj(incoming, outgoing, 1024 * 1024)
    after = source.stat()
    if (before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns) or digest(target) != source_hash:
        raise ValueError(f"file changed during staging: {source}")
    target.chmod(0o444)
    return before.st_size, source_hash


def stage(manifest, queries, destination, owner_base, *, anchor_max_numerator_rank=None,
          anchor_max_positive_power=None, anchor_positive_power_owners=None, query_order="preserve",
          attachments=()):
    attachments = check_attachments(attachments)
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
    original_bytes = queries.read_bytes()
    staged_query_bytes = original_bytes
    anchor_plan = None
    if anchor_max_numerator_rank is not None:
        staged_query_bytes, anchor_plan = plan_anchor_queries(
            original_bytes, masks, anchor_max_numerator_rank, anchor_max_positive_power,
            anchor_positive_power_owners)
    staged_query_bytes, order_plan = plan_query_order(staged_query_bytes, query_order)
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
    attachment_receipts = []
    for path in attachments:
        size, attachment_hash = copy_read_only(path, destination / path.name)
        attachment_receipts.append({"name": path.name, "path": path.name, "source": str(path.resolve()),
                                    "bytes": size, "sha256": attachment_hash})
    source_query_hash = hashlib.sha256(original_bytes).hexdigest()
    if digest(queries) != source_query_hash:
        raise ValueError("query source changed during staging")
    with (destination / "queries.json").open("xb") as stream:
        stream.write(staged_query_bytes)
    if anchor_plan is not None or order_plan is not None:
        with (destination / "queries-original.json").open("xb") as stream:
            stream.write(original_bytes)
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
               "owners": receipts, "query_bytes_unchanged": staged_query_bytes == original_bytes,
               "query_order": query_order,
               "manifest_changes": "owner payload paths only", "family_closure_claim": False}
    if attachment_receipts:
        receipt["attachments"] = attachment_receipts
    if anchor_plan is not None:
        receipt["anchor_plan"] = anchor_plan
    if order_plan is not None:
        receipt["query_order_plan"] = order_plan
    if anchor_plan is not None or order_plan is not None:
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
    parser.add_argument("--query-order", choices=("preserve", "helpers-first"), default="preserve",
                        help="default: preserve bytes; helpers-first uses owner-anchor- IDs as a non-authoritative ordering heuristic")
    parser.add_argument("--anchor-max-numerator-rank", type=cli_unsigned(32, "anchor rank"),
                        help="append one ordinary owner-orthant query per selected owner; disabled by default")
    parser.add_argument("--anchor-max-positive-power", type=cli_unsigned(64, "anchor positive power"),
                        help="optional positive-power bound for appended anchors; requires anchor rank")
    parser.add_argument("--anchor-positive-power-owners",
                        help="comma-separated selected masks receiving the positive-power bound; default: all selected")
    parser.add_argument("--attach", type=Path, action="append", default=[], metavar="FILE",
                        help="copy an opaque receipt (for example entry-plan-receipt.json) read-only beside the inputs")
    args = parser.parse_args()
    try:
        receipt = stage(args.manifest, args.queries, args.destination, args.owner_base,
                        query_order=args.query_order, attachments=args.attach,
                        anchor_max_numerator_rank=args.anchor_max_numerator_rank,
                        anchor_max_positive_power=args.anchor_max_positive_power,
                        anchor_positive_power_owners=(None if args.anchor_positive_power_owners is None else
                                                     [mask.strip() for mask in args.anchor_positive_power_owners.split(",")]))
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    print(json.dumps({key: value for key, value in receipt.items() if key != "owners"}, sort_keys=True))


if __name__ == "__main__":
    main()
