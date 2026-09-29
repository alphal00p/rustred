#!/usr/bin/env python3
"""Rewrite RustRed native `.rrbin` programs from Symbolica export format 5 to 6.

RustRed builds before the Symbolica `ef0db494` pin wrote Symbolica export
(state) format 5 and atom storage format 0; the pinned Symbolica refuses format 5
and writes format 6 / atom format 1. For RustRed's candidate owner programs,
whose native payload is a partial state of plain symbols plus a coefficient
table of Num atoms (rational-polynomial coefficients), the two encodings differ
in exactly two places: the 2-byte export version of the state section (5 -> 6)
and the first byte of every coefficient frame (atom format 0 -> 1). This tool
rewrites those bytes and nothing else; it never decodes or re-encodes algebra.
(Measured on the four-loop and five-loop owners: header-rewritten files are
byte-identical to files regenerated under the new Symbolica.)

It refuses, without writing anything, every input it cannot prove to be of
that shape: an unknown envelope version, a section other than
SYMBOLICA_STATE/COEFFICIENTS/FAMILY/PROGRAM, a state that is not format 5
(including an already converted format-6 file), user data or poly variables
that embed atoms, and any coefficient frame that is not a format-0 Num atom.

usage: convert_native_v5_to_v6.py SOURCE_DIR DEST_DIR
  DEST_DIR must not exist. Every `*.rrbin` under SOURCE_DIR is rewritten, every
  other file is copied byte for byte (symlinks are followed), and
  DEST_DIR/conversion-manifest.json records, per file, the source and output
  sha256 and whether it was rewritten or copied.
Exit status: 0 converted, 1 refused input (nothing left behind), 2 usage.
"""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import struct
import sys
from pathlib import Path

ENVELOPE_MAGIC = b"RRPBIN\r\n"
ENVELOPE_VERSION = 1
ENVELOPE_HEADER_BYTES = 20
SECTION_HEADER_BYTES = 12
SYMBOLICA_STATE, COEFFICIENTS, FAMILY, PROGRAM = 1, 2, 3, 4
ALLOWED_SECTIONS = {SYMBOLICA_STATE, COEFFICIENTS, FAMILY, PROGRAM}
STATE_MAGIC = 0x37871367
SOURCE_STATE_VERSION, TARGET_STATE_VERSION = 5, 6
SOURCE_ATOM_FORMAT, TARGET_ATOM_FORMAT = 0, 1
NUM_ATOM_TYPE = 1
MANIFEST = "conversion-manifest.json"
SCHEMA = "rustred.convert-native-v5-to-v6.v1"


class Refused(ValueError):
    """The input is not a format-5 RustRed program this tool can rewrite."""


def _u16(buf, offset):
    return struct.unpack_from("<H", buf, offset)[0], offset + 2


def _u32(buf, offset):
    return struct.unpack_from("<I", buf, offset)[0], offset + 4


def _u64(buf, offset):
    return struct.unpack_from("<Q", buf, offset)[0], offset + 8


def sections(buf):
    """(tag, payload offset, payload length) of every envelope section."""
    if len(buf) < ENVELOPE_HEADER_BYTES or buf[:8] != ENVELOPE_MAGIC:
        raise Refused("not a RustRed binary program (magic)")
    version, _ = _u32(buf, 8)
    if version != ENVELOPE_VERSION:
        raise Refused(f"unsupported RustRed envelope version {version}")
    count, offset = _u32(buf, 16)
    out = []
    for _ in range(count):
        if offset + SECTION_HEADER_BYTES > len(buf):
            raise Refused("truncated section header")
        tag, offset = _u16(buf, offset)
        _, offset = _u16(buf, offset)
        length, offset = _u64(buf, offset)
        if offset + length > len(buf):
            raise Refused("truncated section")
        out.append((tag, offset, length))
        offset += length
    if offset != len(buf):
        raise Refused("trailing envelope bytes")
    return out


def _check_state(state):
    """Walk a Symbolica partial state export; refuse anything that embeds atoms."""
    magic, offset = _u32(state, 0)
    if magic != STATE_MAGIC:
        raise Refused("Symbolica state magic")
    version, offset = _u16(state, offset)
    if version == TARGET_STATE_VERSION:
        raise Refused("already Symbolica export format 6")
    if version != SOURCE_STATE_VERSION:
        raise Refused(f"unsupported Symbolica export format {version}")
    partial = state[offset] != 1
    offset += 1
    symbols, offset = _u64(state, offset)
    for _ in range(symbols):
        if partial:
            _, offset = _u32(state, offset)
        for _ in range(2):  # name, namespace
            length, offset = _u32(state, offset)
            offset += length
        offset += 1 + 4
        for _ in range(2):  # tags, attributes
            count, offset = _u16(state, offset)
            for _ in range(count):
                length, offset = _u32(state, offset)
                offset += length
        user_data = state[offset]
        offset += 1
        if user_data == 1:
            offset += 8
        elif user_data == 2:
            length, offset = _u32(state, offset)
            offset += length
        elif user_data != 0:
            raise Refused(f"symbol user data tag {user_data} (may embed atoms)")
        offset += 1
    finite_fields, offset = _u64(state, offset)
    offset += 8 * finite_fields
    variable_lists, offset = _u64(state, offset)
    for _ in range(variable_lists):
        variables, offset = _u64(state, offset)
        for _ in range(variables):
            kind = state[offset]
            offset += 1
            if kind == 0:
                offset += 4
            elif kind == 1:
                offset += 8
            else:
                raise Refused("function or power poly variable (embedded atom)")
    if offset != len(state):
        raise Refused("trailing Symbolica state bytes")


def rewrite(data: bytes) -> bytes:
    """Return the format-6 program, or raise Refused."""
    buf = bytearray(data)
    found = {}
    try:
        for tag, offset, length in sections(buf):
            if tag not in ALLOWED_SECTIONS:
                raise Refused(f"section tag {tag} is not supported by this converter")
            if tag in found:
                raise Refused(f"duplicate section tag {tag}")
            found[tag] = (offset, length)
        if SYMBOLICA_STATE not in found or COEFFICIENTS not in found:
            raise Refused("program without Symbolica state or coefficient table")
        offset, length = found[SYMBOLICA_STATE]
        _check_state(bytes(buf[offset:offset + length]))
        struct.pack_into("<H", buf, offset + 4, TARGET_STATE_VERSION)
        offset, length = found[COEFFICIENTS]
        end = offset + length
        count, cursor = _u64(buf, offset)
        for _ in range(count):
            frame_length, cursor = _u64(buf, cursor)
            if frame_length < 10 or cursor + frame_length > end:
                raise Refused("truncated coefficient frame")
            atom_length, _ = _u64(buf, cursor + 1)
            if atom_length != frame_length - 9:
                raise Refused("coefficient frame length mismatch")
            if buf[cursor] != SOURCE_ATOM_FORMAT:
                raise Refused(f"coefficient frame atom format {buf[cursor]}")
            if buf[cursor + 9] & 7 != NUM_ATOM_TYPE:
                raise Refused("coefficient frame is not a Num atom")
            buf[cursor] = TARGET_ATOM_FORMAT
            cursor += frame_length
        if cursor != end:
            raise Refused("trailing coefficient table bytes")
    except (struct.error, IndexError) as error:
        raise Refused(f"truncated program: {error}") from error
    return bytes(buf)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def convert_tree(source: Path, dest: Path) -> dict:
    """Convert SOURCE into the new directory DEST; nothing is left behind on refusal."""
    source, dest = Path(source), Path(dest)
    if not source.is_dir():
        raise Refused(f"source is not a directory: {source}")
    if dest.exists():
        raise Refused(f"destination exists: {dest}")
    dest.mkdir(parents=True)
    entries = []
    try:
        for root, dirs, files in os.walk(source, followlinks=False):
            dirs.sort()
            rel_root = Path(root).relative_to(source)
            (dest / rel_root).mkdir(parents=True, exist_ok=True)
            for name in sorted(files):
                rel = rel_root / name
                data = (source / rel).read_bytes()
                if name.endswith(".rrbin"):
                    try:
                        out = rewrite(data)
                    except Refused as error:
                        raise Refused(f"{rel}: {error}") from error
                    action = "rewritten"
                else:
                    out, action = data, "copied"
                (dest / rel).write_bytes(out)
                entries.append({"path": str(rel), "action": action, "bytes": len(out),
                                "source_sha256": _sha256(data), "sha256": _sha256(out)})
    except BaseException:
        shutil.rmtree(dest, ignore_errors=True)
        raise
    manifest = {"schema": SCHEMA, "source": str(source.resolve()), "destination": str(dest.resolve()),
                "rewritten": sum(e["action"] == "rewritten" for e in entries),
                "copied": sum(e["action"] == "copied" for e in entries), "files": entries}
    (dest / MANIFEST).write_text(json.dumps(manifest, indent=1) + "\n")
    return manifest


def main(argv=None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if len(argv) != 2 or argv[0].startswith("-"):
        print(__doc__.split("usage: ")[1].split("\n")[0], file=sys.stderr)
        return 2
    try:
        manifest = convert_tree(Path(argv[0]), Path(argv[1]))
    except Refused as error:
        print(f"refused: {error}", file=sys.stderr)
        return 1
    print(json.dumps({k: manifest[k] for k in ("source", "destination", "rewritten", "copied")}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
