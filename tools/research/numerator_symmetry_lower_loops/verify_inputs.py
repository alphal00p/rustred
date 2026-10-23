#!/usr/bin/env python3
"""Verify that every pinned source and prepared native file is unchanged."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    if manifest["schema"] != "rustred.lower-loop-symmetry-inputs.v1":
        raise SystemExit("Unexpected input manifest schema")
    checks = []
    for record in manifest["files"]:
        for kind in ("source", "prepared"):
            path = Path(record[kind])
            payload = path.read_bytes()
            checks.append(
                {
                    "path": str(path),
                    "kind": kind,
                    "bytes": len(payload),
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "unchanged": len(payload) == record[kind + "_bytes"]
                    and hashlib.sha256(payload).hexdigest()
                    == record[kind + "_sha256"],
                }
            )
    print(json.dumps({"checks": checks, "all_unchanged": all(c["unchanged"] for c in checks)}, indent=2))
    if not all(c["unchanged"] for c in checks):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
