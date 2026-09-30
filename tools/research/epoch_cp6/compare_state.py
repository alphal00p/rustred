#!/usr/bin/env python3
"""Read-only cross-version comparison of drained CP6 durable mathematical state.

This is a measurement diagnostic, NOT a checkpoint authenticator or certificate.
Run each native version's cold-All gate separately. Stable CP6 sections are
streamed as bytes; the typed-record codec is deliberately not duplicated here.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

from contract import FORMAT, require
from receipts import metadata, read, write_new

CONTRACT = "rustred.cp6-durable-state-comparison.v1"
HEADER = struct.Struct("<8sIIIQ")
CHUNK = 1 << 20
SECTIONS = {
    1: "domain_geometry", 2: "node_flags", 3: "lookup_live_bits",
    4: "ledger", 5: "ordered_dependency_targets", 6: "anchor_scopes_and_pins",
    7: "dispatch", 8: "closure_flags", 9: "frontier_counts",
}
REQUIRED = {f"state-{i}" for i in SECTIONS} | {
    "meta", "owners", "inputs", "input-frontiers", "record-segments", "orthants",
}
LOGICAL = {
    "owner_count", "owners_digest", "lockstep_b", "epoch_rolling", "epoch_cut_size",
    "epoch_publication_order", "k", "watermark", "p0", "max_domains", "max_events",
    "max_frontiers", "ledger_counts", "walk", "edge_runs", "edges", "self_edges",
    "initial_admission", "total_queries", "processed_queries", "input_frontiers",
    "stop_reason", "operational_stop", "admission_failure", "amendments",
    "quarantined", "abandoned_obligations", "g2", "imported_prefix",
    "engine_certification_void", "adaptive_dispatch",
}
DIAGNOSTIC = {
    "schema", "request", "walk_semantics_version", "record_schema", "preparation",
    "records_digest", "edge_digest", "lookup", "verify", "closure",
}
DEFAULTS = {"epoch_rolling": False, "epoch_cut_size": 0,
            "epoch_publication_order": "oldest-prefix", "adaptive_dispatch": None}
ESCROW = {"epoch_base_window", "epoch_result_escrow_jobs", "epoch_result_escrow_bytes"}


def integer(value, minimum=0):
    return type(value) is int and value >= minimum


def escrow_policy(meta):
    """Compare explicit operational bounds; old measurements had no escrow."""
    if meta["schema"] != 5:
        require(not ESCROW.intersection(meta), "escrow fields in an older scalar schema")
        return {"epoch_base_window": meta["lockstep_b"],
                "epoch_result_escrow_jobs": 0, "epoch_result_escrow_bytes": None}
    require(ESCROW <= meta.keys(), "scalar5 escrow fields missing")
    base, extra, retained = (meta[key] for key in
        ("epoch_base_window", "epoch_result_escrow_jobs", "epoch_result_escrow_bytes"))
    require(integer(base, 1) and integer(extra) and base + extra <= 4096
            and integer(meta["lockstep_b"], 1) and base + extra == meta["lockstep_b"],
            "escrow logical window differs")
    if extra:
        require(meta.get("epoch_rolling") is True
                and meta.get("epoch_publication_order", DEFAULTS["epoch_publication_order"]) == "oldest-prefix"
                and integer(retained, 1) and retained <= (1 << 64) - 1,
                "enabled escrow policy differs")
    else:
        require(retained is None, "disabled escrow has a byte budget")
    return {key: meta[key] for key in sorted(ESCROW)}


def file_digest(path, expected_bytes=None, closure_flags=False):
    """SHA256 of actual bytes; optional known closure-cache bit exclusion."""
    before = metadata(path)
    if expected_bytes is not None:
        require(before[3] == expected_bytes, f"section byte count differs: {path}")
    whole, semantic, size = hashlib.sha256(), hashlib.sha256(), 0
    with path.open("rb") as stream:
        while block := stream.read(CHUNK):
            whole.update(block)
            if closure_flags:
                # Header is unchanged. Only tracker bit2 is cached closure;
                # sealed/inspected bits0/1 remain part of the comparison.
                start = max(0, HEADER.size - size)
                semantic.update(block[:start])
                flags = block[start:]
                require(all(flag & ~7 == 0 for flag in flags), "invalid closure flag bits")
                semantic.update(flags.translate(bytes(i & 3 for i in range(256))))
            else:
                semantic.update(block)
            size += len(block)
    require(metadata(path) == before, f"file changed while reading: {path}")
    return {"bytes": size, "sha256": whole.hexdigest(),
            "semantic_sha256": semantic.hexdigest()}


def part_path(directory, name):
    require(isinstance(name, str) and name not in ("", ".", "..")
            and Path(name).name == name, "section filename must be a basename")
    path = directory / name
    metadata(path)  # Refuse symlinks and nonregular files.
    return path


def json_lines(path, count):
    before = metadata(path)
    result = []
    # Owner and query inventories are small metadata, unlike record sidecars.
    require(before[3] <= 16 << 20, "inventory JSON exceeds diagnostic limit")
    from receipts import ROLES
    with path.open() as stream:
        for line in stream:
            result.append(ROLES.loads_document(line))
    require(len(result) == count, "inventory line count differs")
    require(metadata(path) == before, "inventory changed while reading")
    return result


def capture(directory, queries):
    directory, queries = Path(directory), Path(queries)
    require(directory.is_dir() and not directory.is_symlink(), "real checkpoint directory required")
    require(not (directory / "epoch-poison").exists(), "poisoned checkpoint")
    latest = directory / "latest.json"
    before = file_digest(latest)
    envelope = read(latest, 64 << 10)
    require(set(envelope) == {"manifest", "blake3"}, "manifest envelope shape")
    manifest = envelope["manifest"]
    require(set(manifest) == {"format", "schema", "generation", "arity",
                              "walk_semantics_version", "resumable", "files"},
            "manifest fields changed; review comparator")
    schema = manifest["schema"]
    require(type(schema) is int and schema in (1, 2, 3), "unsupported CP6 schema")
    semantics = 4 if schema == 3 else 3
    require(manifest["format"] == FORMAT and manifest["walk_semantics_version"] == semantics
            and manifest["resumable"] is True and integer(manifest["generation"], 1)
            and integer(manifest["arity"], 1) and manifest["arity"] <= 32,
            "manifest version/arity/identity differs")
    refs, paths, identities = {}, {}, {}
    for ref in manifest["files"]:
        require(set(ref) == {"key", "file", "count", "bytes", "blake3"}, "file reference shape")
        key = ref["key"]
        require(key not in refs and integer(ref["count"]) and integer(ref["bytes"]),
                "duplicate section or invalid count")
        require(key in REQUIRED | {"rescue"}, "unknown CP6 section; review comparator")
        refs[key], paths[key] = ref, part_path(directory, ref["file"])
        require(paths[key] not in list(paths.values())[:-1], "section file reused")
        identities[key] = metadata(paths[key])
    require(REQUIRED <= refs.keys(), "required checkpoint sections missing")
    meta = read(paths["meta"], 1 << 20)
    require(set(meta) <= LOGICAL | DIAGNOSTIC | ESCROW and LOGICAL - DEFAULTS.keys() <= meta.keys(),
            "scalar fields changed or missing; review comparator")
    require(type(meta["schema"]) is int and meta["schema"] in ((4, 5) if schema == 3 else (3,))
            and meta["walk_semantics_version"] == semantics
            and (schema != 3 or meta.get("record_schema") == 1), "scalar schema differs")
    policy = escrow_policy(meta)
    ledger = meta["ledger_counts"]
    require(isinstance(ledger, list) and len(ledger) == 8 and all(integer(n) for n in ledger)
            and ledger[0] == ledger[1] == 0, "comparison requires drained, unreserved checkpoints")
    require(meta["initial_admission"] == "complete"
            and meta["processed_queries"] == meta["total_queries"], "admission incomplete")
    require(meta["engine_certification_void"] is False, "void engine authority")
    require(refs["state-7"]["count"] == 0, "unfinished dispatch inventory")
    semantic, receipts = {}, {}
    for number, name in SECTIONS.items():
        key, ref = f"state-{number}", refs[f"state-{number}"]
        with paths[key].open("rb") as stream:
            header = stream.read(HEADER.size)
        require(len(header) == HEADER.size, "truncated CP6 section header")
        magic, version, arity, section, count = HEADER.unpack(header)
        require((magic, version, arity, section, count)
                == (b"EPC6PART", 1, manifest["arity"], number, ref["count"]),
                f"section header differs: {key}")
        digest = file_digest(paths[key], ref["bytes"], closure_flags=number == 8)
        receipts[name] = digest
        if number != 7:
            semantic[name] = {"count": count, "sha256": digest["semantic_sha256"]}
        else:
            # Drained dispatch contains six u64 fields then three zero lengths.
            # Session/counter are transport identity, not chosen graph edges.
            require(ref["bytes"] == HEADER.size + 9 * 8, "drained dispatch size differs")
            with paths[key].open("rb") as stream:
                stream.seek(HEADER.size)
                values = struct.unpack("<9Q", stream.read(9 * 8))
            require(values[6:] == (0, 0, 0), "nonempty dispatch queues")
            semantic[name] = {"k": values[0], "p0": values[1], "b": values[2], "cursor": values[5]}
            receipts[name]["session_and_sequence_counter"] = list(values[3:5])
    for key in ("orthants", "rescue"):
        if key in refs:
            digest = file_digest(paths[key], refs[key]["bytes"])
            receipts[key] = digest
            semantic[key] = {"count": refs[key]["count"], "sha256": digest["sha256"]}
        else:
            semantic[key] = None
    for key in ("owners", "inputs", "input-frontiers"):
        semantic[key] = json_lines(paths[key], refs[key]["count"])
    for row in semantic["inputs"]:
        require(isinstance(row, dict) and row.get("role", "required") in ("required", "auxiliary"),
                "invalid input role")
    semantic["query_file"] = file_digest(queries)
    semantic["arity"] = manifest["arity"]
    semantic["logical_metadata"] = {key: meta.get(key, DEFAULTS.get(key)) for key in sorted(LOGICAL)}
    semantic["escrow_policy"] = policy
    closure = meta["closure"]
    semantic["closure_facts"] = {key: closure[key] for key in ("initial", "unavailable", "inspected")}
    for key, ref in refs.items():
        require(metadata(paths[key]) == identities[key] and identities[key][3] == ref["bytes"],
                f"checkpoint section changed or size differs: {key}")
    require(file_digest(latest) == before, "latest checkpoint changed while comparing")
    return {
        "directory": str(directory), "query_file": str(queries), "semantic": semantic,
        "reported_not_required_equal": {
            "schema": schema, "semantics": semantics, "generation": manifest["generation"],
            "scalar_diagnostics": {key: meta.get(key) for key in sorted(DIAGNOSTIC)},
            "section_receipts": receipts,
            "record_segment_inventory": read(paths["record-segments"], 16 << 20),
        },
    }


def differences(left, right, path=""):
    """Deterministic field paths, including exact JSON scalar types."""
    if type(left) is not type(right):
        return [path]
    if isinstance(left, dict):
        out = []
        for key in sorted(left.keys() | right.keys()):
            child = f"{path}.{key}" if path else key
            out.extend([child] if key not in left or key not in right
                       else differences(left[key], right[key], child))
        return out
    if isinstance(left, list):
        if len(left) != len(right):
            return [path + ".length"]
        return [child for i, (a, b) in enumerate(zip(left, right))
                for child in differences(a, b, f"{path}[{i}]")]
    return [] if left == right else [path]


def compare(left, right, left_queries, right_queries):
    a, b = capture(left, left_queries), capture(right, right_queries)
    mismatch = differences(a["semantic"], b["semantic"])
    return {
        "contract": CONTRACT, "durable_mathematical_state_equal": not mismatch,
        "differences": mismatch, "left": a, "right": b,
        "diagnostic_differences": differences(a["reported_not_required_equal"], b["reported_not_required_equal"]),
        "full_record_equality": "NOT_COMPARED",
        "native_authentication_and_cold_all_required_separately": True,
        "limitations": [
            "No typed-record codec, record/event equality or source-identity replay is implemented here.",
            "Ordered dependency target IDs and anchor scopes/pins are compared in unchanged durable sections.",
            "Request hashes differ across semantics versions; actual query bytes and owner inventory are compared, but launch-option equivalence must be checked against bound commands.",
            "Tracker cached closed bits/counts and refresh revisions/timing are reported, not proof of closure; sealed/inspected flags are compared.",
            "Physical lookup/verify counters, record representation, and drained dispatch session/counter are reported, not required equal.",
            "SHA256 compares local bytes; native readers must authenticate BLAKE3 and validate every checkpoint before accepting measurements.",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("left", type=Path)
    parser.add_argument("right", type=Path)
    parser.add_argument("--left-queries", required=True, type=Path)
    parser.add_argument("--right-queries", required=True, type=Path)
    parser.add_argument("--output", type=Path, help="Create a new report; existing files are never overwritten")
    args = parser.parse_args()
    try:
        report = compare(args.left, args.right, args.left_queries, args.right_queries)
        if args.output:
            write_new(args.output, report)
        print(json.dumps(report, indent=2, allow_nan=False))
        return 0 if report["durable_mathematical_state_equal"] else 1
    except (OSError, ValueError, KeyError, TypeError, struct.error) as error:
        print(f"CP6 comparison refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
