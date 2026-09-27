#!/usr/bin/env python3
"""Regenerate the 16 combined four-loop owner programs and verify them against selection.json.

The owner payloads (16 single-sector bundles, 6,562,373 bytes) are not committed. This script
reruns the two `family-candidates` generations that produced them (same argv as the original
build, binary 4a17f9c7), copies each owner's checkpoint shard
`root<parent>/sectors/sector-<native_ordinal>.rrbin` to `<work>/owners/<name>` and checks its
byte size and SHA-256 against `selection.json`, and the two checkpoint manifests against the
digests in `selection.json` `receipts`. Any mismatch is an error (exit 1): a different binary may
legitimately produce different bytes, and then the pins in this directory no longer apply.

The result `<work>` is an owner base for `stage_saved_owner_campaign.py --manifest selection.json
--owner-base <work>`. Stdlib only; no algebra is done here (the native binary generates).

    regenerate_owners.py --executable TMP/fable51-controls/bin/rustred-4a17f9c7 \
        --work-directory TMP/c4l-owners [--cpus 52-63,308-311] [--n-cores 16]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOTS = (("root1022", "9"), ("root511", "0"))  # label, --nonpositive-indices (zero-based)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def generate(executable: Path, work: Path, label: str, nonpositive: str, n_cores: int, cpus: str | None):
    argv = [str(executable), "family-candidates", "--input", "family.toml", "--input-format", "toml",
            "--nonpositive-indices", nonpositive, "--n-cores", str(n_cores), "--exact-backend", "sparse",
            "--numerical-depth", "2", "--finite-case-policy", "search", "--bundle-max-bytes", "1073741824",
            "--bundle-max-entries", "8000000", "--bundle-max-total-coefficient-bytes", "536870912",
            "--checkpoint-dir", f"{label}/sectors", "--checkpoint-max-bytes", "4294967296",
            "--output", f"{label}.candidates.rrbin", "--report-output", f"{label}.generation.toml"]
    if cpus:
        argv = ["taskset", "-c", cpus] + argv
    env = dict(os.environ)
    env.update({"RAYON_NUM_THREADS": str(n_cores), "OMP_NUM_THREADS": "1", "OMP_THREAD_LIMIT": "1",
                "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1", "BLIS_NUM_THREADS": "1",
                "SYMBOLICA_HIDE_BANNER": "1", "NO_COLOR": "1"})
    (work / label).mkdir()
    start = time.monotonic()
    with (work / f"{label}.stdout").open("w") as out, (work / f"{label}.stderr").open("w") as err:
        code = subprocess.call(argv, cwd=work, env=env, stdout=out, stderr=err)
    return {"label": label, "argv": argv, "exit_code": code, "wall_seconds": round(time.monotonic() - start, 2)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--executable", required=True, type=Path)
    parser.add_argument("--work-directory", required=True, type=Path, help="must not exist")
    parser.add_argument("--selection", type=Path, default=HERE / "selection.json")
    parser.add_argument("--n-cores", type=int, default=16, help="the pinned build used 16")
    parser.add_argument("--cpus", help="optional taskset CPU list for the generator")
    args = parser.parse_args()
    args.executable = args.executable.resolve()
    if not os.environ.get("SYMBOLICA_LICENSE"):
        print("warning: SYMBOLICA_LICENSE is not set", file=sys.stderr)
    work = args.work_directory
    work.mkdir(parents=True)
    shutil.copyfile(HERE / "four_loop_common_basis.toml", work / "family.toml")
    selection = json.loads(args.selection.read_text())
    receipt = {"schema": "rustred.c4l-owner-regeneration.v1", "executable": str(args.executable),
               "executable_sha256": sha256(args.executable),
               "pinned_generator_sha256": selection["receipts"]["generator_executable_sha256"],
               "family_toml_sha256": sha256(work / "family.toml"), "generations": [], "owners": [],
               "violations": [], "family_closure_claim": False}
    for label, nonpositive in ROOTS:
        run = generate(args.executable, work, label, nonpositive, args.n_cores, args.cpus)
        receipt["generations"].append(run)
        if run["exit_code"] != 0:
            receipt["violations"].append(f"{label}: family-candidates exit {run['exit_code']}")
    pins = selection["receipts"]["checkpoint_manifest_sha256"]
    for label, _ in ROOTS:
        manifest = work / label / "sectors" / "checkpoint.toml"
        if not manifest.exists():
            receipt["violations"].append(f"{label}: no checkpoint.toml")
            continue
        actual = sha256(manifest)
        if actual != pins[label]:
            receipt["violations"].append(f"{label}: checkpoint.toml sha256 {actual} != pinned {pins[label]}")
    (work / "owners").mkdir()
    for owner in selection["owners"]:
        label = f"root{owner['parent']}"
        manifest = work / label / "sectors" / "checkpoint.toml"
        source = work / label / "sectors" / f"sector-{owner['native_ordinal']}.rrbin"
        row = {"mask": owner["mask"], "path": owner["path"], "source": str(source.relative_to(work))}
        if manifest.exists():
            sectors = tomllib.loads(manifest.read_text())["sectors"]
            mask = "".join("1" if bit else "0" for bit in sectors[owner["native_ordinal"]])
            if mask != owner["mask"]:
                receipt["violations"].append(f"{owner['mask']}: ordinal {owner['native_ordinal']} holds {mask}")
        if not source.exists():
            receipt["violations"].append(f"{owner['mask']}: missing {row['source']}")
            receipt["owners"].append(row)
            continue
        target = work / owner["path"]
        shutil.copyfile(source, target)
        row.update({"bytes": target.stat().st_size, "sha256": sha256(target)})
        if row["bytes"] != owner["bytes"] or row["sha256"] != owner["sha256"]:
            receipt["violations"].append(f"{owner['mask']}: {row['bytes']} bytes sha256 {row['sha256']} "
                                         f"!= pinned {owner['bytes']} {owner['sha256']}")
        receipt["owners"].append(row)
    receipt["status"] = "PASS" if not receipt["violations"] else "FAIL"
    (work / "regeneration-receipt.json").write_text(json.dumps(receipt, indent=1) + "\n")
    print(json.dumps({k: receipt[k] for k in ("status", "violations", "generations")}, indent=1))
    return 0 if receipt["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
