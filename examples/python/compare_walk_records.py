#!/usr/bin/env python3
"""Compare two owner-domain walk reports (result.json) record by record.

strict   Ordered old-binary vs new-binary control: identical completed record
         geometry, native/guard/dependency counters and outcomes, ignoring only
         timing (seconds), checkpoint bookkeeping and scheduling diagnostics.
multiset Ready or cross-policy control: equal multisets of
         (phase, owner, lower, upper, rank, power_bounds, outcome) and native
         inspection counts within a stated relative tolerance. `--shape`
         selects the outcome component: `discharged` (default; error-free and
         obligation discharged, independent of whether the domain was
         inspected natively or delegated), `kind` (adds record kind and
         responsibility, only meaningful within one publication policy) or
         `geometry` (no outcome at all).

Both files are streamed; multiset mode keeps one 16-byte digest per distinct
record shape. Neither mode certifies family closure; run the audit first.

A comparison is INVALID (exit 3), whatever the records say, when either walk
has no domain record (a run that never started compares equal to another one),
when either walk reports an `error` (or an `*_error` status), or when a
`metrics.json` beside either result.json (written by run_control.py) records a
nonzero `exit_code` for a walk whose status is not `incomplete` or `paused`
(both exit 4 by design). `--allow-empty` waives only the first rule and
`--allow-failed` only the other two.
Exit status: 0 PASS, 1 FAIL, 2 usage or unreadable input, 3 INVALID.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

_AUDIT_SPEC = importlib.util.spec_from_file_location(
    "audit_owner_domain_walk", Path(__file__).with_name("audit_owner_domain_walk.py"))
AUDIT = importlib.util.module_from_spec(_AUDIT_SPEC)
_AUDIT_SPEC.loader.exec_module(AUDIT)

IGNORED_TOP_LEVEL = frozenset({"checkpoint", "parallel", "worker_allocation", "workers",
                               "requested_inspection_workers", "scheduling_policy", "timing_scope",
                               "traversal_timing_boundary", "resume_supported", "domains",
                               # G2' anchor-index telemetry: run structure and timing, not walk state.
                               "g2_index_telemetry"})
# Closure-monitor telemetry driven by wall time (refresh cadence and scratch/storage
# estimates); the semantic closure keys (edges, revisions, closed counts) stay compared.
CLOSURE_TELEMETRY = frozenset({"refresh_count", "refresh_scratch_estimate_bytes",
                               "retained_storage_estimate_bytes"})
GEOMETRY_FIELDS = ("phase", "owner", "lower", "upper", "rank", "power_bounds")
KIND_FIELDS = ("record_kind", "responsibility_status", "local_inspection_finished",
               "residual_inspection_finished")
DISCHARGED_STATUS = ("discharged_by_representative", "discharged_by_residual_and_initial_anchor",
                     "discharged_by_residual_and_g2_anchors")
SHAPES = ("geometry", "discharged", "kind")
EXAMPLE_LIMIT = 20
RETAINED_SHAPES = 100_000


def is_timing(key):
    return key == "seconds" or key.endswith("_seconds") or key.endswith("_unix_time")


def strip(value, ignored=frozenset()):
    """Drop timing keys (and explicitly ignored keys) recursively."""
    if isinstance(value, dict):
        return {key: strip(item, ignored) for key, item in value.items()
                if not is_timing(key) and key not in ignored}
    if isinstance(value, list):
        return [strip(item, ignored) for item in value]
    return value


def brief(value, limit=160):
    text = json.dumps(value, sort_keys=True, default=str)
    return text if len(text) <= limit else text[:limit - 1] + "…"


def record_shape(record, shape="discharged"):
    parts = [record.get(field) for field in GEOMETRY_FIELDS]
    if shape != "geometry":
        outcome = {"error_is_none": record.get("error") is None,
                   "frontiers": len(record.get("frontiers") or []),
                   "discharged": record.get("local_inspection_finished") is True
                   or record.get("responsibility_status") in DISCHARGED_STATUS}
        if shape == "kind":
            outcome.update({field: record.get(field) for field in KIND_FIELDS})
        parts.append(outcome)
    text = json.dumps(parts, sort_keys=True, separators=(",", ":"), default=str)
    return hashlib.sha256(text.encode("utf-8")).digest()[:16], text


class Walk:
    """Streams one result.json, yielding domain records and collecting scalar top-level entries."""

    def __init__(self, path):
        self.path = Path(path)
        self.top = {}
        self.sha256 = None
        self.records = 0
        self.native_by_phase = Counter()
        self._events = AUDIT.stream_walk(self.path)

    def next_record(self):
        for item in self._events:
            if item[0] == "domain":
                self.records += 1
                record = item[1]
                if record.get("record_kind") != "delegated_not_inspected":
                    self.native_by_phase[record.get("phase")] += 1
                return record
            if item[0] == "top":
                self.top[item[1]] = item[2]
            elif item[0] == "sha256":
                self.sha256 = item[1]
        return None

    def drain(self):
        while self.next_record() is not None:
            pass


def compare_top_level(first, second, ignored):
    ignored = IGNORED_TOP_LEVEL | set(ignored)
    left = strip({key: value for key, value in first.items() if key not in ignored})
    right = strip({key: value for key, value in second.items() if key not in ignored})
    for side in (left, right):
        if isinstance(side.get("descendant_closure"), dict):
            side["descendant_closure"] = strip(side["descendant_closure"], CLOSURE_TELEMETRY)
    differences = {}
    for key in sorted(set(left) | set(right)):
        if key not in left or key not in right or left[key] != right[key]:
            differences[key] = {"a": brief(left.get(key, "<absent>")), "b": brief(right.get(key, "<absent>"))}
    return differences


# Walk statuses that end with exit 4 by design (a finished walk with an open
# classification, a cooperative pause); any other nonzero exit voids a run.
EXIT4_STATUSES = frozenset({"incomplete", "paused"})


def invalid_reasons(first, second, allow_empty=False, allow_failed=False):
    """Why a comparison of these two drained walks would be void (empty: valid)."""
    reasons = []
    for side, walk in (("a", first), ("b", second)):
        if walk.records == 0 and not allow_empty:
            reasons.append(f"{side}: 0 domain records in {walk.path}")
        if allow_failed:
            continue
        status, error = walk.top.get("status"), walk.top.get("error")
        if error not in (None, "") or str(status).endswith("error"):
            reasons.append(f"{side}: walk status {status!r}, error {brief(error)}")
        metrics = walk.path.with_name("metrics.json")
        if metrics.is_file():
            try:
                code = json.loads(metrics.read_text()).get("exit_code")
            except (OSError, ValueError, AttributeError) as failure:
                reasons.append(f"{side}: unreadable {metrics}: {failure}")
                continue
            if code not in (0, None) and not (code == 4 and status in EXIT4_STATUSES):
                reasons.append(f"{side}: {metrics} has exit_code {code} with walk status {status!r}")
    return reasons


def verdict_of(passed, reasons):
    return "INVALID" if reasons else ("PASS" if passed else "FAIL")


def strict_compare(first_path, second_path, ignore_top=(), ignore_record=(), allow_empty=False,
                   allow_failed=False):
    first, second = Walk(first_path), Walk(second_path)
    ignored_record = frozenset(ignore_record)
    differing = 0
    examples = []
    while True:
        left, right = first.next_record(), second.next_record()
        if left is None and right is None:
            break
        if left is None or right is None:
            first.drain()
            second.drain()
            break
        stripped_left, stripped_right = strip(left, ignored_record), strip(right, ignored_record)
        if stripped_left != stripped_right:
            differing += 1
            if len(examples) < EXAMPLE_LIMIT:
                keys = sorted(key for key in set(stripped_left) | set(stripped_right)
                              if stripped_left.get(key, "<absent>") != stripped_right.get(key, "<absent>"))
                examples.append({"position": first.records - 1, "id_a": left.get("id"), "id_b": right.get("id"),
                                 "differing_keys": keys[:12]})
    top_differences = compare_top_level(first.top, second.top, ignore_top)
    equal = first.records == second.records and differing == 0 and not top_differences
    reasons = invalid_reasons(first, second, allow_empty, allow_failed)
    return {"mode": "strict", "verdict": verdict_of(equal, reasons), "identical": equal,
            "invalid_reasons": reasons,
            "a": {"path": str(first.path), "sha256": first.sha256, "records": first.records,
                  "native_by_phase": dict(first.native_by_phase)},
            "b": {"path": str(second.path), "sha256": second.sha256, "records": second.records,
                  "native_by_phase": dict(second.native_by_phase)},
            "differing_records": differing, "record_examples": examples,
            "top_level_differences": top_differences,
            "ignored": {"top_level": sorted(IGNORED_TOP_LEVEL | set(ignore_top)), "record": sorted(ignored_record),
                        "timing_keys": "seconds, *_seconds, *_unix_time (recursively)",
                        "closure_telemetry": sorted(CLOSURE_TELEMETRY)},
            "family_closure_claim": False}


def within(a, b, tolerance):
    if a is None or b is None:
        return None
    return abs(a - b) <= tolerance * max(a, b, 1)


def multiset_compare(first_path, second_path, native_tolerance=0.0, shape="discharged",
                     allow_empty=False, allow_failed=False):
    if shape not in SHAPES:
        raise ValueError(f"shape must be one of {SHAPES}")
    first, second = Walk(first_path), Walk(second_path)
    counts = Counter()
    retained = {}
    kinds = {"a": Counter(), "b": Counter()}
    for walk, sign, side in ((first, 1, "a"), (second, -1, "b")):
        while (record := walk.next_record()) is not None:
            kinds[side][record.get("record_kind")] += 1
            key, text = record_shape(record, shape)
            counts[key] += sign
            if len(retained) < RETAINED_SHAPES:
                retained.setdefault(key, text)
    only_a = {key: value for key, value in counts.items() if value > 0}
    only_b = {key: -value for key, value in counts.items() if value < 0}
    native_a, native_b = first.top.get("completed_nodes"), second.top.get("completed_nodes")
    phase_checks = {}
    for phase in sorted(set(first.native_by_phase) | set(second.native_by_phase)):
        a, b = first.native_by_phase.get(phase, 0), second.native_by_phase.get(phase, 0)
        phase_checks[phase] = {"a": a, "b": b, "within_tolerance": within(a, b, native_tolerance)}
    native_ok = within(native_a, native_b, native_tolerance)
    equal_multisets = not only_a and not only_b
    verdict = equal_multisets and native_ok is True and all(row["within_tolerance"] for row in phase_checks.values())
    reasons = invalid_reasons(first, second, allow_empty, allow_failed)
    return {"mode": "multiset", "verdict": verdict_of(verdict, reasons), "equal_multisets": equal_multisets,
            "invalid_reasons": reasons,
            "a": {"path": str(first.path), "sha256": first.sha256, "records": first.records,
                  "native_inspections": native_a, "native_by_phase": dict(first.native_by_phase)},
            "b": {"path": str(second.path), "sha256": second.sha256, "records": second.records,
                  "native_inspections": native_b, "native_by_phase": dict(second.native_by_phase)},
            "shapes_only_in_a": sum(only_a.values()), "shapes_only_in_b": sum(only_b.values()),
            "examples_only_in_a": [retained.get(key, "<unretained shape>") for key in list(only_a)[:EXAMPLE_LIMIT]],
            "examples_only_in_b": [retained.get(key, "<unretained shape>") for key in list(only_b)[:EXAMPLE_LIMIT]],
            "native_tolerance": native_tolerance, "native_within_tolerance": native_ok,
            "native_by_phase": phase_checks, "record_kinds": {side: dict(counter) for side, counter in kinds.items()},
            "shape": shape, "shape_fields": list(GEOMETRY_FIELDS) + ([] if shape == "geometry" else ["outcome"]),
            "family_closure_claim": False}


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--mode", choices=("strict", "multiset"), required=True)
    parser.add_argument("a", type=Path, help="first result.json")
    parser.add_argument("b", type=Path, help="second result.json")
    parser.add_argument("--native-tolerance", type=float, default=0.0,
                        help="multiset: admitted relative difference of native inspection counts (default 0)")
    parser.add_argument("--shape", choices=SHAPES, default="discharged",
                        help="multiset: outcome component of the record shape (default discharged)")
    parser.add_argument("--ignore-top", action="append", default=[], metavar="KEY",
                        help="strict: additional top-level key to ignore; repeatable")
    parser.add_argument("--ignore-record", action="append", default=[], metavar="KEY",
                        help="strict: additional per-record key to ignore; repeatable")
    parser.add_argument("--output", type=Path, help="write the comparison JSON here as well")
    parser.add_argument("--allow-empty", action="store_true",
                        help="do not treat a walk without domain records as INVALID")
    parser.add_argument("--allow-failed", action="store_true",
                        help="do not treat a walk error or a failed run's exit_code as INVALID")
    args = parser.parse_args(argv)
    if not 0 <= args.native_tolerance < 1:
        parser.error("native tolerance must be in [0, 1)")
    try:
        if args.mode == "strict":
            report = strict_compare(args.a, args.b, args.ignore_top, args.ignore_record, args.allow_empty,
                                    args.allow_failed)
        else:
            report = multiset_compare(args.a, args.b, args.native_tolerance, args.shape, args.allow_empty,
                                      args.allow_failed)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    text = json.dumps(report, indent=2, sort_keys=True, allow_nan=False)
    if args.output is not None:
        args.output.write_text(text + "\n")
    print(text)
    return {"PASS": 0, "FAIL": 1}.get(report["verdict"], 3)


if __name__ == "__main__":
    raise SystemExit(main())
