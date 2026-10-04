"""Read-only identity checks for explicitly sampled, committed CP6 sidecars.

These checks do not authenticate the unsampled record body or prove closure.
The native writer seals/fsyncs/drops its tail before publishing its registry;
only that captured registry, never a filename alone, selects eligible files.
"""
import hashlib
import json
import os
from pathlib import Path
import stat


MIB = 1 << 20


def identity(st):
    if not stat.S_ISREG(st.st_mode):
        raise ValueError("sealed sampling requires a regular non-symlink file")
    return {key: getattr(st, "st_" + key)
            for key in ("dev", "ino", "size", "mtime_ns", "ctime_ns")}


def read_ranges(path, ranges, expected=None):
    """One read-only, no-follow fd for all ranges of one selected segment."""
    path = Path(path)
    if path.parent.resolve() != path.parent:
        raise ValueError("sealed sampling rejects symlink parent paths")
    before = identity(path.lstat())
    if expected is not None and before != expected:
        raise ValueError("planned sealed file identity changed")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        if identity(os.fstat(fd)) != before:
            raise ValueError("sealed path changed while opening")
        data = []
        for start, count in ranges:
            if not 0 <= start <= before["size"] or not 0 < count <= MIB or start + count > before["size"]:
                raise ValueError("invalid bounded sealed read")
            block = os.pread(fd, count, start)
            if len(block) != count:
                raise ValueError("short bounded sealed read")
            data.append(block)
        if identity(os.fstat(fd)) != before or identity(path.lstat()) != before:
            raise ValueError("sealed file changed during bounded read")
        return data, before
    finally:
        os.close(fd)


def snapshot(path):
    path = Path(path).absolute()
    size = identity(path.lstat())["size"]
    if not 0 < size <= MIB:
        raise ValueError("live sampling metadata exceeds 1MiB bound or is empty")
    data, binding = read_ranges(path, [(0, size)])
    raw = data[0]
    return dict(path=str(path), identity=binding,
                sha256=hashlib.sha256(raw).hexdigest(), text=raw.decode("utf-8"))


def value(snap):
    raw = snap["text"].encode("utf-8")
    if len(raw) != snap["identity"]["size"] or hashlib.sha256(raw).hexdigest() != snap["sha256"]:
        raise ValueError("captured metadata snapshot differs from its binding")
    return json.loads(raw)


def validate_source(source):
    snapshots = source["sealed_snapshots"]
    envelope = value(snapshots["latest"])
    manifest = envelope["manifest"]
    generation = manifest["generation"]
    cp = Path(source["campaign"]) / "checkpoints/main"
    if (envelope != source["latest_manifest"] or manifest["format"] != "RUSTRED-WALK-CP6"
            or manifest["schema"] != 3 or source["latest_manifest_sha256"] != snapshots["latest"]["sha256"]
            or snapshots["latest"]["path"] != str(cp / "latest.json")):
        raise ValueError("captured committed manifest mismatch")
    for key, role in (("meta", "meta"), ("record-segments", "registry")):
        entries = [entry for entry in manifest["files"] if entry["key"] == key]
        if len(entries) != 1:
            raise ValueError("committed metadata must have one generation entry")
        entry, snap = entries[0], snapshots[role]
        if (entry["file"] != f"epoch-{generation:020}-{key}.part"
                or snap["path"] != str(cp / entry["file"])
                or snap["identity"]["size"] != entry["bytes"]):
            raise ValueError("committed metadata generation/length mismatch")
        parsed = value(snap)
        if key == "meta":
            if parsed["record_schema"] != 1 or source["record_schema"] != 1:
                raise ValueError("unsupported live record schema")
        elif (parsed != source["record_segments"] or len(parsed) != entry["count"]
              or source["record_segments_inventory_sha256"] != snap["sha256"]):
            raise ValueError("captured committed registry mismatch")
    first, previous = 0, 0
    for segment in source["record_segments"]:
        gen = segment["generation"]
        if (not previous < gen <= generation or segment["file"] != f"records-{gen:020}.bin"
                or segment["first"] != first or segment["count"] <= 0 or segment["bytes"] <= 0):
            raise ValueError("invalid committed segment generation/file/count")
        first += segment["count"]
        previous = gen
    if (value(snapshots["receipt"]) != source["input_receipt"]
            or snapshots["receipt"]["path"] != str(Path(source["campaign"]) / "inputs/input-receipt.json")
            or snapshots["receipt"]["sha256"] != source["input_receipt_sha256"]
            or snapshots["active"]["path"] != str(Path(source["campaign"]) / "active-run.json")
            or value(snapshots["active"]).get("executable_sha256") != source["executable_sha256"]):
        raise ValueError("captured production input/run binding mismatch")


def verify_source(source):
    validate_source(source)
    # latest may atomically advance. Never reselect it; only the captured
    # immutable generation files and original run/input identities are checked.
    for role in ("meta", "registry", "receipt", "active"):
        saved = source["sealed_snapshots"][role]
        if snapshot(saved["path"]) != saved:
            raise ValueError("pinned committed metadata or run identity changed")


def validate_plan(plan):
    if len(plan["sources"]) != 1 or plan["metadata_only_sources"]:
        raise ValueError("live sealed mode requires exactly one source campaign")
    source = plan["sources"][0]
    validate_source(source)
    windows = plan["windows"]
    generations = sorted({w["generation"] for w in windows})
    if len(generations) != 2 or len(windows) != 8:
        raise ValueError("live sealed mode requires two generations and eight windows")
    committed = source["latest_manifest"]["manifest"]["generation"]
    for gen in generations:
        if gen >= committed:
            raise ValueError("live sampling requires older committed sealed generations")
        chosen = [w for w in windows if w["generation"] == gen]
        segment = next(s for s in source["record_segments"] if s["generation"] == gen)
        expected = [(segment["bytes"] * n // 8, min(MIB, segment["bytes"] - segment["bytes"] * n // 8))
                    for n in (1, 3, 5, 7)]
        if [(w["start"], w["bytes"]) for w in chosen] != expected:
            raise ValueError("live sample must retain the four fixed byte windows")
        for window in chosen:
            if (window["source_id"] != 0 or window["stat_identity"]["size"] != segment["bytes"]
                    or window["stat_identity"]["mtime_ns"] != window["stat_mtime_ns"]):
                raise ValueError("live window/file identity mismatch")
    if plan["total_requested_bytes"] > 8 * MIB:
        raise ValueError("live sample exceeds 8MiB bound")


def read_windows(plan):
    """Read at most 8MiB; no record-body digest, restore, lock or lifecycle action."""
    source = plan["sources"][0]
    verify_source(source)
    data = {}
    for path in dict.fromkeys(w["path"] for w in plan["windows"]):
        chosen = [(i, w) for i, w in enumerate(plan["windows"]) if w["path"] == path]
        expected = chosen[0][1]["stat_identity"]
        if any(w["stat_identity"] != expected for _, w in chosen):
            raise ValueError("inconsistent planned segment identities")
        blocks, _ = read_ranges(path, [(w["start"], w["bytes"]) for _, w in chosen], expected)
        data.update((i, block) for (i, _), block in zip(chosen, blocks))
    verify_source(source)
    return data
