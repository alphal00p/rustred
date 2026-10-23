#!/usr/bin/env python3
"""Run the isolated release helper and retain exact commands/resource receipts."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cpus", required=True)
    parser.add_argument("--time-executable", default="time")
    args = parser.parse_args()
    cpus = [int(value) for value in args.cpus.split(",")]
    if not cpus:
        raise SystemExit("At least one assigned CPU is required")
    binary = args.binary.resolve(strict=True)
    inputs = args.inputs.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    identity = hashlib.sha256(binary.read_bytes()).hexdigest()
    time_executable = shutil.which(args.time_executable)
    if time_executable is None:
        raise SystemExit("GNU time is unavailable; pass --time-executable explicitly")
    environment = os.environ.copy()
    environment.update(SYMBOLICA_HIDE_BANNER="1", RAYON_NUM_THREADS="1")

    def run(case: str, cpu: int, source: Path) -> dict:
        command = [
            "taskset", "-c", str(cpu),
            time_executable, "-v", "-o", str(output / f"{case}.time"),
            str(binary), case, str(source), str(output / case),
        ]
        started = time.monotonic()
        with (output / f"{case}.stdout").open("x") as stdout, (output / f"{case}.stderr").open("x") as stderr:
            process = subprocess.run(command, env=environment, stdout=stdout, stderr=stderr)
        receipt = {
            "case": case,
            "command": command,
            "binary_sha256": identity,
            "returncode": process.returncode,
            "supervised_wall_s": time.monotonic() - started,
            "CPU_affinity": [cpu],
            "compilation_excluded": True,
            "numerical_values_used": False,
        }
        with (output / f"{case}.receipt.json").open("x") as stream:
            json.dump(receipt, stream, indent=2)
        print(json.dumps(receipt), flush=True)
        return receipt

    receipts = [run("k6", cpus[0], inputs)]
    if receipts[-1]["returncode"]:
        raise SystemExit(receipts[-1]["returncode"])
    with ThreadPoolExecutor(max_workers=min(4, len(cpus))) as pool:
        futures = [
            pool.submit(run, case, cpus[index % len(cpus)], inputs)
            for index, case in enumerate(("h", "fg", "bmw", "x"))
        ]
        receipts.extend(future.result() for future in futures)
    if all(receipt["returncode"] == 0 for receipt in receipts):
        receipts.append(run("combined", cpus[0], output))
    with (output / "receipts.json").open("x") as stream:
        json.dump(receipts, stream, indent=2)
    if any(receipt["returncode"] for receipt in receipts):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
