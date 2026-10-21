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
    sys.modules[name] = value
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
    for action in ("run", "extend", "inspect", "refine", "finalize-refinement", "publish"):
        sub = commands.add_parser(action, allow_abbrev=False)
        if action != "inspect":
            sub.epilog = ("Fresh campaigns default to the legacy symbolic-domain engine (Epoch/CP6); "
                          "existing campaigns retain their saved engine. Finite-demand is opt-in: "
                          "first run --engine finite-demand --finite-policy POLICY.json "
                          "--cpus CPUSET --max-memory-bytes BYTES --host-memory-reserve-bytes BYTES. "
                          "Subsequent finite commands retain these settings; --workers N with --cpus CPUSET "
                          "changes the reserved worker budget. Explicit policy/RAM overrides "
                          "are operational, while window partition settings are immutable. "
                          "--windows-per-batch N groups existing windows in one shared parallel solve/commit (default 1). "
                          "--worker-pool persistent|per-group and --window-preparation serial|parallel "
                          "select operational finite scheduling without changing the saved partition. "
                          "Finite operation_limits in --finite-policy defaults to {} (uncapped operations); "
                          "nested counts may be explicit unsigned values or null, and top-level null preserves source limits. "
                          "Inputs must already be staged; no legacy checkpoint import. "
                          "An interrupted initial preparation can be retried explicitly with "
                          "run --repair-preparation; native refuses existing checkpoint state.")
        sub.add_argument("--campaign", type=Path, required=True)
        sub.add_argument("--executable", type=Path, help="publisher/refiner/inspector binary (default current optimized build)")
        if action != "inspect":
            sub.add_argument("--dry-run", action="store_true", help="show/validate commands without starting work")
        if action in ("run", "extend", "publish"):
            sub.add_argument("--deep-verification", action=argparse.BooleanOptionalAction, default=None,
                             help="independently replay the saved dependency graph before publication (default saved preference or off)")
        if action == "run":
            sub.add_argument("--engine", choices=("legacy", "finite-demand"),
                             help="fresh default: legacy symbolic-domain (Epoch/CP6); finite-demand is opt-in; saved engine retained on resume")
            sub.add_argument("--resume", action="store_true", help="resume; also selected automatically if a checkpoint exists")
        if action == "extend":
            sub.add_argument("--rank", type=int, required=True, help="new cumulative input numerator-rank ceiling")
            bounds = sub.add_mutually_exclusive_group()
            bounds.add_argument("--max-power-difference", type=int, help="new D=A-R ceiling; omitted preserves current ceiling")
            bounds.add_argument("--unbounded-power-difference", action="store_true", help="remove only the additional stage ceiling")
        if action == "refine":
            sub.add_argument("--fresh-search", action="store_true",
                             help="start a new relations-only round after finalizing stopped work; old search cursors/shape are not inherited (specify both depths; defaults 0/0 and exact)")
            sub.add_argument("--refinement-strategy", choices=("exact", "modular-circuits"),
                             help="refinement backend (default saved preference or exact); modular discovery accepts only exactly verified maps")
            sub.add_argument("--modular-policy", type=Path,
                             help="JSON modular discovery/reconstruction resource allowances; explicit modular refinement only")
            sub.add_argument("--workers", dest="refinement_workers", type=int, help="refiner-only worker budget; may change on resume")
            sub.add_argument("--cpus", dest="refinement_cpus", help="refiner-only CPU affinity; must match --workers")
            sub.add_argument("--relation-batch-rows", type=int, help="exact refiner's positive parallel window; may change on resume")
            sub.add_argument("--relation-block-rows", type=int, help="exact refiner's positive elimination block size; may change on resume")
            sub.add_argument("--max-relation-columns", type=int,
                             help="operational column budget; 0 removes the limit (default saved preference or unlimited); may change on resume")
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
    if args.action != "refine" and any(option.split("=", 1)[0] in ("--fresh-search", "--master-fresh-search")
                                       for option in extra):
        parser.error("fresh search requires the refine action")
    campaign = args.campaign.resolve()
    try:
        # Engine dispatch precedes all legacy checkpoint access. This regular
        # sibling import also avoids coupling unrelated helper-test adapters.
        finite = importlib.import_module("finite_saved_campaign")
        selected = finite.engine(campaign, getattr(args, "engine", None))
        if selected == finite.ENGINE:
            return finite.public_main(args, extra)
        if args.action not in ("run",) and extra:
            parser.error("unrecognized arguments: " + " ".join(extra))
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
                if args.fresh_search:
                    command.append("--master-fresh-search")
                if args.refinement_strategy is not None:
                    command += ["--master-refinement-strategy", args.refinement_strategy]
                if args.modular_policy is not None:
                    command += ["--master-modular-policy", str(args.modular_policy.resolve())]
                if args.refinement_workers is not None:
                    if args.refinement_workers <= 0: raise ValueError("refinement workers must be positive")
                    command += ["--master-refinement-workers",str(args.refinement_workers)]
                if args.refinement_cpus is not None:
                    command += ["--master-refinement-cpus",args.refinement_cpus]
                for name in ("relation_batch_rows", "relation_block_rows"):
                    value = getattr(args, name)
                    if value is not None:
                        if value <= 0: raise ValueError("relation batch/block rows must be positive")
                        command += ["--master-" + name.replace("_", "-"), str(value)]
                if args.max_relation_columns is not None:
                    if args.max_relation_columns < 0:
                        raise ValueError("max relation columns must be nonnegative (0 means unlimited)")
                    command += ["--master-max-relation-columns", str(args.max_relation_columns)]
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
            elif args.action == "finalize-refinement":
                command.append("--finalize-refinement")
            elif args.action == "publish":
                command.append("--publish-only")
            if args.action in ("run", "extend", "publish") and args.deep_verification is not None:
                command.append("--deep-verification" if args.deep_verification else "--no-deep-verification")
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
