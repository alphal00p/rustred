#!/usr/bin/env python3
"""Count/preview a finite starting envelope, never claim IBP closure.

Pass explicit sector masks or an existing owner-selection manifest. All
combinatorics and power counting run in Rust via the public Python API or CLI.
The named profile assumes conventional Feynman-gauge marginal UV forests;
see docs/finite_starting_domains.md before claiming physical coverage.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--loops", required=True, type=int)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--sectors", nargs="+")
    source.add_argument("--owner-selection", type=Path)
    parser.add_argument("--preview", type=int, default=8)
    parser.add_argument("--max-positive-layers", type=int, default=128)
    parser.add_argument("--cli", type=Path,
                        help="Use this release CLI instead of the installed Python extension")
    args = parser.parse_args()
    sectors = args.sectors
    if args.owner_selection is not None:
        with args.owner_selection.open() as stream:
            sectors = [owner["mask"] for owner in json.load(stream)["owners"]]
    specification = json.dumps({
        "schema": "rustred.entry-domain.json.v1",
        "profile": {"name": "renormalizable_marginal_feynman", "loops": args.loops},
        "sectors": sectors,
        "max_positive_layers_per_sector": args.max_positive_layers,
        "max_preview_targets": args.preview,
    })
    if args.cli is None:
        import rustred
        result = rustred.entry_domain_plan(specification)
    else:
        completed = subprocess.run(
            [str(args.cli.resolve()), "entry-domain-plan", "--input", "-", "--output", "-"],
            input=specification, text=True, capture_output=True, check=True,
        )
        result = json.loads(completed.stdout)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
