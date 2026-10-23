#!/usr/bin/env python3
"""Pin immutable packaged inputs and decompress copies into a fresh directory."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = args.package.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    records = []
    paths = [source / "unit_mass_vacuum_k6.rrbin"]
    for parent in ("h", "fg", "bmw", "x"):
        paths.extend(
            source / "four_loop" / (parent + suffix)
            for suffix in (".candidates.rrbin.gz", ".rrnorm.bin", ".toml", ".csv")
        )
    for path in paths:
        payload = path.read_bytes()
        native = gzip.decompress(payload) if path.suffix == ".gz" else payload
        relative = path.relative_to(source)
        destination = output / relative
        if path.suffix == ".gz":
            destination = destination.with_suffix("")
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open("xb") as stream:
            stream.write(native)
        records.append(
            {
                "source": str(path),
                "source_sha256": sha(payload),
                "source_bytes": len(payload),
                "prepared": str(destination),
                "prepared_sha256": sha(native),
                "prepared_bytes": len(native),
            }
        )
    manifest = {
        "schema": "rustred.lower-loop-symmetry-inputs.v1",
        "source_package": str(source),
        "files": records,
        "ordinary_source_rows_permitted": 0,
        "numerical_values_used": False,
        "master_minimality_claim": False,
    }
    with (output / "manifest.json").open("x") as stream:
        json.dump(manifest, stream, indent=2)
        stream.write("\n")
    print(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()
