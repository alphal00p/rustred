#!/usr/bin/env python3
"""Regenerate seven-loop graph inputs using the existing generic native pipeline.

The default contraction census expands each distinct matroid class once.
Optionally visit every labelled physical subset as a slower control instead.
Neither route generates IBPs or certifies reduction closure.
"""
import argparse
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
PIPELINE = ROOT / "six_loop_study/topology"


def commands(output, threads=1, labelled_subsets=False):
    output = Path(output).resolve()
    common = [sys.executable, "-B"]
    steps = [
        ("enumeration", common + [str(PIPELINE / "enumerate.py"), "--loops", "7",
                                   "--threads", str(threads), "--output", str(output)]),
        ("classification", common + [str(PIPELINE / "classify.py"), "--directory", str(output)]),
        ("exact parent cover", common + [str(PIPELINE / "cover.py"), "--directory", str(output)]),
        ("native family export", common + [str(PIPELINE / "export_families.py"), "--directory", str(output)]),
    ]
    contraction_script = "contractions.py" if labelled_subsets else "unique_contractions.py"
    steps.extend([
        ("full contraction census", common + [str(PIPELINE / contraction_script),
                                               "--manifest", str(output / "manifest.json")]),
        ("selected-sector export", common + [str(PIPELINE / "export_families.py"),
                                              "--directory", str(output)]),
    ])
    steps.append(("independent witness audit", common + [str(PIPELINE / "audit.py"),
                                                         "--directory", str(output)]))
    return steps


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--labelled-subsets", action="store_true",
                        help="diagnostic: enumerate all labelled parent subsets instead of quotient traversal")
    args = parser.parse_args()
    if args.threads <= 0:
        parser.error("threads must be positive")
    for label, command in commands(args.output, args.threads, args.labelled_subsets):
        print(f"Seven-loop topology preparation: {label}", flush=True)
        subprocess.run(command, check=True, cwd=ROOT)


if __name__ == "__main__":
    main()
