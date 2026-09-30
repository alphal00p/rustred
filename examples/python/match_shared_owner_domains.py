#!/usr/bin/env python3
"""Classify explicit parametric boxes using saved owner rules via Rust.

Python only forwards inputs and resource policy. Exit zero means exact local
classification, which may contain gaps or invalid source conditions; it does
not imply applicability, RHS reduction or recursive closure. With
--follow-successors, zero instead requires all scheduled local domains resolved;
routing/guard frontiers remain incomplete. Neither mode claims family closure.
No IBPs are generated. Each JSON query supplies its own rank cap (null means unbounded).
"""
import argparse
import os
from pathlib import Path
import sys


ALLOWANCES = (
    "max-queries", "max-query-bytes", "max-total-pieces", "max-rules-per-query",
    "max-terminal-checks-per-query", "max-predicates-per-query",
    "max-pieces-per-query", "max-cells-per-query",
    "max-split-operations-per-query", "max-coordinate-cells-per-query",
    "max-guard-univariate-degree",
)
REFINEMENT = "max-bounded-refinement-cells-per-query"
QUERY_ALLOWANCES = ("max-queries", "max-query-bytes")
REFINEMENT_AXES = "bounded-refinement-axes"
REFINEMENT_AXIS_CHOICES = ("inactive-only", "finite-axes")
TRANSFER_LOOKAHEAD = "transfer-unreserved-lookahead"
INITIAL_D_REUSE = "reuse-initial-d-bands"
JOINT_SUPPORT_PRUNING = "route-joint-source-support-pruning"
PUBLICATION_POLICY = "publication-policy"
PUBLICATION_POLICIES = ("ordered", "owner-batched", "ready", "epoch")
EPOCH_INSPECTOR_LOOKUP = "epoch-inspector-lookup"
EPOCH_INSPECTOR_LOOKUP_MODES = ("all-miss", "snapshot")
EPOCH_ROLLING = "epoch-rolling"
EPOCH_DISPATCH = "epoch-dispatch"
EPOCH_DISPATCH_POLICIES = ("fifo", "adaptive")
EPOCH_PUBLICATION_ORDER = "epoch-publication-order"
EPOCH_PUBLICATION_ORDERS = ("oldest-prefix", "oldest-ready")
EPOCH_CUT_SIZE = "epoch-cut-size"
EPOCH_WINDOW = "epoch-window"
EPOCH_BATCH_OPTIONS = (EPOCH_PUBLICATION_ORDER, EPOCH_CUT_SIZE, EPOCH_WINDOW)
EPOCH_PREPARATION_OPTIONS = ("epoch-preparation-workers", "epoch-preparation-max-obligations",
                             "epoch-preparation-max-retirements")
EPOCH_ESCROW_OPTIONS = ("epoch-result-escrow-jobs", "epoch-result-escrow-bytes")
EPOCH_DATA_OPTIONS = (*EPOCH_BATCH_OPTIONS, *EPOCH_PREPARATION_OPTIONS, *EPOCH_ESCROW_OPTIONS)
INSPECTION_WORKERS = "inspection-workers"
APPLICATION_REFINEMENT = "apply-cell-refinement-max-cardinality"
FRONTIER_POLICY = "frontier-policy"
FRONTIER_POLICIES = ("record", "stop")
WALK_ALLOWANCES = ("workers", "max-domains", "max-frontiers", "max-successor-events", "max-containment-checks",
                   "max-rhs-cells-per-query", "max-term-visits-per-query",
                   "max-native-operations-per-query", "max-rhs-events-per-query",
                   "max-shift-groups-per-query", "max-sign-splits-per-query")


def positive(text: str) -> int:
    if not text.isascii() or not text.isdecimal() or int(text) == 0:
        raise argparse.ArgumentTypeError("work allowance must be a positive integer")
    return int(text)


def nonnegative(text: str) -> int:
    if not text.isascii() or not text.isdecimal():
        raise argparse.ArgumentTypeError("refinement allowance must be a nonnegative integer")
    return int(text)


def query_allowance(text: str) -> int:
    value = positive(text)
    if value > 2 * sys.maxsize + 1:
        raise argparse.ArgumentTypeError("query allowance must fit the native unsigned pointer-sized integer")
    return value


def application_cardinality(text: str) -> int:
    value = positive(text)
    if value > 2 * sys.maxsize + 1:
        raise argparse.ArgumentTypeError("application cardinality must fit the native unsigned pointer-sized integer")
    return value


def containment_limit(text: str) -> int | str:
    # None remains the omitted-option sentinel, so explicit unlimited still
    # receives scope validation and is forwarded to the native parser.
    if text == "unlimited":
        return text
    return positive(text)


class StoreTrueOnce(argparse.Action):
    """Reject repeated opt-in flags rather than silently hiding duplicates."""
    def __call__(self, parser, namespace, values, option_string=None):
        if getattr(namespace, self.dest, False):
            parser.error(f"{option_string} may be supplied only once")
        setattr(namespace, self.dest, True)


class StoreOnce(argparse.Action):
    """Keep an explicit scalar policy from silently overriding itself."""
    def __call__(self, parser, namespace, value, option_string=None):
        if getattr(namespace, self.dest, None) is not None:
            parser.error(f"{option_string} may be supplied only once")
        setattr(namespace, self.dest, value)


def validate_inspection_workers(parser, workers, inspectors, containment_cap):
    if inspectors is None:
        return
    available = 1 if workers == 1 else workers - 1
    if not 1 <= inspectors <= available:
        parser.error("inspection workers must be positive and leave one coordinator when workers > 1")
    if containment_cap not in (None, "unlimited") and inspectors != available:
        parser.error("finite containment cap requires all non-coordinator workers for inspection")


def preparation_count(text):
    value = positive(text)
    if value > (1 << 32) - 1:
        raise argparse.ArgumentTypeError("Epoch preparation counts must be in 1..=u32::MAX")
    return value


def add_epoch_preparation_arguments(parser):
    parser.add_argument("--epoch-preparation-workers", type=nonnegative, action=StoreOnce,
                        help="P2 helpers inside --workers, not extra threads; zero selects serial preparation")
    for option in EPOCH_PREPARATION_OPTIONS[1:]:
        parser.add_argument("--" + option, type=preparation_count, action=StoreOnce,
                            help="per-cut logical scratch/output count, not RAM bytes or total work; default u32::MAX")


def validate_epoch_preparation(options, symbolic, publication, workers, inspectors=None, containment_cap=None):
    """Shared thin-driver validation; Rust validates again before loading inputs."""
    values = [options.get(name.replace("-", "_")) for name in EPOCH_PREPARATION_OPTIONS]
    if not any(value is not None for value in values):
        return
    if not symbolic or publication != "epoch":
        raise ValueError("Epoch preparation options require a symbolic walk with epoch publication")
    helpers, *counts = values
    for value in counts:
        if value is not None and (type(value) is not int or not 1 <= value < 1 << 32):
            raise ValueError("Epoch preparation counts must be in 1..=u32::MAX")
    if helpers is None:
        return
    available = 1 if workers == 1 else workers - 1
    if type(helpers) is not int or not 0 <= helpers < available:
        raise ValueError("Epoch preparation workers must leave one inspector within the total budget")
    if inspectors is not None and inspectors != available - helpers:
        raise ValueError("Epoch preparation and inspection workers must exactly partition the non-coordinator budget")
    if containment_cap not in (None, "unlimited") and helpers:
        raise ValueError("finite containment cap requires all non-coordinator workers for inspection")


def validate_publication_policy(parser, policy, transfer_lookahead, checkpoint, subdivision):
    if policy in ("ready", "epoch") and transfer_lookahead is None:
        parser.error(f"{policy} publication requires --transfer-unreserved-lookahead")
    if checkpoint and policy == "owner-batched":
        parser.error("checkpoint/resume requires ordered, ready or epoch publication")
    if subdivision and policy not in (None, "ordered"):
        parser.error("physical subdivision requires ordered publication")


def validate_frontier_policy(parser, policy, checkpoint):
    """A10: stop saves and stops at the first frontier, so it needs a checkpoint."""
    if policy == "stop" and not checkpoint:
        parser.error("--frontier-policy stop requires --checkpoint or --resume")


def validate_epoch_inspector_lookup(mode, symbolic, publication, checkpoint):
    if mode is None:
        return
    if mode not in EPOCH_INSPECTOR_LOOKUP_MODES:
        raise ValueError("epoch inspector lookup must be all-miss or snapshot")
    if not symbolic or publication != "epoch" or not checkpoint:
        raise ValueError("--epoch-inspector-lookup requires a symbolic successor walk, "
                         "--publication-policy epoch and --checkpoint or --resume")


def validate_epoch_rolling(enabled, symbolic, publication, checkpoint):
    if enabled and (not symbolic or publication != "epoch" or not checkpoint):
        raise ValueError("--epoch-rolling requires a symbolic successor walk, "
                         "--publication-policy epoch and --checkpoint or --resume")


def validate_epoch_dispatch(policy, rolling, symbolic, publication, checkpoint):
    if policy is None:
        return
    if policy not in EPOCH_DISPATCH_POLICIES:
        raise ValueError("epoch dispatch must be fifo or adaptive")
    if not symbolic or publication != "epoch" or not checkpoint:
        raise ValueError("--epoch-dispatch requires a symbolic successor walk, "
                         "--publication-policy epoch and --checkpoint or --resume")
    if policy == "adaptive" and not rolling:
        raise ValueError("--epoch-dispatch adaptive requires --epoch-rolling")


def epoch_batch_size(text):
    value = positive(text)
    if value > 4096:
        raise argparse.ArgumentTypeError("Epoch cut/window must be in 1..4096")
    return value


def add_epoch_batch_arguments(parser):
    parser.add_argument("--" + EPOCH_PUBLICATION_ORDER, choices=EPOCH_PUBLICATION_ORDERS,
                        action=StoreOnce, help="rolling merge order; default oldest-prefix; frozen on resume")
    parser.add_argument("--" + EPOCH_CUT_SIZE, type=epoch_batch_size, action=StoreOnce,
                        help="rolling publication batch size (default 16); frozen on resume")
    parser.add_argument("--" + EPOCH_WINDOW, type=epoch_batch_size, action=StoreOnce,
                        help="rolling unmerged-job bound; omitted resume inherits the saved bound")
    parser.add_argument("--epoch-result-escrow-jobs", type=nonnegative, action=StoreOnce,
                        help="extra logical reservations for complete results; default zero, no extra threads")
    parser.add_argument("--epoch-result-escrow-bytes", type=query_allowance, action=StoreOnce,
                        help="positive returned-result byte admission budget; not a hard RSS cap")


def validate_epoch_batch(publication_order, cut_size, window, rolling, symbolic, publication, checkpoint,
                         escrow_jobs=None, escrow_bytes=None):
    if escrow_jobs is not None and (type(escrow_jobs) is not int or not 0 <= escrow_jobs < 4096):
        raise ValueError("Epoch result escrow jobs must be an integer in 0..4095")
    jobs = escrow_jobs or 0
    if jobs:
        if type(escrow_bytes) is not int or not 1 <= escrow_bytes <= 2 * sys.maxsize + 1:
            raise ValueError("Epoch result escrow requires a positive native-sized byte admission budget")
        if publication_order not in (None, "oldest-prefix"):
            raise ValueError("Epoch result escrow requires oldest-prefix publication")
        if window is not None and window + jobs > 4096:
            raise ValueError("Epoch base window plus escrow jobs must be at most 4096")
    elif escrow_bytes is not None:
        raise ValueError("Epoch result escrow bytes require positive escrow jobs")
    if publication_order is None and cut_size is None and window is None and not jobs:
        return
    if not rolling or not symbolic or publication != "epoch" or not checkpoint:
        raise ValueError("Epoch publication order, cut size and window require a symbolic rolling "
                         "Epoch walk with --checkpoint or --resume")
    if publication_order not in (None, *EPOCH_PUBLICATION_ORDERS):
        raise ValueError("Epoch publication order must be oldest-prefix or oldest-ready")
    for value in (cut_size, window):
        if value is not None and (type(value) is not int or not 1 <= value <= 4096):
            raise ValueError("Epoch cut/window must be integers in 1..4096")
    if window is not None and window < (cut_size or 16):
        raise ValueError("Epoch window must be at least the cut size")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--queries", type=Path, required=True)
    parser.add_argument("--owner-base", type=Path, default=Path("."))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--events", type=Path)
    parser.add_argument("--stop-file", type=Path)
    checkpoints = parser.add_mutually_exclusive_group()
    checkpoints.add_argument("--checkpoint", type=Path)
    checkpoints.add_argument("--resume", type=Path)
    parser.add_argument("--checkpoint-interval-seconds", type=positive)
    parser.add_argument("--unbounded-work", action=StoreTrueOnce, nargs=0, default=False)
    parser.add_argument("--apply-subdivision-axis", type=nonnegative, action=StoreOnce)
    parser.add_argument("--apply-subdivision-cut", type=nonnegative, action=StoreOnce)
    parser.add_argument("--no-progress", action="store_true")
    parser.add_argument("--follow-successors", action="store_true",
                        help="share symbolic successor domains; unresolved routes remain explicit")
    parser.add_argument("--route-domain-overcover", action="store_true",
                        help="share admitted route rank overcovers without expanding numerator polynomials")
    parser.add_argument("--" + JOINT_SUPPORT_PRUNING, action=StoreTrueOnce, nargs=0, default=False,
                        help="opt into necessary joint source-support mask pruning; requires route overcover; default off")
    parser.add_argument("--max-route-masks-per-query", type=positive)
    for option in ALLOWANCES:
        parser.add_argument("--" + option,
                            type=query_allowance if option in QUERY_ALLOWANCES else positive,
                            action=StoreOnce if option in QUERY_ALLOWANCES else "store",
                            help="optional native work/storage allowance, not a rank restriction")
    parser.add_argument("--" + REFINEMENT, type=nonnegative,
                        help="exact bounded refinement faces per query; zero disables refinement")
    parser.add_argument("--" + REFINEMENT_AXES, choices=REFINEMENT_AXIS_CHOICES,
                        help="local refinement axes (native default: inactive-only); finite-axes also permits explicitly bounded positive axes, not routing or closure")
    parser.add_argument("--" + TRANSFER_LOOKAHEAD, type=positive,
                        help="opt into unreserved containment delegation with fixed logical dispatch lookahead; requires unlimited containment checks")
    parser.add_argument("--" + INITIAL_D_REUSE, action=StoreTrueOnce, nargs=0, default=False,
                        help="reuse an exact initial same-owner D band, retaining its obligation; requires successor walk and unreserved delegation")
    parser.add_argument("--" + PUBLICATION_POLICY, choices=PUBLICATION_POLICIES,
                        help="successor publication: ordered (default), owner-batched, ready or epoch; ready/epoch require unreserved delegation; epoch checkpoints use CP6")
    parser.add_argument("--" + EPOCH_INSPECTOR_LOOKUP, choices=EPOCH_INSPECTOR_LOOKUP_MODES,
                        action=StoreOnce, help="CP6 Epoch comparison control; default all-miss; frozen on resume")
    parser.add_argument("--" + EPOCH_ROLLING, action=StoreTrueOnce, nargs=0, default=False,
                        help="opt into bounded rolling CP6 execution; frozen on resume")
    parser.add_argument("--" + EPOCH_DISPATCH, choices=EPOCH_DISPATCH_POLICIES, action=StoreOnce,
                        help="pending-job dispatch: fifo (default) or adaptive; adaptive requires rolling")
    add_epoch_batch_arguments(parser)
    add_epoch_preparation_arguments(parser)
    parser.add_argument("--" + INSPECTION_WORKERS, type=positive, action=StoreOnce,
                        help="explicit partition: N inspectors, workers-1-N admission helpers and one coordinator; one worker stays inline; requires successor walk")
    parser.add_argument("--" + APPLICATION_REFINEMENT, type=application_cardinality, action=StoreOnce,
                        help="opt into singleton refinement of one finite varying selected Apply-cell axis up to this cardinality; default off; not a cumulative work cap; requires successor walk")
    parser.add_argument("--" + FRONTIER_POLICY, choices=FRONTIER_POLICIES, action=StoreOnce,
                        help="record (native default) keeps walking past explicit frontiers; stop saves the "
                             "checkpoint and stops at the first new frontier (exit 4); requires a checkpoint")
    for option in WALK_ALLOWANCES:
        parser.add_argument("--" + option,
                            type=containment_limit if option == "max-containment-checks" else positive,
                            help="positive diagnostic cap or unlimited (default)" if
                            option == "max-containment-checks" else None)
    args = parser.parse_args()
    if not args.follow_successors and (args.checkpoint is not None or args.resume is not None
            or args.checkpoint_interval_seconds is not None or args.unbounded_work
            or args.apply_subdivision_axis is not None or args.apply_subdivision_cut is not None):
        parser.error("checkpoint, unbounded work and subdivision require --follow-successors")
    if args.checkpoint_interval_seconds is not None and args.checkpoint is None and args.resume is None:
        parser.error("checkpoint interval requires --checkpoint or --resume")
    if (args.apply_subdivision_axis is None) != (args.apply_subdivision_cut is None):
        parser.error("subdivision requires both axis and cut")
    if not args.follow_successors and (args.route_domain_overcover or args.route_joint_source_support_pruning or args.reuse_initial_d_bands or any(
            getattr(args, option.replace("-", "_")) is not None
            for option in (*WALK_ALLOWANCES, TRANSFER_LOOKAHEAD, PUBLICATION_POLICY, INSPECTION_WORKERS, APPLICATION_REFINEMENT,
                           FRONTIER_POLICY, "max-route-masks-per-query"))):
        parser.error("successor work allowances require --follow-successors")
    if args.max_route_masks_per_query is not None and not args.route_domain_overcover:
        parser.error("route mask allowance requires --route-domain-overcover")
    if args.route_joint_source_support_pruning and not args.route_domain_overcover:
        parser.error("joint source-support pruning requires --route-domain-overcover")
    if args.transfer_unreserved_lookahead is not None and args.max_containment_checks not in (None, "unlimited"):
        parser.error("--transfer-unreserved-lookahead requires unlimited containment checks")
    if args.reuse_initial_d_bands and args.transfer_unreserved_lookahead is None:
        parser.error("--reuse-initial-d-bands requires --transfer-unreserved-lookahead")
    validate_publication_policy(parser, args.publication_policy, args.transfer_unreserved_lookahead,
                                args.checkpoint is not None or args.resume is not None,
                                args.apply_subdivision_axis is not None)
    try:
        validate_epoch_inspector_lookup(args.epoch_inspector_lookup, args.follow_successors,
                                        args.publication_policy, args.checkpoint is not None or args.resume is not None)
        validate_epoch_rolling(args.epoch_rolling, args.follow_successors,
                               args.publication_policy, args.checkpoint is not None or args.resume is not None)
        validate_epoch_dispatch(args.epoch_dispatch, args.epoch_rolling, args.follow_successors,
                                args.publication_policy, args.checkpoint is not None or args.resume is not None)
        validate_epoch_batch(args.epoch_publication_order, args.epoch_cut_size, args.epoch_window,
                             args.epoch_rolling, args.follow_successors, args.publication_policy,
                             args.checkpoint is not None or args.resume is not None,
                             args.epoch_result_escrow_jobs, args.epoch_result_escrow_bytes)
        validate_epoch_preparation(vars(args), args.follow_successors, args.publication_policy,
                                   args.workers or 1, args.inspection_workers, args.max_containment_checks)
    except ValueError as error:
        parser.error(str(error))
    validate_inspection_workers(parser, args.workers or 1, args.inspection_workers,
                                args.max_containment_checks)
    validate_frontier_policy(parser, args.frontier_policy, args.checkpoint is not None or args.resume is not None)
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
    if args.follow_successors:
        command.append("--follow-successors")
    if args.epoch_rolling:
        command.append("--" + EPOCH_ROLLING)
    if args.epoch_dispatch == "adaptive":
        command += ["--" + EPOCH_DISPATCH, "adaptive"]
    if args.route_domain_overcover:
        command.append("--route-domain-overcover")
    if args.route_joint_source_support_pruning:
        command.append("--" + JOINT_SUPPORT_PRUNING)
    if args.reuse_initial_d_bands:
        command.append("--" + INITIAL_D_REUSE)
    if args.unbounded_work:
        command.append("--unbounded-work")
    for option in ("checkpoint", "resume", "checkpoint_interval_seconds",
                   "apply_subdivision_axis", "apply_subdivision_cut"):
        if (value := getattr(args, option)) is not None:
            command += ["--" + option.replace("_", "-"), str(value)]
    for option in (*ALLOWANCES, REFINEMENT, REFINEMENT_AXES, *WALK_ALLOWANCES, TRANSFER_LOOKAHEAD, PUBLICATION_POLICY, EPOCH_INSPECTOR_LOOKUP, *EPOCH_DATA_OPTIONS, INSPECTION_WORKERS, APPLICATION_REFINEMENT,
                   FRONTIER_POLICY, "max-route-masks-per-query"):
        if (value := getattr(args, option.replace("-", "_"))) is not None:
            command.extend(["--" + option, str(value)])
    # Inherit the license without persisting or printing it. Replacement keeps
    # caller affinity/limits and exit status; no extra pool or elapsed deadline.
    os.execve(command[0], command, environment)


if __name__ == "__main__":
    main()
