#!/usr/bin/env python3
"""Classify explicit parametric boxes using saved owner rules via Rust.

Python only forwards inputs and resource policy. Exit zero means exact local
classification, which may contain gaps or invalid source conditions; it does
not imply applicability, RHS reduction or recursive closure. No IBPs are
generated. Each JSON query supplies its own rank cap (null means unbounded).
"""
import argparse
import os
from pathlib import Path


ALLOWANCES = (
    "max-queries", "max-total-pieces", "max-rules-per-query",
    "max-terminal-checks-per-query", "max-predicates-per-query",
    "max-pieces-per-query", "max-cells-per-query",
    "max-split-operations-per-query", "max-coordinate-cells-per-query",
)


def positive(text: str) -> int:
    if not text.isascii() or not text.isdecimal() or int(text) == 0:
        raise argparse.ArgumentTypeError("work allowance must be a positive integer")
    return int(text)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--queries", type=Path, required=True)
    parser.add_argument("--owner-base", type=Path, default=Path("."))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--events", type=Path)
    parser.add_argument("--stop-file", type=Path)
    parser.add_argument("--no-progress", action="store_true")
    for option in ALLOWANCES:
        parser.add_argument("--" + option, type=positive,
                            help="optional native work/storage allowance, not a rank restriction")
    args = parser.parse_args()
    environment = os.environ.copy()
    for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                 "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS",
                 "SYMBOLICA_HIDE_BANNER"):
        environment[name] = "1"
    command = [str(args.executable.resolve()), "owner-domain-match",
               "--manifest", str(args.manifest), "--queries", str(args.queries),
               "--owner-base", str(args.owner_base), "--output", str(args.output)]
    for option in ("events", "stop_file"):
        if (value := getattr(args, option)) is not None:
            command.extend(["--" + option.replace("_", "-"), str(value)])
    if args.no_progress:
        command.append("--no-progress")
    for option in ALLOWANCES:
        if (value := getattr(args, option.replace("-", "_"))) is not None:
            command.extend(["--" + option, str(value)])
    # Inherit the license without persisting or printing it. Replacement keeps
    # caller affinity/limits and exit status; no extra pool or elapsed deadline.
    os.execve(command[0], command, environment)


if __name__ == "__main__":
    main()
