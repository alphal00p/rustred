#!/usr/bin/env python3
"""Inspect explicit saved candidates on their own guarded domains via Rust.

Exit zero means requested one-hop inspections/rendering finished. It does NOT
mean first-priority applicability, feasibility, solved complements, recursive
coverage or closure. The native report retains guarded complements/problems;
native expression text is display-only. This launcher performs no algebra,
generation, guard synthesis, queue insertion or elapsed-deadline enforcement.
The optional --work-limits JSON accepts applied.cell_refinement_max_cardinality
as a positive integer (null/omitted means off). This native singleton-cell policy
adds no workers; ineligible and affine-adapted cells remain unchanged.
"""
import argparse
import os
from pathlib import Path

ALLOWANCES = ("max-queries", "max-report-events", "max-report-bytes", "max-expression-bytes")


def positive(text: str) -> int:
    if not text.isascii() or not text.isdecimal() or int(text) == 0:
        raise argparse.ArgumentTypeError("allowance must be a positive integer")
    return int(text)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    for name in ("executable", "manifest", "queries", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--owner-base", type=Path, default=Path("."))
    for name in ("work-limits", "events", "stop-file"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--no-progress", action="store_true")
    for name in ALLOWANCES:
        parser.add_argument("--" + name, type=positive)
    args = parser.parse_args()
    command = [str(args.executable.resolve()), "owner-guarded-apply",
               "--manifest", str(args.manifest), "--queries", str(args.queries),
               "--owner-base", str(args.owner_base), "--output", str(args.output)]
    for name in ("work-limits", "events", "stop-file", *ALLOWANCES):
        value = getattr(args, name.replace("-", "_"))
        if value is not None:
            command.extend(["--" + name, str(value)])
    if args.no_progress:
        command.append("--no-progress")
    environment = os.environ.copy()
    for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                 "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS",
                 "SYMBOLICA_HIDE_BANNER"):
        environment[name] = "1"
    # Caller affinity/limits and native status survive exec; any license stays
    # in the inherited environment and is neither printed nor persisted.
    os.execve(command[0], command, environment)


if __name__ == "__main__":
    main()
