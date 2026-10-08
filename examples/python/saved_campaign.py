#!/usr/bin/env python3
"""Run, extend, inspect or explicitly refine a saved RustRed campaign.

All mathematics and binary payloads stay native. These commands start actual
work unless --dry-run is supplied; no hidden JSON editing or hash lookup is
needed. Use --help after a subcommand for the small public steering surface.
"""
from __future__ import annotations

import argparse
import importlib.util
import os
from pathlib import Path
import shlex
import sys


def sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def native_executable(campaign, supplied=None):
    if supplied is not None:
        path = Path(supplied).resolve()
    else:
        path = Path(__file__).resolve().parents[2] / "target/release/rustred"
        phases = sibling("campaign_phases")
        policy = phases.configuration(campaign)
        frozen = policy.get("executable")
        active_path = campaign / "master-reduction/active-phase.json"
        active = phases.read_json(active_path) if active_path.is_file() else {}
        active_directory = Path(active.get("directory", ""))
        latest = active_directory / "latest.json"
        resumable_phase = (active.get("operation") in ("publish", "refine") and latest.is_file()
                           and phases.read_json(latest).get("status") not in
                           ("completed_nonminimal", "published_unrefined"))
        if frozen and (resumable_phase or not path.is_file()):
            # A later cargo build must not silently fork a paused native row
            # cursor. An explicit --executable remains an intentional upgrade.
            path = campaign / "master-reduction" / frozen["path"]
    if not path.is_file() or not os.access(path, os.X_OK):
        raise ValueError("supply --executable /path/to/current/rustred (an optimized native build is required)")
    return path


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    commands = parser.add_subparsers(dest="action", required=True)
    for action in ("run", "extend", "inspect", "refine", "publish"):
        sub = commands.add_parser(action, allow_abbrev=False)
        sub.add_argument("--campaign", type=Path, required=True)
        sub.add_argument("--executable", type=Path, help="publisher/refiner/inspector binary (default current optimized build)")
        if action != "inspect":
            sub.add_argument("--dry-run", action="store_true", help="show/validate commands without starting work")
        if action == "run":
            sub.add_argument("--resume", action="store_true", help="resume; also selected automatically if a checkpoint exists")
        if action == "extend":
            sub.add_argument("--rank", type=int, required=True, help="new cumulative input numerator-rank ceiling")
            bounds = sub.add_mutually_exclusive_group()
            bounds.add_argument("--max-power-difference", type=int, help="new D=A-R ceiling; omitted preserves current ceiling")
            bounds.add_argument("--unbounded-power-difference", action="store_true", help="remove only the additional stage ceiling")
        if action == "refine":
            sub.add_argument("--collection-artifact", type=Path, action="append", default=[],
                             help="also collect terminals from a compatible published artifact (repeatable; native preparation remains in Rust)")
            sub.add_argument("--normalization-profile", choices=("conservative", "standard"),
                             help="finite normalization budget (default saved preference, then source artifact profile)")
            sub.add_argument("--seed-depth", type=int, help="finite IBP search depth, unrelated to input rank (default saved preference or 0)")
            sub.add_argument("--containing-sector-depth", type=int,
                             help="promote up to N inactive indices to +1 for extra IBP sources (default saved preference or 0)")
            sub.add_argument("--saved-rule-assistance", action=argparse.BooleanOptionalAction, default=None,
                             help="include applicable saved-rule and routing equations (default saved refinement preference or off)")
            sub.add_argument("--circuit-symmetry-assistance", action=argparse.BooleanOptionalAction, default=None,
                             help="include native circuit-reflection equations (default saved refinement preference or off)")
            sub.add_argument("--finite-feedback", action=argparse.BooleanOptionalAction, default=None,
                             help="combine retained finite rows after full-U aliases (default saved preference or on)")
        if action == "inspect":
            sub.add_argument("--format", choices=("auto", "table", "json"), default="auto")
    args, extra = parser.parse_known_args(argv)
    if args.action not in ("run",) and extra:
        parser.error("unrecognized arguments: " + " ".join(extra))
    campaign = args.campaign.resolve()
    try:
        executable = native_executable(campaign, args.executable)
        if args.action == "inspect":
            command = [str(executable), "artifact-inspect", "--campaign-directory", str(campaign), "--format", args.format]
        else:
            if args.action == "extend":
                helper = sibling("extend_rank_campaign")
                if args.rank < 0:
                    raise ValueError("input rank must be nonnegative")
                difference = (None if args.unbounded_power_difference else args.max_power_difference
                              if args.max_power_difference is not None else helper.PRESERVE_DIFFERENCE)
                receipt = helper.extend(campaign, args.rank, max_power_difference=difference, dry_run=args.dry_run)
                print(f"Scope {'preview' if args.dry_run else 'extension'}: rank ≤{receipt['rank']}, "
                      f"D ceiling {receipt['max_power_difference']}; {receipt['new_required_queries']} added query domains.", flush=True)
            command = [sys.executable, "-B", str(Path(__file__).with_name("production_saved_owner_campaign.py")),
                       "--campaign-directory", str(campaign), "--master-reduction-executable", str(executable)]
            if args.action != "run" or args.resume or (campaign / "checkpoints/main/latest.json").is_file():
                command.append("--resume")
            elif not (campaign / "bin/executable.json").is_file():
                command += ["--executable", str(executable)]
            if args.action == "refine":
                command.append("--refine-masters")
                for artifact in args.collection_artifact:
                    command += ["--master-collection-artifact", str(artifact.resolve())]
                if args.normalization_profile is not None:
                    command += ["--master-normalization-profile", args.normalization_profile]
                if args.saved_rule_assistance is not None:
                    command.append("--master-saved-rule-assistance" if args.saved_rule_assistance
                                   else "--no-master-saved-rule-assistance")
                if args.circuit_symmetry_assistance is not None:
                    command.append("--master-circuit-symmetry-assistance" if args.circuit_symmetry_assistance
                                   else "--no-master-circuit-symmetry-assistance")
                if args.finite_feedback is not None:
                    command.append("--master-finite-feedback" if args.finite_feedback
                                   else "--no-master-finite-feedback")
                if args.seed_depth is not None:
                    if args.seed_depth < 0:
                        raise ValueError("seed depth must be nonnegative")
                    command += ["--master-seed-depth", str(args.seed_depth)]
                if args.containing_sector_depth is not None:
                    if args.containing_sector_depth < 0:
                        raise ValueError("containing-sector depth must be nonnegative")
                    command += ["--master-containing-sector-depth", str(args.containing_sector_depth)]
            elif args.action == "publish":
                command.append("--publish-only")
            command += extra
            if args.dry_run:
                if args.action == "extend":
                    invocation = list(sys.argv[1:] if argv is None else argv)
                    command = [sys.executable, "-B", str(Path(__file__).resolve()),
                               *(part for part in invocation if part != "--dry-run")]
                else:
                    command += ["--start"]
                print("No native work launched. Command: " + shlex.join(command))
                return 0
            command.append("--start")
        # Replace this thin wrapper so signals reach the owned native supervisor.
        os.execv(command[0], command)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    raise SystemExit(main())
