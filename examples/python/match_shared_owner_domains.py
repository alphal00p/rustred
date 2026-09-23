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


ALLOWANCES = (
    "max-queries", "max-total-pieces", "max-rules-per-query",
    "max-terminal-checks-per-query", "max-predicates-per-query",
    "max-pieces-per-query", "max-cells-per-query",
    "max-split-operations-per-query", "max-coordinate-cells-per-query",
    "max-guard-univariate-degree",
)
REFINEMENT = "max-bounded-refinement-cells-per-query"
REFINEMENT_AXES = "bounded-refinement-axes"
REFINEMENT_AXIS_CHOICES = ("inactive-only", "finite-axes")
TRANSFER_LOOKAHEAD = "transfer-unreserved-lookahead"
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


def containment_limit(text: str) -> int | str:
    # None remains the omitted-option sentinel, so explicit unlimited still
    # receives scope validation and is forwarded to the native parser.
    if text == "unlimited":
        return text
    return positive(text)


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
    parser.add_argument("--follow-successors", action="store_true",
                        help="share symbolic successor domains; unresolved routes remain explicit")
    parser.add_argument("--route-domain-overcover", action="store_true",
                        help="share admitted route rank overcovers without expanding numerator polynomials")
    parser.add_argument("--max-route-masks-per-query", type=positive)
    for option in ALLOWANCES:
        parser.add_argument("--" + option, type=positive,
                            help="optional native work/storage allowance, not a rank restriction")
    parser.add_argument("--" + REFINEMENT, type=nonnegative,
                        help="exact bounded refinement faces per query; zero disables refinement")
    parser.add_argument("--" + REFINEMENT_AXES, choices=REFINEMENT_AXIS_CHOICES,
                        help="local refinement axes (native default: inactive-only); finite-axes also permits explicitly bounded positive axes, not routing or closure")
    parser.add_argument("--" + TRANSFER_LOOKAHEAD, type=positive,
                        help="opt into unreserved containment delegation with fixed logical dispatch lookahead; requires unlimited containment checks")
    for option in WALK_ALLOWANCES:
        parser.add_argument("--" + option,
                            type=containment_limit if option == "max-containment-checks" else positive,
                            help="positive diagnostic cap or unlimited (default)" if
                            option == "max-containment-checks" else None)
    args = parser.parse_args()
    if not args.follow_successors and (args.route_domain_overcover or any(
            getattr(args, option.replace("-", "_")) is not None
            for option in (*WALK_ALLOWANCES, TRANSFER_LOOKAHEAD, "max-route-masks-per-query"))):
        parser.error("successor work allowances require --follow-successors")
    if args.max_route_masks_per_query is not None and not args.route_domain_overcover:
        parser.error("route mask allowance requires --route-domain-overcover")
    if args.transfer_unreserved_lookahead is not None and args.max_containment_checks not in (None, "unlimited"):
        parser.error("--transfer-unreserved-lookahead requires unlimited containment checks")
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
    if args.route_domain_overcover:
        command.append("--route-domain-overcover")
    for option in (*ALLOWANCES, REFINEMENT, REFINEMENT_AXES, *WALK_ALLOWANCES, TRANSFER_LOOKAHEAD, "max-route-masks-per-query"):
        if (value := getattr(args, option.replace("-", "_"))) is not None:
            command.extend(["--" + option, str(value)])
    # Inherit the license without persisting or printing it. Replacement keeps
    # caller affinity/limits and exit status; no extra pool or elapsed deadline.
    os.execve(command[0], command, environment)


if __name__ == "__main__":
    main()
