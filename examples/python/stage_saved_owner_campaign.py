#!/usr/bin/env python3
"""Copy immutable saved-owner inputs into a persistent, portable campaign folder.

No native loading, rule generation, domain transformation, or campaign launch.
The supplied query document is copied byte for byte. Only owner payload paths
in the copied selection are changed to relative paths within the new folder.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import time


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def stage(manifest, queries, destination, owner_base):
    manifest_bytes = manifest.read_bytes()
    selection = json.loads(manifest_bytes)
    owners = selection.get("owners")
    if not isinstance(owners, list) or not owners:
        raise ValueError("selection must contain a nonempty owner list")
    masks = [row.get("mask") for row in owners]
    if any(not isinstance(mask, str) or not mask or set(mask) - {"0", "1"} for mask in masks) or len(set(masks)) != len(masks):
        raise ValueError("owner masks must be unique binary strings")
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
    with queries.open("rb") as incoming, (destination / "queries.json").open("xb") as outgoing:
        shutil.copyfileobj(incoming, outgoing, 1024 * 1024)
    query_hash = digest(queries)
    if digest(destination / "queries.json") != query_hash:
        raise ValueError("query source changed during staging")
    with (destination / "selection.json").open("x") as stream:
        json.dump(selection, stream, indent=2)
        stream.write("\n")
    for name in ("selection.json", "queries.json"):
        (destination / name).chmod(0o444)
    receipt = {"schema": "rustred.staged-owner-inputs.v1", "prepared_unix_time": time.time(),
               "source_manifest": str(manifest.resolve()),
               "source_manifest_sha256": hashlib.sha256(manifest_bytes).hexdigest(),
               "selection_sha256": digest(destination / "selection.json"),
               "source_queries": str(queries.resolve()), "queries_sha256": query_hash,
               "owner_count": len(owners), "owner_bytes": sum(row["bytes"] for row in owners),
               "owners": receipts, "query_bytes_unchanged": True,
               "manifest_changes": "owner payload paths only", "family_closure_claim": False}
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
    args = parser.parse_args()
    receipt = stage(args.manifest, args.queries, args.destination, args.owner_base)
    print(json.dumps({key: value for key, value in receipt.items() if key != "owners"}, sort_keys=True))


if __name__ == "__main__":
    main()
