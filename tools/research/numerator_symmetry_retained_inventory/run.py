#!/usr/bin/env python3
"""Run a raw symmetry-only native assay with a caller-owned RSS allowance."""
from __future__ import annotations

import argparse
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
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--provider", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--profile", choices=("conservative", "standard"), required=True)
    parser.add_argument("--max-rss-bytes", type=int, required=True)
    parser.add_argument("--target-powers-json")
    parser.add_argument("--time-executable", default="time")
    args = parser.parse_args()
    if args.max_rss_bytes <= 0:
        raise SystemExit("--max-rss-bytes must be positive")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    stop = output / "memory-stop"
    binary = args.binary.resolve(strict=True)
    command = [shutil.which(args.time_executable) or args.time_executable, "-v", "-o", str(output / "process.time"),
        str(binary), str(args.baseline.resolve(strict=True)), str(args.provider.resolve(strict=True)),
        str(output / "native"), str(stop), args.profile]
    if args.target_powers_json is not None:
        command.append(args.target_powers_json)
    started = time.monotonic()
    peak = 0
    with (output / "stdout.jsonl").open("x") as stdout, (output / "stderr.log").open("x") as stderr:
        process = subprocess.Popen(command, stdout=stdout, stderr=stderr, start_new_session=True)
        while process.poll() is None:
            owned = []
            for item in Path("/proc").iterdir():
                if not item.name.isdigit():
                    continue
                try:
                    fields = (item / "stat").read_text().split(") ", 1)[1].split()
                    if int(fields[2]) == process.pid:
                        owned.append((int(item.name), int(fields[21]) * os.sysconf("SC_PAGE_SIZE"), fields[19]))
                except (FileNotFoundError, ProcessLookupError, PermissionError, ValueError):
                    pass
            used = sum(rss for _, rss, _ in owned)
            peak = max(peak, used)
            if used >= args.max_rss_bytes:
                stop.write_text("caller RSS allowance reached\n")
                for pid, _, ticks in owned:
                    if pid == process.pid:
                        continue
                    try:
                        current = Path(f"/proc/{pid}/stat").read_text().split(") ", 1)[1].split()
                        if int(current[2]) == process.pid and current[19] == ticks:
                            os.kill(pid, 9)
                    except (FileNotFoundError, ProcessLookupError):
                        pass
                break
            time.sleep(1)
        returncode = process.wait()
    receipt = {"command": command, "returncode": returncode, "wall_s": time.monotonic() - started,
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "affinity": sorted(os.sched_getaffinity(0)),
        "RAM_allowance_bytes": args.max_rss_bytes, "peak_sampled_RSS_bytes": peak, "memory_censored": stop.exists(),
        "no_time_or_row_limit": True, "new_ibp_generation": False, "saved_rule_application": False}
    with (output / "receipt.json").open("x") as stream:
        json.dump(receipt, stream, indent=2)
    print(json.dumps(receipt))
    raise SystemExit(returncode)


if __name__ == "__main__":
    main()
