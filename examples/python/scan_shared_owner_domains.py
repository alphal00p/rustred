#!/usr/bin/env python3
"""Inspect saved parametric successor domains through the release Rust CLI.

No algebra, IBP generation, concrete target enumeration or closure certification
is performed in Python. A completed scan is NOT a complete reduction campaign.
The Symbolica license is inherited, never written to a file.
"""
import argparse
import os
from pathlib import Path


def rank(text: str) -> int:
    value = int(text)
    if not 0 <= value <= 2**32 - 1:
        raise argparse.ArgumentTypeError("rank must be in 0..=4294967295")
    return value


def positive(text: str) -> int:
    value = int(text)
    if value <= 0:
        raise argparse.ArgumentTypeError("work allowance must be positive")
    return value


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--owner-base", type=Path, default=Path("."))
    scope = parser.add_mutually_exclusive_group(required=True)
    scope.add_argument("--max-numerator-rank", type=rank)
    scope.add_argument("--unbounded-rank", action="store_true",
                       help="scan all saved rule geometry; not a closure claim")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--events", type=Path)
    parser.add_argument("--stop-file", type=Path)
    allowances = ("max-rules-per-owner", "max-terms-per-owner",
                  "max-regions-per-owner", "max-total-regions", "max-summary-groups")
    for option in allowances:
        parser.add_argument("--" + option, type=positive,
                            help="optional native work/storage allowance, not a rank restriction")
    args = parser.parse_args()
    environment = os.environ.copy()
    for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                 "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS",
                 "SYMBOLICA_HIDE_BANNER"):
        environment[name] = "1"
    command = [str(args.executable.resolve()), "owner-domain-scan",
               "--manifest", str(args.manifest), "--owner-base", str(args.owner_base),
               "--output", str(args.output)]
    if args.unbounded_rank:
        command.append("--unbounded-rank")
    else:
        command.extend(["--max-numerator-rank", str(args.max_numerator_rank)])
    for option in ("events", "stop_file"):
        if (value := getattr(args, option)) is not None:
            command.extend(["--" + option.replace("_", "-"), str(value)])
    for option in allowances:
        if (value := getattr(args, option.replace("-", "_"))) is not None:
            command.extend(["--" + option, str(value)])
    # Replace the steering process: affinity/resource limits supplied by the
    # caller apply directly to Rust. No deadline or additional worker pool.
    os.execve(command[0], command, environment)


if __name__ == "__main__":
    main()
