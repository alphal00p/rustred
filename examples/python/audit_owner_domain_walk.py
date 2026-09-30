#!/usr/bin/env python3
"""Streaming, phase-aware audit of an exhausted Ordered/Ready/Epoch owner-domain walk.

Every logical record of `result.json` is streamed once with bounded memory:
aliases must resolve to a same-phase, same-owner completed native
representative; Apply/Route native statistics must be internally consistent
(zero problems, zero missing routes, successor sums); the queue, delegation
ledger and worker pool must be drained; frontiers must be zero; initial and
partial-anchor obligations must be discharged; the input queries must be
preserved: `inputs` follows the query document order, each distinct initial
record equals the first query naming it (the one that admitted it), and a later
query may only alias into an earlier same-owner initial record that
syntactically contains it (helpers-first query documents). Every alias is
re-checked to be an exact integer-set subset of its direct representative, and
every partial record's D >= cut slice of its initial anchor, with an
independent interval predicate cross-checked by lattice-point enumeration on
small sets; committed records must have accepted exactly their stream's
events; retained frontiers, failures and refusal provenance must be explicit
in records (parity); no record carrying a frontier or error may be reported
descendant-closed; and helper roots and physics queries are reported
separately. `--require-closure` additionally requires every root and record to
be descendant-closed. Violations are collected, written to `audit.json` and
cause a nonzero exit. This checks recorded native completion and explicit
dependencies only; it does not replay IBP identities or certify family
closure (edge-based re-derivation: `rustred walk-verify-closure`).

G2' residual-anchor records (`g2_residual_anchor_inspection`, walks run with
`--g2-residual-anchors union`) are checked independently of the engine: the
record order is the publication (merge) order, so each record's merge stamp
is recomputed from its stream position and must equal the recorded one; the
snapshot stamp is at most it; every anchor's recorded stamp is its stream
position and lies strictly below the snapshot (well-founded merge order, no
anchor cycle); the anchor is a same-owner Apply Native record, an initial
D-band partial (lending only its D < cut slice) or a G2' record with a
residual; no G2' record is an initial record; and Q is covered exactly by
its residual D band and the anchor scopes (an interval region-splitting
predicate written here, cross-checked by lattice-point enumeration on small
Q). The command must carry `--g2-residual-anchors union` iff such records exist.
"""
from __future__ import annotations

import argparse
from array import array
import codecs
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys
import time

_ROLES_SPEC = importlib.util.spec_from_file_location(
    "owner_query_roles", Path(__file__).with_name("owner_query_roles.py"))
ROLES = importlib.util.module_from_spec(_ROLES_SPEC)
_ROLES_SPEC.loader.exec_module(ROLES)

MAX_VALUE_BYTES = 64 * 1024 * 1024
SENTINEL = 2 ** 64 - 1
NATIVE, DELEGATED, PARTIAL, G2 = 1, 2, 3, 4
KIND_CODES = {"native_inspection": NATIVE, "delegated_not_inspected": DELEGATED,
              "partial_initial_overlap_inspection": PARTIAL, "g2_residual_anchor_inspection": G2}
NATIVE_AUTHORITY = "same_snapshot_phase_owner_native_summary"
ROUTE_EVENT_PARTS = ("apply_domains", "route_domains", "zero_sectors", "missing_routes")
LEDGER_ZERO = ("native_frontier_blocked", "native_failed", "native_cancelled", "pending_native_publications",
               "delegated_pending", "delegated_frontier_blocked", "delegated_failure_blocked",
               "delegated_cancelled", "partial_initial_blocked")
POOL_ZERO = ("active_workers", "occupied_native_slots", "dispatched_uncommitted_domains",
             "finished_uncommitted_domains", "worker_buffered_events",
             "worker_buffered_logical_bytes", "completed_escrow_entries")
TOP_ZERO = ("queued_nodes", "frontiers", "failed_nodes", "pending_descendant_domains")
AMENDMENT_SCHEMA = "rustred.owner-domain-walk-amendment.json.v1"
# An amended (rescued) walk may keep frontier-blocked obligations: the
# frontier-bearing nodes and their ancestors are quarantined, never required.
RESCUE_ABANDONED_KIND = "rescue_abandoned_dead_cone"
RESCUE_BLOCKED = ("native_frontier_blocked", "delegated_frontier_blocked", "partial_initial_blocked")
POWER_FIELDS = ("max_positive_power", "min_power_difference", "max_power_difference")


class Stream:
    """Incremental JSON reader: one value at a time, SHA-256 of all bytes."""

    def __init__(self, path):
        self.file = Path(path).open("rb")
        self.hash = hashlib.sha256()
        self.buffer = ""
        self.cursor = 0
        self.eof = False
        self.decoder = json.JSONDecoder()
        self.utf8 = codecs.getincrementaldecoder("utf-8")()

    def more(self):
        self.buffer = self.buffer[self.cursor:]
        self.cursor = 0
        block = self.file.read(1024 * 1024)
        self.hash.update(block)
        self.buffer += self.utf8.decode(block, final=not block)
        self.eof = not block
        if len(self.buffer) > MAX_VALUE_BYTES:
            raise ValueError("one JSON value exceeds the bounded audit buffer")

    def peek(self):
        while True:
            while self.cursor < len(self.buffer) and self.buffer[self.cursor].isspace():
                self.cursor += 1
            if self.cursor < len(self.buffer):
                return self.buffer[self.cursor]
            if self.eof:
                return ""
            self.more()

    def token(self, expected):
        if self.peek() != expected:
            raise ValueError(f"expected {expected!r} near {self.buffer[self.cursor:self.cursor + 80]!r}")
        self.cursor += 1

    def value(self):
        if not self.peek():
            raise ValueError("unexpected end of JSON input")
        while True:
            try:
                result, end = self.decoder.raw_decode(self.buffer, self.cursor)
                # A number at a chunk boundary may still continue.
                if end == len(self.buffer) and not self.eof:
                    self.more()
                    continue
                self.cursor = end
                return result
            except json.JSONDecodeError:
                if self.eof:
                    raise ValueError("malformed JSON value") from None
                self.more()

    def finish(self):
        if self.peek():
            raise ValueError("trailing JSON input")
        self.file.close()
        return self.hash.hexdigest()


def stream_walk(path):
    """Yield ("top", key, value) for scalar top-level entries and ("domain", record) per row."""
    stream = Stream(path)
    try:
        yield from _stream_walk(stream)
    finally:
        stream.file.close()


def _stream_walk(stream):
    seen = set()
    stream.token("{")
    while stream.peek() != "}":
        key = stream.value()
        if not isinstance(key, str) or key in seen:
            raise ValueError("invalid or duplicate top-level key")
        seen.add(key)
        stream.token(":")
        if key == "domains":
            stream.token("[")
            while stream.peek() != "]":
                yield ("domain", stream.value())
                if stream.peek() != "]":
                    stream.token(",")
                    if stream.peek() == "]":
                        raise ValueError("trailing comma in domains array")
            stream.token("]")
            yield ("top", key, "<streamed>")
        else:
            yield ("top", key, stream.value())
        if stream.peek() != "}":
            stream.token(",")
            if stream.peek() == "}":
                raise ValueError("trailing comma in top-level object")
    stream.token("}")
    yield ("sha256", stream.finish())


def read_json(path):
    return ROLES.loads_document(Path(path).read_text())


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def flag_value(command, name, default=None):
    count = command.count(name)
    if count == 0:
        return default
    if count != 1:
        raise ValueError(f"command must contain {name} at most once")
    return command[command.index(name) + 1]


class GrowingArray:
    """Position-addressed storage that tolerates out-of-order record IDs."""

    def __init__(self, typecode, fill):
        self.values = array(typecode)
        self.fill = fill

    def ensure(self, index):
        if index >= len(self.values):
            self.values.extend([self.fill] * (index + 1 - len(self.values)))

    def __setitem__(self, index, value):
        self.ensure(index)
        self.values[index] = value

    def __getitem__(self, index):
        return self.values[index] if index < len(self.values) else self.fill

    def __len__(self):
        return len(self.values)


class Audit:
    def __init__(self, limit=200):
        self.violations = []
        self.suppressed = 0
        self.limit = limit

    def check(self, condition, message):
        if not condition:
            if len(self.violations) < self.limit:
                self.violations.append(message)
            else:
                self.suppressed += 1
        return condition


def check_native_stats(audit, phase, stats, identity, abandoned=False):
    if not isinstance(stats, dict):
        audit.check(False, f"record {identity}: stats missing")
        return {}
    numeric = {key: value for key, value in stats.items() if type(value) is int}
    for key, value in numeric.items():
        audit.check(value >= 0, f"record {identity}: negative {key}")
    if phase == "Apply":
        for key in ("problems", "unsupported_support_successors", "conditional_unsupported_support_successors"):
            audit.check(numeric.get(key) == 0, f"record {identity}: Apply {key} must be 0")
        audit.check(numeric.get("same_support_successors", 0) + numeric.get("strict_subsupport_successors", 0)
                    == numeric.get("successors"), f"record {identity}: Apply successor sum mismatch")
    else:
        audit.check(numeric.get("missing_routes") == 0, f"record {identity}: Route missing_routes must be 0")
        audit.check(numeric.get("masks_pruned", 0) <= numeric.get("masks_examined", 0),
                    f"record {identity}: Route pruned more masks than examined")
        audit.check(abandoned or numeric.get("events") == sum(numeric.get(key, 0) for key in ROUTE_EVENT_PARTS),
                    f"record {identity}: Route event accounting mismatch")
    return numeric


def within(limit, value, sign=1):
    """Rust `limit.is_none_or(|a| value.is_some_and(|b| sign * b <= sign * a))`; None is unbounded."""
    return limit is None or (value is not None and sign * value <= sign * limit)


def optional(value, low, high):
    """None, or a JSON integer (not a bool) in [low, high)."""
    return value is None or (type(value) is int and low <= value < high)


def parsed_domain(domain, rank_field, arity):
    """(lower, upper, rank, powers) as the walker's query parser reads `domain`, or None if it rejects it.

    Mirrors matching/input.rs `parse`: u64 lower and u64-or-null upper bounds of
    owner arity with lower <= upper, an explicit u32-or-null rank, and power
    bounds with only the known keys (a missing key is None): a u64
    max_positive_power and i64 differences with min <= max
    (`DomainPowerBounds::validate`). Walker records always write every key.
    """
    lower, upper, powers = domain.get("lower"), domain.get("upper"), domain.get("power_bounds")
    if not (rank_field in domain and isinstance(powers, dict) and set(powers) <= set(POWER_FIELDS)
            and all(isinstance(axis, list) and len(axis) == arity for axis in (lower, upper))):
        return None
    rank, (positive, least, most) = domain[rank_field], (powers.get(field) for field in POWER_FIELDS)
    if not (all(value is not None and optional(value, 0, 2 ** 64) for value in lower)
            and all(optional(value, 0, 2 ** 64) for value in upper)
            and all(high is None or low <= high for low, high in zip(lower, upper))
            and optional(rank, 0, 2 ** 32) and optional(positive, 0, 2 ** 64)
            and optional(least, -2 ** 63, 2 ** 63) and optional(most, -2 ** 63, 2 ** 63)
            and (least is None or most is None or least <= most)):
        return None
    return lower, upper, rank, (positive, least, most)


def alias_contains(record, query):
    """Whether a later initial query may alias into an earlier admitted initial record.

    Mirrors the walker's syntactic `Domain::contains` (walking/queue.rs): same
    owner, `rank_contains`, `DomainPowerBounds::contains` and per-axis lower and
    upper bounds, None meaning unbounded, on a record carrying every walker
    field and a query the walker's parser accepts (`parsed_domain`). Initial
    queries pass through the full `Queue::admit`: exact key, the dominant full
    orthant (the lower=0, upper=None, unconstrained-power special case of this
    predicate) or general containment, so the target need not be a full
    orthant. The unlimited lane decides with the stronger `DomainPowerSummary`
    inclusion; only this sufficient implication is accepted here, so a
    semantic-only alias (or a query without a `power_bounds` object, which the
    parser reads as unconstrained) is still reported as a changed query.
    """
    owner, powers = record.get("owner"), record.get("power_bounds")
    if not (isinstance(owner, str) and owner == query.get("owner")
            and isinstance(powers, dict) and set(powers) == set(POWER_FIELDS)):
        return False
    outer, inner = parsed_domain(record, "rank", len(owner)), parsed_domain(query, "max_numerator_rank", len(owner))
    if outer is None or inner is None:
        return False
    (lower, upper, rank, powers), (inner_lower, inner_upper, inner_rank, inner_powers) = outer, inner
    return (within(rank, inner_rank)
            and within(powers[0], inner_powers[0])
            and within(powers[1], inner_powers[1], -1)
            and within(powers[2], inner_powers[2])
            and all(bound <= inner for bound, inner in zip(lower, inner_lower))
            and all(within(bound, inner) for bound, inner in zip(upper, inner_upper)))


# ---- Exact lattice predicates, independent of the walker's containment code ----
#
# A domain is the integer set {x in N^n : lower <= x <= upper, R <= rank,
# A <= max_positive_power, min_power_difference <= D <= max_power_difference}
# with physical indices n_i = x_i + 1 on owner axes and n_i = -x_i otherwise,
# A = sum(max(n_i, 0)) = sum_owner(x_i + 1), R = sum_other(x_i), D = A - R
# (rust `DomainPowerBounds`/`project`). A and R are sums over disjoint axis
# groups of integer intervals, so each takes every integer between its box
# extremes and (A, R) ranges over a full integer rectangle; D = A - R then
# takes every integer between the rectangle's extremes. Emptiness is thus an
# interval test, and inclusion is emptiness of the inner set intersected with
# the negation of each outer constraint. Nothing here reuses or mirrors
# `DomainPowerSummary`; `box_points` enumerates small sets as a brute-force
# cross-check of both.


def g2_anchor_locally_eligible(row):
    """Own native/residual success, not transitive closure or responsibility.

    A historical anchor may later reach a blocked descendant. Its exact loan
    remains valid only with that dependency retained; its own frontier or an
    abandoned inspection never lends a native scope.
    """
    if row.get("error") is not None or row.get("frontiers") != [] or row.get("rescue_abandoned") is True:
        return False
    kind = row.get("record_kind")
    if kind == "native_inspection":
        return row.get("local_inspection_finished") is True
    if kind == "partial_initial_overlap_inspection":
        return row.get("residual_inspection_finished") is True
    if kind == "g2_residual_anchor_inspection":
        block = row.get("g2_residual_anchors")
        return (row.get("residual_inspection_finished") is True and isinstance(block, dict)
                and block.get("residual_power_bounds") is not None)
    return False


def box_of(domain, rank_field="rank", phase=None):
    """(owner, phase, lower, upper, rank, A, D_min, D_max) of a walker record or query, or None."""
    owner = domain.get("owner") if isinstance(domain, dict) else None
    if not (isinstance(owner, str) and owner and set(owner) <= {"0", "1"}):
        return None
    parsed = parsed_domain(domain, rank_field, len(owner))
    if parsed is None:
        return None
    lower, upper, rank, (positive, least, most) = parsed
    return (owner, domain.get("phase", phase), tuple(lower), tuple(upper), rank, positive, least, most)


def _sum_interval(lower, upper, axes):
    low = sum(lower[axis] for axis in axes)
    high = None if any(upper[axis] is None for axis in axes) else sum(upper[axis] for axis in axes)
    return low, high


def _cap(value, bound):
    """min(value, bound) with None meaning +infinity."""
    return bound if value is None else value if bound is None else min(value, bound)


def _floor(value, bound):
    """max(value, bound) with None meaning -infinity."""
    return bound if value is None else value if bound is None else max(value, bound)


def lattice_nonempty(owner, lower, upper, a_low=None, a_high=None, r_low=None, r_high=None,
                     d_low=None, d_high=None):
    """Whether some integer point satisfies the box and the A/R/D intervals (None: unbounded)."""
    if any(high is not None and high < low for low, high in zip(lower, upper)):
        return False
    active = [axis for axis, bit in enumerate(owner) if bit == "1"]
    other = [axis for axis, bit in enumerate(owner) if bit != "1"]
    box_a_low, box_a_high = _sum_interval(lower, upper, active)
    box_a_low += len(active)
    box_a_high = None if box_a_high is None else box_a_high + len(active)
    box_r_low, box_r_high = _sum_interval(lower, upper, other)
    a_low, a_high = _floor(box_a_low, a_low), _cap(box_a_high, a_high)
    r_low, r_high = _floor(box_r_low, r_low), _cap(box_r_high, r_high)
    if (a_high is not None and a_high < a_low) or (r_high is not None and r_high < r_low):
        return False
    # D over the (A, R) rectangle: every integer in [a_low - r_high, a_high - r_low].
    low = _floor(None if r_high is None else a_low - r_high, d_low)
    high = _cap(None if a_high is None else a_high - r_low, d_high)
    return low is None or high is None or low <= high


def box_nonempty(box, **extra):
    owner, _, lower, upper, rank, positive, least, most = box
    lower, upper = list(lower), list(upper)
    for axis, value in extra.pop("raise_lower", {}).items():
        lower[axis] = max(lower[axis], value)
    for axis, value in extra.pop("cut_upper", {}).items():
        if value < lower[axis]:
            return False
        upper[axis] = _cap(upper[axis], value)
    return lattice_nonempty(owner, lower, upper, extra.get("a_low"), _cap(positive, extra.get("a_high")),
                            extra.get("r_low"), _cap(rank, extra.get("r_high")),
                            _floor(least, extra.get("d_low")), _cap(most, extra.get("d_high")))


def box_contains(outer, inner):
    """Exact integer-set inclusion inner <= outer (phase is the caller's check)."""
    if not box_nonempty(inner):
        return True
    owner, _, lower, upper, rank, positive, least, most = outer
    if owner != inner[0]:
        return False
    for axis, (low, high) in enumerate(zip(lower, upper)):
        if high is not None and box_nonempty(inner, raise_lower={axis: high + 1}):
            return False
        if low > 0 and box_nonempty(inner, cut_upper={axis: low - 1}):
            return False
    return not ((rank is not None and box_nonempty(inner, r_low=rank + 1))
                or (positive is not None and box_nonempty(inner, a_low=positive + 1))
                or (least is not None and box_nonempty(inner, d_high=least - 1))
                or (most is not None and box_nonempty(inner, d_low=most + 1)))


def box_member(box, point):
    owner, _, lower, upper, rank, positive, least, most = box
    if not all(low <= x and (high is None or x <= high) for x, low, high in zip(point, lower, upper)):
        return False
    a = sum(x + 1 for x, bit in zip(point, owner) if bit == "1")
    r = sum(x for x, bit in zip(point, owner) if bit != "1")
    return ((rank is None or r <= rank) and (positive is None or a <= positive)
            and (least is None or a - r >= least) and (most is None or a - r <= most))


def box_points(box, limit):
    """Every lattice point of `box` if its A/R-capped rectangle has at most `limit` points, else None."""
    owner, _, lower, upper, rank, positive, _, _ = box
    active = [axis for axis, bit in enumerate(owner) if bit == "1"]
    other = [axis for axis, bit in enumerate(owner) if bit != "1"]
    caps = list(upper)
    for group, total in ((active, None if positive is None else positive - len(active)), (other, rank)):
        if total is None:
            continue
        floor = sum(lower[axis] for axis in group)
        for axis in group:
            caps[axis] = _cap(caps[axis], total - floor + lower[axis])
    size = 1
    for low, high in zip(lower, caps):
        if high is None:
            return None
        size *= max(high - low + 1, 0)
        if size > limit:
            return None
    points = [()]
    for low, high in zip(lower, caps):
        points = [point + (x,) for point in points for x in range(low, high + 1)]
    return [point for point in points if box_member(box, point)]


class Containment:
    """Exact inclusion with a bounded brute-force lattice cross-check."""

    def __init__(self, max_points=256, budget=2_000_000):
        self.max_points, self.budget = max_points, budget
        self.exact = self.syntactic = self.brute_force_checks = self.brute_force_points = 0
        self.disagreements = 0

    def __call__(self, outer, inner):
        self.exact += 1
        result = box_contains(outer, inner)
        if self.max_points and self.brute_force_points < self.budget:
            points = box_points(inner, self.max_points)
            if points is not None:
                self.brute_force_checks += 1
                self.brute_force_points += len(points)
                brute = all(box_member(outer, point) for point in points) and (
                    not points or outer[0] == inner[0])
                if brute != result:
                    self.disagreements += 1
                    return False
        return result

    def json(self):
        return {"exact_checks": self.exact, "brute_force_checks": self.brute_force_checks,
                "brute_force_points": self.brute_force_points, "brute_force_max_points": self.max_points,
                "brute_force_point_budget": self.budget, "exact_vs_brute_force_disagreements": self.disagreements}


# ---- Exact multi-target cover for G2' records -------------------------------
# A region is a box with interval bounds on A, R and D; `region_minus` splits
# region minus one target into disjoint regions over the target's constraints
# (each region is decided exactly by `lattice_nonempty`), and `union_covered`
# recurses over the targets. Only the region budget can leave it undecided.


def region_of(box):
    owner, _, lower, upper, rank, positive, least, most = box
    return (owner, list(lower), list(upper), (None, positive), (None, rank), (least, most))


def region_nonempty(region):
    owner, lower, upper, a, r, d = region
    return lattice_nonempty(owner, lower, upper, a[0], a[1], r[0], r[1], d[0], d[1])


def region_with(region, bound, negated):
    owner, lower, upper, a, r, d = region
    lower, upper = list(lower), list(upper)
    kind, axis, value = bound
    if kind == "axis_low":
        if negated:
            if value == 0:
                return None
            upper[axis] = _cap(upper[axis], value - 1)
        else:
            lower[axis] = max(lower[axis], value)
    elif kind == "axis_high":
        if negated:
            lower[axis] = max(lower[axis], value + 1)
        else:
            upper[axis] = _cap(upper[axis], value)
    elif kind == "r_high":
        r = (_floor(r[0], value + 1), r[1]) if negated else (r[0], _cap(r[1], value))
    elif kind == "a_high":
        a = (_floor(a[0], value + 1), a[1]) if negated else (a[0], _cap(a[1], value))
    elif kind == "d_low":
        d = (d[0], _cap(d[1], value - 1)) if negated else (_floor(d[0], value), d[1])
    else:  # d_high
        d = (_floor(d[0], value + 1), d[1]) if negated else (d[0], _cap(d[1], value))
    region = (owner, lower, upper, a, r, d)
    return region if region_nonempty(region) else None


def region_minus(region, target):
    owner, _, lower, upper, rank, positive, least, most = target
    bounds = []
    for axis, (low, high) in enumerate(zip(lower, upper)):
        bounds.append(("axis_low", axis, low))
        if high is not None:
            bounds.append(("axis_high", axis, high))
    if rank is not None:
        bounds.append(("r_high", None, rank))
    if positive is not None:
        bounds.append(("a_high", None, positive))
    if least is not None:
        bounds.append(("d_low", None, least))
    if most is not None:
        bounds.append(("d_high", None, most))
    pieces, rest = [], region
    for bound in bounds:
        piece = region_with(rest, bound, True)
        if piece is not None:
            pieces.append(piece)
        rest = region_with(rest, bound, False)
        if rest is None:
            break
    return pieces


def union_covered(box, targets, budget=1 << 18):
    """Exact box <= union(targets) for same-phase boxes; None when the region budget runs out."""
    remaining = [budget]

    def covered(region, index):
        if index == len(targets):
            return False
        if remaining[0] <= 0:
            return None
        remaining[0] -= 1
        if targets[index][0] != region[0]:
            return covered(region, index + 1)
        for piece in region_minus(region, targets[index]):
            result = covered(piece, index + 1)
            if result is not True:
                return result
        return True

    region = region_of(box)
    if not region_nonempty(region):
        return True
    return covered(region, 0)


def d_band(box, low, high):
    owner, phase, lower, upper, rank, positive, least, most = box
    return (owner, phase, lower, upper, rank, positive, _floor(least, low), _cap(most, high))


def residual_bounds(box, cut):
    """The D < cut residual power bounds the walker inspects for a partial record."""
    _, _, _, _, _, positive, least, most = box
    return {"max_positive_power": positive, "min_power_difference": least,
            "max_power_difference": _cap(most, cut - 1)}


def high_slice(box, cut):
    owner, phase, lower, upper, rank, positive, least, most = box
    return (owner, phase, lower, upper, rank, positive, _floor(least, cut), most)


def resumed_attempts(resumed, uncommitted, kinds, check):
    """Uncommitted attempts a checkpoint carried into this session; returns their count.

    A walk that never resumed must have none. A resumed walk may report the
    in-flight inspections its paused session returned but never published:
    each is `committed: false` with `resume_reinspects_unfinished_part: true`,
    and its domain must be published natively (or as a partial inspection) in
    this result, since resume re-inspects it from the start.
    """
    if not resumed:
        check(uncommitted == [], "uncommitted_inspections must be empty")
        return 0
    if not check(isinstance(uncommitted, list), "uncommitted_inspections must be a list"):
        return 0
    for row in uncommitted:
        identity = row.get("id") if isinstance(row, dict) else None
        if not check(type(identity) is int and identity >= 0
                     and row.get("committed") is False and row.get("resume_reinspects_unfinished_part") is True,
                     f"resumed run: uncommitted entry {identity!r} is not a carried earlier-session attempt"):
            continue
        check(identity < len(kinds) and kinds[identity] in (NATIVE, PARTIAL, G2),
              f"resumed run: carried attempt {identity} was not re-inspected and published natively")
    return len(uncommitted)


def locate(run, queries=None, command=None, receipt=None):
    """Find the native argv, query document and resource receipt for one run directory."""
    run = Path(run)
    if command is not None:
        argv = read_json(command)
    elif (run / "request.json").is_file():
        argv = read_json(run / "request.json")["command"]
    elif (run / "command.json").is_file():
        argv = read_json(run / "command.json")
    else:
        raise ValueError("no request.json or command.json describes the native command")
    if not isinstance(argv, list) or not all(isinstance(item, str) for item in argv):
        raise ValueError("native command must be a list of strings")
    queries_path = Path(queries) if queries is not None else flag_value(argv, "--queries")
    if queries_path is None:
        raise ValueError("no query document: pass --queries or a command with --queries")
    if receipt is not None:
        receipt_path = Path(receipt)
    elif (run / "supervisor-result.json").is_file():
        receipt_path = run / "supervisor-result.json"
    elif (run / "guard" / "result.json").is_file():
        receipt_path = run / "guard" / "result.json"
    else:
        receipt_path = None
    workers = flag_value(argv, "--workers")
    inspectors = flag_value(argv, "--inspection-workers")
    lookahead = flag_value(argv, "--transfer-unreserved-lookahead")
    checkpoint = flag_value(argv, "--checkpoint") or flag_value(argv, "--resume")
    amendments = [Path(argv[index + 1]) for index, item in enumerate(argv[:-1]) if item == "--amend-queries"]
    return {"command": argv, "queries": Path(queries_path), "receipt": receipt_path,
            "amendments": amendments,
            "policy": flag_value(argv, "--publication-policy", "ordered"),
            "workers": None if workers is None else int(workers),
            "inspection_workers": None if inspectors is None else int(inspectors),
            "lookahead": None if lookahead is None else int(lookahead),
            "checkpoint": None if checkpoint is None else Path(checkpoint),
            "resumed": "--resume" in argv}


def pair_verifier(audit, result_path, report, verify_report, require_closure):
    """Bind a `rustred walk-verify-closure` report to the result.json this audit read.

    The verifier binds result.json record by record (content digests) to the
    checkpoint generation it read and records the file's canonical path, byte
    length, mtime and blake3. Here the audited file must be that same file
    (path, length, mtime), its checkpoint generation the verifier's, and the
    verifier's verdict PASS (with closure required, and every root independently
    verified, when this audit requires closure).
    """
    check = audit.check
    pairing = {"verify_report": str(verify_report), "paired": False}
    held = []

    def need(condition, message):
        # Every pairing check is an audit violation when it fails and a term of
        # the conjunction recorded as `paired`.
        held.append(bool(check(condition, message)))

    try:
        verifier = read_json(verify_report)
    except (OSError, ValueError) as error:
        check(False, f"verifier pairing: unreadable report: {error}")
        return pairing
    binding = verifier.get("result_binding")
    if not check(isinstance(binding, dict), "verifier pairing: the verifier bound no result.json (--no-result?)"):
        return pairing
    stat = Path(result_path).stat()
    generation = (report.get("checkpoint") or {}).get("generation")
    pairing.update(verifier_verdict=verifier.get("verdict"), verifier_generation=verifier.get("checkpoint", {}).get("generation"),
                   result_generation=generation, file_blake3=binding.get("file_blake3"),
                   records_compared=binding.get("records_compared"))
    need(Path(binding.get("canonical_path", "")) == Path(result_path).resolve(),
         "verifier pairing: the verifier bound a different result.json")
    need(binding.get("file_bytes") == stat.st_size and binding.get("file_mtime_unix_ns") == stat.st_mtime_ns,
         "verifier pairing: result.json changed since the verifier bound it (size or mtime)")
    need(binding.get("generation_matches") is True and binding.get("generation") == generation
         and verifier.get("checkpoint", {}).get("generation") == generation,
         "verifier pairing: checkpoint generation differs between the verifier and this result")
    need(binding.get("records_mismatched") == 0 and binding.get("records_not_published") == 0
         and binding.get("duplicate_rows") == 0, "verifier pairing: result rows differ from the verified generation")
    need(verifier.get("verdict") == "PASS", f"verifier pairing: verifier verdict {verifier.get('verdict')!r} is not PASS")
    need(verifier.get("mutation") is None,
         "verifier pairing: the verifier report is a --mutate negative control, not a certificate")
    if require_closure:
        need(verifier.get("closure_required") is True, "verifier pairing: the verifier did not require closure")
        # The gate (assert_oracle_pass.py): every root independently verified.
        total = verifier.get("roots_total")
        need(isinstance(total, int) and total >= 1 and verifier.get("roots_independently_verified") == total,
             "verifier pairing: not every root is independently verified "
             f"({verifier.get('roots_independently_verified')!r} of {total!r})")
    pairing["paired"] = all(held)
    return pairing


class CheckpointOnlySummary(Exception):
    """A summary is not a failed checkpoint, and is never a full-result proof."""


def audit_walk(run, queries=None, command=None, receipt=None, expect_schema=None, require_closure=False,
               containment=None, verify_report=None):
    run = Path(run)
    audit = Audit()
    report = {"audit": "FAIL", "run_directory": str(run), "family_closure_claim": False,
              "all_local_obligations_discharged": False, "full_family_closure_claim": False,
              "closure_required": require_closure,
              "scope": "recorded native completion and explicit dependencies; not IBP replay or termination"}
    containment = containment if containment is not None else Containment()
    try:
        located = locate(run, queries, command, receipt)
        report.update(native_command=located["command"], queries_path=str(located["queries"]),
                      publication_policy=located["policy"], resumed=located["resumed"],
                      receipt_path=None if located["receipt"] is None else str(located["receipt"]))
        report.update(_audit(run, located, audit, expect_schema, require_closure, containment))
        if verify_report is not None:
            report["verifier_pairing"] = pair_verifier(audit, run / "result.json", report, verify_report,
                                                       require_closure)
    except CheckpointOnlySummary as error:
        report["incomplete_reason"] = str(error)
    except (OSError, ValueError, KeyError, TypeError, IndexError, OverflowError) as error:
        audit.check(False, f"structural: {type(error).__name__}: {error}")
    report["containment_oracle"] = containment.json()
    report["violations"] = audit.violations
    report["violations_suppressed"] = audit.suppressed
    report["audit"] = ("FAIL" if audit.violations else
                       "INCOMPLETE" if "incomplete_reason" in report else "PASS")
    report["all_local_obligations_discharged"] = report["audit"] == "PASS"
    certification = report.get("certification")
    if isinstance(certification, dict):
        # Engine-reported closure re-checked for consistency only: this audit
        # sees no edges. Independent (edge-based, re-inspected) verification
        # is `rustred walk-verify-closure`; it counts here only when that
        # report is paired to this very result (--verify-report) and PASSes.
        certification["engine_closure_consistent"] = report["audit"] == "PASS" and require_closure
        pairing = report.get("verifier_pairing") or {}
        certification["independently_verified"] = (certification["engine_closure_consistent"]
                                                   and pairing.get("paired") is True)
    return report


def rescue_certification(run, top, queries, amended, documents, raw_inputs, query_count, inputs, amended_inputs,
                         total, containment, query_roles, require_closure, check, request_digest=None):
    """Rescued walk: amendment chain structure, amended inputs, per-physics-query certification.

    A query is certified iff some input record (of an original or an amended
    query, in input order) is descendant-closed, has the phase of the query's
    own record and contains the query (the audit's own lattice predicate).
    With closure required, every explicitly required query must be
    certified; auxiliary records may stay open (a frontier-
    bearing helper is quarantined, never required).
    """
    chain = top.get("amendments")
    chain = chain if isinstance(chain, list) else []
    check(len(chain) == len(documents), "rescued walk: result amendments != command --amend-queries")
    for index, (link, document) in enumerate(zip(chain, documents)):
        link = link if isinstance(link, dict) else {}
        check(link.get("sequence") == document.get("sequence") == index + 1,
              f"rescued walk: amendment {index + 1} sequence")
        parent = request_digest if index == 0 else (chain[index - 1] or {}).get("digest")
        check(document.get("parent") == link.get("parent") and (parent is None or link.get("parent") == parent),
              f"rescued walk: amendment {index + 1} does not chain from its parent")
        check(link.get("queries") == len(document.get("queries") or []),
              f"rescued walk: amendment {index + 1} query count")
        check(link.get("first_input") == query_count + sum(len(d.get("queries") or []) for d in documents[:index]),
              f"rescued walk: amendment {index + 1} first_input")
    check(len(amended_inputs) == len(amended), "rescued walk: amended inputs != amended queries")
    for position, ((sequence, row), (query_id, record)) in enumerate(zip(amended, amended_inputs)):
        entry = raw_inputs[query_count + position] if query_count + position < len(raw_inputs) else {}
        check(query_id == row.get("id") and entry.get("amendment") == sequence,
              f"rescued walk: amended input {position} is not amended query {row.get('id')!r}")
        check(type(record) is int and 0 <= record < total, f"amended query {row.get('id')!r}: no record")
    everything = [*((query, record) for query, (_, record) in zip(queries, inputs)),
                  *((row, record) for (_, row), (_, record) in zip(amended, amended_inputs))]
    needed = {record for _, record in everything if type(record) is int}
    rows = {}
    for item in stream_walk(run / "result.json"):
        if item[0] == "domain" and item[1].get("id") in needed:
            rows[item[1]["id"]] = item[1]
    candidates = [(record, rows.get(record)) for _, record in everything]
    for (query, record), (_, row) in zip(everything[len(queries):], candidates[len(queries):]):
        if row is not None:
            check(row.get("phase") in ("Apply", "Route") and containment(box_of(row), box_of(
                query, "max_numerator_rank", row.get("phase"))),
                  f"amended query {query.get('id')!r} is not contained in its record {record}")
    physics = {"total": 0, "certified": 0, "certified_through_amended_records": 0, "uncertified": []}
    helper_records = set()
    for position, (query, record) in enumerate(everything):
        own = rows.get(record)
        if query_roles[query["id"]] == "auxiliary":
            if type(record) is int:
                helper_records.add(record)
            continue
        physics["total"] += 1
        phase = own.get("phase") if own is not None else "Apply"
        inner = box_of(query, "max_numerator_rank", phase)
        via = next((index for index, (candidate, row) in enumerate(candidates)
                    if row is not None and row.get("descendant_closed") is True and row.get("phase") == phase
                    and inner is not None and containment(box_of(row), inner)), None)
        if via is None:
            if len(physics["uncertified"]) < 1000:
                physics["uncertified"].append(query.get("id"))
            if require_closure:
                check(False, f"closure required: physics query {query.get('id')!r} has no closed containing record")
            continue
        physics["certified"] += 1
        physics["certified_through_amended_records"] += via >= len(queries)
    open_helpers = sorted(record for record in helper_records
                          if (rows.get(record) or {}).get("descendant_closed") is not True)
    engine = top.get("query_certification")
    engine = engine if isinstance(engine, dict) else {}
    check(engine.get("queries_total") == len(everything), "rescued walk: engine query_certification total")
    return {"scope": "per physics query through the first descendant-closed input record that contains it "
                     "(this audit's lattice predicate); helper records reported, not required",
            "amendments": len(documents), "amended_queries": len(amended),
            "digest_check": "blake3 chain verified by the native resume and walk-verify-closure",
            "physics_queries": physics,
            "helper_records": {"total": len(helper_records), "not_closed": len(open_helpers),
                               "not_closed_ids": open_helpers[:1000]},
            "engine_queries_certified": engine.get("queries_certified")}


def _audit(run, located, audit, expect_schema, require_closure=False, containment=None):
    check = audit.check
    containment = containment if containment is not None else Containment()
    policy = located["policy"]
    check(policy in ("ordered", "ready", "epoch"), f"unsupported publication policy {policy!r}")
    # Walk semantics 3 (epoch, W2 stage S2): records in merge order, every
    # native record carries accepted_events, the pool reports merged and
    # discarded inspections. Legacy epoch-export.json has full result records;
    # resumable CP6 summaries are explicitly INCOMPLETE below, never CP5 or
    # empty full-record proofs.
    streamed_order = policy in ("ready", "epoch")
    queries_document = read_json(located["queries"])
    queries = queries_document["queries"]
    query_roles = ROLES.query_roles(queries_document, require_explicit=bool(located.get("amendments")))
    check(bool(queries) and len({query["id"] for query in queries}) == len(queries),
          "queries must be nonempty with unique ids")
    arity = len(queries[0]["owner"])
    check(all(len(query["owner"]) == arity for query in queries), "queries must share one owner arity")
    check(arity <= 62, "owner arity above 62 is not supported by the bounded audit")
    # Frontier rescue (resume-time amendments, the W1 rescue note): the
    # command's --amend-queries files in chain order. Their byte digests are
    # blake3-chained and checked by the native resume and walk-verify-closure;
    # here the chain structure the result reports, the amended rows, their
    # input records (containment) and the per-physics-query certification.
    amended = []
    amendment_documents = []
    for path in located.get("amendments", []):
        document = read_json(path)
        check(document.get("schema") == AMENDMENT_SCHEMA, f"amendment {path}: schema != {AMENDMENT_SCHEMA}")
        amendment_documents.append(document)
        rows = document.get("queries")
        check(isinstance(rows, list), f"amendment {path}: queries must be a list")
        for identity in document.get("supersede", []):
            check(query_roles.get(identity) == "auxiliary",
                  f"amendment {path}: supersede may name only an earlier auxiliary query: {identity!r}")
        for row in rows if isinstance(rows, list) else []:
            check(row.get("id") not in query_roles, f"amendment {path}: query ID already declared")
            # Required IDs cannot be relabelled even in malformed evidence.
            query_roles.setdefault(row.get("id"), "auxiliary")
            amended.append((document.get("sequence"), row))
    rescue = bool(amendment_documents)
    check(len({query["id"] for query in queries} | {row.get("id") for _, row in amended})
          == len(queries) + len(amended), "amended query ids must be unique across the chain and the queries")
    # Aliased queries share a record, so the distinct initial records are a
    # prefix of at most one record per query; retain that bounded prefix.
    query_count = len(queries)
    owner_phase = GrowingArray("Q", SENTINEL)
    kinds = GrowingArray("B", 0)
    direct = GrowingArray("Q", SENTINEL)
    final = GrowingArray("Q", SENTINEL)
    initial = {}
    kind_counts, phases, native_phases, ranks = Counter(), Counter(), Counter(), Counter()
    apply_stats, route_stats = Counter(), Counter()
    accepted = 0
    out_of_order = 0
    prior = -1
    top = {}
    count = 0
    result_sha = None
    # Alias containment: aliases wait for their (later) direct representative;
    # one whose representative streamed first (Ready order) is re-checked in a
    # second pass. Partial records wait for their initial anchor row.
    waiting, late, partials = {}, [], []
    closed_claims, parity = Counter(), Counter()
    alias_checks = 0
    # G2': stream positions (merge stamps), G2' records, partial cuts.
    position = GrowingArray("Q", SENTINEL)
    g2_records, g2_residual_ids, partial_cut = [], set(), {}
    g2_counts = Counter()

    def check_alias(identity, alias_box, representative, representative_box):
        nonlocal alias_checks
        alias_checks += 1
        if not check(representative_box is not None, f"record {identity}: alias representative has no domain"):
            return
        check(alias_box[1] == representative_box[1] and containment(representative_box, alias_box),
              f"record {identity}: alias domain not contained in representative {representative}")

    for item in stream_walk(run / "result.json"):
        if item[0] == "sha256":
            result_sha = item[1]
            continue
        if item[0] == "top":
            top[item[1]] = item[2]
            if item[1] == "full_result_in_output_document" and item[2] is False:
                raise CheckpointOnlySummary(
                    "checkpoint-only output has no full record proof; run native walk-verify-closure "
                    "on the checkpoint with --no-result (omitting --result still auto-selects a nearby "
                    "result.json); this audit has not validated the checkpoint")
            continue
        row = item[1]
        identity = row.get("id")
        if not check(type(identity) is int and identity >= 0, f"record #{count}: invalid id"):
            count += 1
            continue
        count += 1
        kinds.ensure(identity)
        if not check(kinds[identity] == 0, f"record {identity}: duplicate id"):
            continue
        box = box_of(row)
        check(box is not None, f"record {identity}: malformed domain")
        for alias, alias_box in waiting.pop(identity, ()):
            check_alias(alias, alias_box, identity, box)
        claim = row.get("descendant_closed")
        closed_claims[{True: "true", False: "false", None: "null"}.get(claim, "invalid")] += 1
        check(claim in (True, False, None), f"record {identity}: invalid descendant_closed")
        kind, owner, phase = row.get("record_kind"), row.get("owner"), row.get("phase")
        check(phase in ("Apply", "Route"), f"record {identity}: invalid phase {phase!r}")
        valid_owner = isinstance(owner, str) and len(owner) == arity and set(owner) <= {"0", "1"}
        check(valid_owner, f"record {identity}: invalid owner {owner!r}")
        rank = row.get("rank")
        check(rank is None or (type(rank) is int and rank >= 0), f"record {identity}: invalid rank")
        code = KIND_CODES.get(kind)
        if not check(code is not None, f"record {identity}: unknown record_kind {kind!r}"):
            continue
        out_of_order += identity < prior
        prior = identity
        kind_counts[kind] += 1
        phases[phase] += 1
        ranks[rank] += 1
        kinds[identity] = code
        position[identity] = count - 1
        owner_phase[identity] = ((int(owner, 2) << 1) | int(phase == "Route")) if valid_owner else SENTINEL
        if identity < query_count:
            initial[identity] = row
        if code == DELEGATED:
            check(row.get("local_inspection_finished") is False, f"record {identity}: alias claims local inspection")
            status = row.get("responsibility_status")
            blocked = (rescue and isinstance(status, dict) and set(status) == {"blocked_by_representative_frontiers"})
            check(status == "discharged_by_representative" or blocked,
                  f"record {identity}: alias responsibility_status")
            check(not (blocked and claim is True), f"record {identity}: frontier-blocked alias reported closed")
            check(row.get("containment_authority") == NATIVE_AUTHORITY, f"record {identity}: alias containment authority")
            representative, resolved = row.get("representative_id"), row.get("final_representative_id")
            if check(type(representative) is int and representative > identity and type(resolved) is int and resolved >= 0,
                     f"record {identity}: alias representative must be a later id"):
                direct[identity] = representative
                final[identity] = resolved
                if box is not None:
                    if kinds[representative]:
                        late.append((identity, box, representative))
                    else:
                        waiting.setdefault(representative, []).append((identity, box))
            continue
        frontiers, error = row.get("frontiers"), row.get("error")
        parity["record_frontiers"] += len(frontiers) if isinstance(frontiers, list) else 0
        parity["error_records"] += error is not None
        refusals = row.get("optional_refusals")
        parity["refusal_provenance_entries"] += len(refusals) if isinstance(refusals, list) else 0
        # Seal rule (F8): only a native with 0 frontiers and no error seals, so
        # only such a record (or an alias) may be reported descendant-closed.
        check(not (claim is True and (error is not None or frontiers != [])),
              f"record {identity}: descendant_closed despite a frontier or error")
        check(error is None, f"record {identity}: native error recorded")
        if rescue:
            # Frontiers stay explicit, never closed; a frontier-free native
            # is still discharged (its seal does not depend on the taint).
            check(isinstance(frontiers, list), f"record {identity}: frontiers must be a list")
            parity["rescue_frontier_records"] += frontiers != []
            if code == NATIVE:
                check(row.get("local_classification_discharged") is (frontiers == []),
                      f"record {identity}: classification discharge != frontier-free")
        else:
            check(frontiers == [], f"record {identity}: nonzero frontiers")
            check(row.get("local_classification_discharged") is True, f"record {identity}: classification not discharged")
        abandoned = row.get("rescue_abandoned") is True
        if abandoned:
            # Rescue: published without inspection, one bookkeeping frontier.
            parity["rescue_abandoned_records"] += 1
            check(rescue and code == NATIVE and isinstance(frontiers, list) and len(frontiers) == 1
                  and isinstance(frontiers[0], dict) and frontiers[0].get("kind") == RESCUE_ABANDONED_KIND
                  and claim is not True and (row.get("stats") or {}).get("events") == 1,
                  f"record {identity}: malformed rescue-abandoned record")
        if code == NATIVE:
            check(row.get("local_inspection_finished") is (not abandoned),
                  f"record {identity}: native inspection unfinished")
        elif code == G2:
            check(phase == "Apply", f"record {identity}: G2' inspection outside Apply")
            check(row.get("local_inspection_finished") is False, f"record {identity}: G2' record claims full inspection")
            check(row.get("residual_inspection_finished") is True, f"record {identity}: G2' residual unfinished")
            status = row.get("responsibility_status")
            blocked_key = "blocked_by_residual_or_g2_anchor_frontiers"
            blocked = (rescue and isinstance(status, dict) and set(status) == {blocked_key}
                       and type(status[blocked_key]) is int and status[blocked_key] > 0)
            check(status == "discharged_by_residual_and_g2_anchors" or blocked,
                  f"record {identity}: G2' responsibility_status")
            check(not (blocked and claim is True), f"record {identity}: frontier-blocked G2' record reported closed")
            g2_counts["blocked_responsibility_records"] += int(blocked)
            block = row.get("g2_residual_anchors")
            if check(isinstance(block, dict) and block.get("mode") == "union"
                     and block.get("coordinates_and_rank_unchanged") is True
                     and isinstance(block.get("anchors"), list) and box is not None,
                     f"record {identity}: malformed G2' block"):
                residual = block.get("residual_power_bounds")
                if residual is not None:
                    g2_residual_ids.add(identity)
                g2_records.append((identity, box, block))
        else:
            check(phase == "Apply", f"record {identity}: partial inspection outside Apply")
            check(row.get("local_inspection_finished") is False, f"record {identity}: partial claims full inspection")
            check(row.get("residual_inspection_finished") is True, f"record {identity}: residual inspection unfinished")
            status = row.get("responsibility_status")
            blocked = (rescue and isinstance(status, dict)
                       and set(status) == {"blocked_by_residual_or_initial_anchor_frontiers"})
            check(status == "discharged_by_residual_and_initial_anchor" or blocked,
                  f"record {identity}: partial responsibility_status")
            check(not (blocked and claim is True), f"record {identity}: frontier-blocked partial reported closed")
            link = row.get("initial_overlap")
            link = link if isinstance(link, dict) else {}
            check(link.get("coordinates_and_rank_unchanged") is True, f"record {identity}: partial overlap changed coordinates")
            check(link.get("authority") == NATIVE_AUTHORITY, f"record {identity}: partial overlap authority")
            anchor = link.get("anchor_id")
            if check(type(anchor) is int and anchor >= 0, f"record {identity}: partial anchor id missing"):
                final[identity] = anchor
                cut = link.get("cut")
                if check(type(cut) is int and box is not None, f"record {identity}: partial cut missing"):
                    partials.append((identity, box, anchor, cut, link.get("residual_power_bounds")))
                    partial_cut[identity] = cut
        if phase == "Route":
            check(row.get("conservative_route_overcover") is True, f"record {identity}: Route without conservative overcover")
        numeric = check_native_stats(audit, phase, row.get("stats"), identity, abandoned)
        target = apply_stats if phase == "Apply" else route_stats
        for field, value in numeric.items():
            target[field] += value
        native_phases[phase] += 1
        events = row.get("accepted_events")
        if streamed_order:
            check(type(events) is int and events >= 0, f"record {identity}: {policy} record lacks accepted_events")
        if events is not None:
            # F7: a committed record accepted exactly the events its native stream emitted.
            if check(type(events) is int and events >= 0, f"record {identity}: invalid accepted_events"):
                accepted += events
                parity["records_with_accepted_events"] += 1
                check(events == numeric.get("events"), f"record {identity}: accepted_events != stats.events")
    for representative, entries in waiting.items():
        for alias, _ in entries:
            check(False, f"record {alias}: alias representative {representative} has no record")
    if late:
        needed = {representative for _, _, representative in late}
        boxes = {}
        for item in stream_walk(run / "result.json"):
            if item[0] == "domain" and item[1].get("id") in needed:
                boxes[item[1]["id"]] = box_of(item[1])
        for alias, alias_box, representative in late:
            check_alias(alias, alias_box, representative, boxes.get(representative))
    for identity, box, anchor, cut, residual in partials:
        anchor_row = initial.get(anchor)
        anchor_box = box_of(anchor_row) if anchor_row is not None else None
        if not check(anchor_box is not None, f"record {identity}: partial anchor {anchor} has no initial row"):
            continue
        # The D >= cut slice is discharged by the anchor, the D < cut residual natively.
        check(anchor_box[1] == box[1] and containment(anchor_box, high_slice(box, cut)),
              f"record {identity}: partial D>=cut slice not contained in anchor {anchor}")
        check(residual == residual_bounds(box, cut), f"record {identity}: partial residual bounds != D<cut slice")
    check(top.get("domains") == "<streamed>", "result has no domains array")
    check(count > 0, "no domain records")
    check(count == len(kinds) and all(kinds[index] != 0 for index in range(len(kinds))),
          "record ids are not a contiguous 0..n-1 set")
    total = len(kinds)
    # The walker admits queries and records "inputs" in query document order
    # (checked below); a later query may alias into an admitted record, so the
    # first query naming a record is the one that admitted it.
    inputs = top.get("inputs")
    raw_inputs = inputs if isinstance(inputs, list) else []
    inputs = [(entry["id"], entry["domain"]) for entry in raw_inputs]
    # Amended inputs follow the original ones (chain order); only the original
    # queries admit the initial prefix.
    amended_inputs = inputs[query_count:] if rescue else []
    inputs = inputs[:query_count] if rescue else inputs
    admitting = {}
    for query_id, record in inputs:
        admitting.setdefault(record, query_id)
    initial_count = len(admitting)
    # Dependency resolution: aliases point forward to completed native records.
    resolved = GrowingArray("Q", SENTINEL)
    resolved.ensure(max(total - 1, 0))
    for identity in range(total - 1, -1, -1):
        code = kinds[identity]
        if code == DELEGATED:
            representative = direct[identity]
            if not check(representative != SENTINEL and representative < total and kinds[representative] != 0,
                         f"record {identity}: alias representative out of range"):
                resolved[identity] = identity
                continue
            check(owner_phase[representative] == owner_phase[identity], f"record {identity}: cross-phase/owner alias")
            target = resolved[representative]
            check(final[identity] == target, f"record {identity}: final representative {final[identity]} != resolved {target}")
            check(kinds[target] in (NATIVE, PARTIAL, G2), f"record {identity}: final representative is not native")
            check(owner_phase[target] == owner_phase[identity], f"record {identity}: final representative owner/phase differs")
            resolved[identity] = target
        else:
            resolved[identity] = identity
            if code == PARTIAL:
                anchor = final[identity]
                if check(anchor != SENTINEL and anchor < initial_count and anchor < identity,
                         f"record {identity}: partial anchor must be an earlier initial record"):
                    check(kinds[anchor] == NATIVE and owner_phase[anchor] == owner_phase[identity]
                          and not owner_phase[identity] & 1,
                          f"record {identity}: partial anchor is not a same-owner Apply native inspection")
    native_count = sum(native_phases.values())
    check(native_count == total - kind_counts["delegated_not_inspected"], "native/alias partition mismatch")
    # G2' residual-anchor records (module note).
    g2_flag = flag_value(located["command"], "--g2-residual-anchors", "off")
    check(not g2_records or g2_flag == "union",
          "G2' records without --g2-residual-anchors union in the command")
    if g2_records:
        needed = {identity for identity, _, _ in g2_records}
        for _, _, block in g2_records:
            for anchor in block["anchors"]:
                if isinstance(anchor, dict) and type(anchor.get("id")) is int:
                    needed.add(anchor["id"])
        boxes, local_eligibility = {}, {}
        for item in stream_walk(run / "result.json"):
            if item[0] == "domain" and item[1].get("id") in needed:
                boxes[item[1]["id"]] = box_of(item[1])
                local_eligibility[item[1]["id"]] = g2_anchor_locally_eligible(item[1])
        for identity, box, block in g2_records:
            g2_counts["records"] += 1
            own = position[identity]
            stamp, snapshot = block.get("merge_stamp"), block.get("snapshot_stamp")
            check(identity >= initial_count, f"record {identity}: G2' record on an initial record")
            check(stamp == own, f"record {identity}: G2' merge_stamp {stamp!r} != stream position {own}")
            if not check(type(snapshot) is int and 0 <= snapshot <= own,
                         f"record {identity}: G2' snapshot {snapshot!r} not <= its stream position {own}"):
                continue
            targets, admissible = [], True
            residual = block.get("residual_power_bounds")
            if residual is not None:
                g2_counts["residual"] += 1
                band = (residual.get("min_power_difference"), residual.get("max_power_difference"))
                if check(type(band[0]) is int and type(band[1]) is int and band[0] <= band[1]
                         and residual.get("max_positive_power") == box[5],
                         f"record {identity}: G2' residual is not a D band of the domain"):
                    targets.append(d_band(box, band[0], band[1]))
                else:
                    admissible = False
            else:
                g2_counts["full_cover"] += 1
            check(block.get("residual_pieces") == (0 if residual is None else 1),
                  f"record {identity}: G2' residual_pieces")
            check(len(block["anchors"]) > 0, f"record {identity}: G2' record without anchors")
            previous = -1
            for anchor in block["anchors"]:
                g2_counts["anchor_links"] += 1
                a = anchor.get("id") if isinstance(anchor, dict) else None
                if not check(type(a) is int and 0 <= a < total and kinds[a] != 0,
                             f"record {identity}: G2' anchor {a!r} has no record"):
                    admissible = False
                    continue
                a_stamp, a_kind = anchor.get("stamp"), anchor.get("kind")
                check(a_stamp == position[a], f"record {identity}: G2' anchor {a} stamp {a_stamp!r} != stream position {position[a]}")
                if not check(position[a] < snapshot, f"record {identity}: G2' anchor {a} not merged before the snapshot"):
                    admissible = False
                check(position[a] > previous, f"record {identity}: G2' anchors not in stamp order")
                previous = position[a]
                expected = {"native": NATIVE, "initial_d_band": PARTIAL, "g2_residual": G2}.get(a_kind)
                ok = expected is not None and kinds[a] == expected and (expected != G2 or a in g2_residual_ids)
                if not check(ok and owner_phase[a] == owner_phase[identity] and not owner_phase[a] & 1,
                             f"record {identity}: G2' anchor {a} ({a_kind!r}) is not a same-owner Apply Native, "
                             f"initial-D-band or G2' residual record"):
                    admissible = False
                    continue
                anchor_box = boxes.get(a)
                if not check(local_eligibility.get(a) is True,
                             f"record {identity}: G2' anchor {a} has no completed frontier-free local inspection"):
                    admissible = False
                    continue
                if not check(anchor_box is not None, f"record {identity}: G2' anchor {a} has no domain"):
                    admissible = False
                    continue
                g2_counts[f"anchor_{a_kind}"] += 1
                targets.append(d_band(anchor_box, None, partial_cut[a] - 1) if expected == PARTIAL else anchor_box)
            if not admissible:
                continue
            covered = union_covered(box, targets)
            g2_counts["union_checks"] += 1
            points = box_points(box, containment.max_points) if containment.max_points else None
            if points is not None:
                g2_counts["union_brute_force_checks"] += 1
                brute = all(any(box_member(target, point) for target in targets) for point in points)
                if covered is not None and brute != covered:
                    g2_counts["union_disagreements"] += 1
                    covered = False
            check(covered is True, f"record {identity}: G2' domain not covered by its residual and anchors ({covered})")
    if rescue:
        # A rescued walk drains with its quarantined frontiers explicit.
        frontier_free = top.get("frontiers") == 0
        check(top.get("status") == ("locally_resolved" if frontier_free else "incomplete")
              and top.get("error") is None, "rescued walk status is not locally_resolved/incomplete by frontiers")
        check(top.get("all_scheduled_domains_resolved") is frontier_free,
              "rescued walk all_scheduled_domains_resolved != frontier-free")
    else:
        check(top.get("status") == "locally_resolved" and top.get("error") is None, "walk status is not locally_resolved")
        check(top.get("all_scheduled_domains_resolved") is True, "not all scheduled domains resolved")
    check(top.get("recursive_worklist_exhausted") is True, "recursive worklist not exhausted")
    for field in TOP_ZERO:
        if not (rescue and field == "frontiers"):
            check(top.get(field) == 0, f"{field} must be 0")
    check(top.get("input_frontiers") == [], "input_frontiers must be empty")
    carried = resumed_attempts(located["resumed"], top.get("uncommitted_inspections"), kinds, check)
    for field in ("scheduled_nodes", "processed_nodes", "committed_domains"):
        check(top.get(field) == total, f"{field} != logical record count")
    for field in ("completed_nodes", "native_processed_nodes"):
        check(top.get(field) == native_count, f"{field} != native record count")
    schema = top.get("schema")
    check(isinstance(schema, str) and schema.startswith("rustred.owner-domain-walk.json.v"), "unknown result schema")
    if expect_schema is not None:
        check(schema == expect_schema, f"schema {schema!r} != expected {expect_schema!r}")
    check(top.get("events") == top.get("committed_events") == apply_stats["events"] + route_stats["events"],
          "event totals do not match native statistics")
    check(top.get("routed_domains") == native_phases["Route"], "routed_domains != native Route inspections")
    check(top.get("route_masks") == route_stats["masks_examined"], "route_masks != examined masks")
    if streamed_order:
        check(accepted == top.get("events"), f"{policy} accepted_events do not sum to events")
        check(top.get("contiguous_publication_watermark") == total, f"{policy} publication watermark not contiguous")
    else:
        check(out_of_order == 0, "ordered records published out of order")
    for field in ("successors", "conditional_successors", "optional_coefficient_refusals",
                  "optional_original_refusals", "optional_coalesced_refusals"):
        check(top.get(field) == apply_stats[field], f"{field} != Apply statistics")
    check(top.get("unbounded_rank_domains") == ranks[None], "unbounded_rank_domains mismatch")
    finite = max((rank for rank in ranks if rank is not None), default=None)
    check(top.get("max_scheduled_finite_rank") == finite, "max_scheduled_finite_rank mismatch")
    check(top.get("initial_entry_domains_total") == top.get("initial_entry_domains_inspected")
          == top.get("initial_entry_domains_published") == initial_count, "initial entry obligations not discharged")
    mapping = dict(inputs)
    check(len(inputs) == len(mapping) == query_count and set(mapping) == {query["id"] for query in queries}
          and all(type(record) is int for record in admitting) and list(admitting) == list(range(initial_count)),
          "inputs do not map every query to an initial record")
    check([query_id for query_id, _ in inputs] == [query["id"] for query in queries],
          "inputs are not in query document order")
    aliased = 0
    for query in queries:
        record = mapping.get(query["id"])
        row = initial.get(record) if type(record) is int and record < initial_count else None
        if not check(row is not None, f"query {query['id']}: no initial record"):
            continue
        check(row.get("record_kind") == "native_inspection" and row.get("phase") == "Apply",
              f"query {query['id']}: initial record is not an Apply native inspection")
        if admitting[record] != query["id"] and alias_contains(row, query):
            aliased += 1
            continue
        for field in ("owner", "lower", "upper", "power_bounds"):
            check(row.get(field) == query.get(field), f"query {query['id']}: {field} changed")
        check(row.get("rank") == query.get("max_numerator_rank"), f"query {query['id']}: rank changed")
    # Frontier/error/refusal parity: every retained frontier, failure and
    # refusal provenance entry is explicit in exactly one record or receipt.
    uncommitted = top.get("uncommitted_inspections")
    carried_frontiers = sum(len(row.get("frontiers") or []) for row in uncommitted
                            if isinstance(row, dict)) if isinstance(uncommitted, list) else 0
    input_frontiers = top.get("input_frontiers")
    input_frontiers = len(input_frontiers) if isinstance(input_frontiers, list) else 0
    check(top.get("frontiers") == input_frontiers + parity["record_frontiers"] + carried_frontiers,
          "frontier parity: top-level frontiers != input + record + carried frontiers")
    check(top.get("failed_nodes") == parity["error_records"], "error parity: failed_nodes != records with an error")
    check((top.get("error") is None) == (parity["error_records"] == 0), "error parity: walk error vs record errors")
    check(parity["refusal_provenance_entries"] <= apply_stats["optional_coefficient_refusals"],
          "refusal parity: more refusal provenance entries than optional coefficient refusals")
    closure = top.get("descendant_closure")
    closure = closure if isinstance(closure, dict) else {}
    available = closure.get("available") is True
    if require_closure and rescue:
        # Rescued walk: per physics query, below (the quarantined frontier
        # cones stay open by construction).
        check(available, "closure required: descendant closure unavailable")
    elif require_closure:
        # Every certified root needs an exhausted, frontier-free, error-free
        # cone; the checks above make that global, so every node must be closed.
        check(available, "closure required: descendant closure unavailable")
        check(closure.get("unresolved_domains") == 0, "closure required: unresolved_domains != 0")
        check(closure.get("initial_total") == initial_count and closure.get("initial_closed") == initial_count,
              "closure required: initial_closed != initial_total != distinct initial records")
        check(closure.get("total_domains") == closure.get("total_closed") == total,
              "closure required: total_closed != total_domains != logical records")
        check(closed_claims["true"] == total, "closure required: a record is not descendant_closed")
    if available:
        check(closure.get("initial_total") == initial_count, "closure initial_total != distinct initial records")
        check(closure.get("total_domains") == total, "closure total_domains != logical records")
        check(closure.get("total_closed") == closed_claims["true"] and
              closure.get("unresolved_domains") == total - closed_claims["true"],
              "closure counters != per-record descendant_closed claims")
    roles = {}
    for query in queries:
        record = mapping.get(query["id"])
        row = initial.get(record) if type(record) is int else None
        label = "helper" if query_roles[query["id"]] == "auxiliary" else "physics"
        role = "admitting" if type(record) is int and admitting.get(record) == query["id"] else "absorbed"
        closed = row is not None and row.get("descendant_closed") is True
        entry = roles.setdefault(label, Counter())
        entry["total"] += 1
        entry[role] += 1
        entry["closed"] += closed
    roots = [row for index, row in initial.items() if index < initial_count]
    certification = {
        "scope": "engine descendant_closed annotations re-checked for consistency; roots are the distinct "
                 "initial records; query roles are the immutable exact-ID declaration",
        "query_roles": "immutable_exact_id_declaration; undeclared_queries_required", "closure_available": available,
        "roots": {"total": initial_count, "closed": sum(row.get("descendant_closed") is True for row in roots)},
        "queries": {label: dict(counts) for label, counts in sorted(roles.items())},
        "record_closed_claims": dict(closed_claims),
    }
    if rescue:
        certification["rescue"] = rescue_certification(
            run, top, queries, amended, amendment_documents, raw_inputs, query_count, inputs, amended_inputs,
            total, containment, query_roles, require_closure, check)
    ledger = top.get("delegation")
    ledger = ledger if isinstance(ledger, dict) else {}
    blocked = {field: ledger.get(field) for field in RESCUE_BLOCKED} if rescue else {}
    blocked_total = sum(value for value in blocked.values() if type(value) is int)
    check(ledger.get("all_ledger_obligations_discharged") is (blocked_total == 0),
          "ledger obligations not discharged" if not rescue
          else "rescued walk: ledger discharge != no frontier-blocked obligation")
    check(ledger.get("logical_publications") == total, "ledger logical_publications != records")
    check(ledger.get("native_publications") == native_count
          and type(ledger.get("native_discharged")) is int
          and ledger.get("native_discharged") + (blocked.get("native_frontier_blocked") or 0) == native_count,
          "ledger native publications/discharges != native records")
    check(ledger.get("delegated_publications") == ledger.get("transferred_obligations")
          == kind_counts["delegated_not_inspected"]
          and ledger.get("delegated_resolved") == kind_counts["delegated_not_inspected"]
          - (blocked.get("delegated_frontier_blocked") or 0),
          "ledger delegated counts != alias records")
    check(ledger.get("partial_initial_inspections") == kind_counts["partial_initial_overlap_inspection"],
          "ledger partial_initial_inspections != partial records")
    if g2_records or "g2_records" in ledger:
        check(ledger.get("g2_records") == len(g2_records), "ledger g2_records != G2' records")
        check(ledger.get("g2_blocked") == g2_counts["blocked_responsibility_records"],
              "ledger g2_blocked != responsibility-blocked G2' records")
    for field in LEDGER_ZERO:
        if not (rescue and field in RESCUE_BLOCKED):
            check(ledger.get(field) == 0, f"ledger {field} must be 0")
    if rescue:
        check((blocked.get("native_frontier_blocked") or 0) >= parity["rescue_frontier_records"],
              "rescued walk: native_frontier_blocked < frontier-bearing native records")
    pool = top.get("parallel")
    pool = pool if isinstance(pool, dict) else {}
    # Attempt counters accumulate across checkpoint sessions. A paused session
    # lists the attempts it returned but never published as carried entries.
    # A crash after a periodic save leaves no such list: results that were
    # finished but unpolled (or in Ready escrow) at that save were counted as
    # returned, then re-inspected after the resume. A resumed run may therefore
    # exceed the count; the surplus is reported, not a violation.
    expected = native_count + carried
    returned = pool.get("returned_inspections")
    surplus = None
    if policy == "epoch":
        # Every merged inspection is a native record; discarded (requeued)
        # attempts are reported beside them, never merged.
        merged, discarded = pool.get("merged_inspections"), pool.get("discarded_inspections")
        if check(merged == native_count and type(discarded) is int and discarded >= 0
                 and returned == merged + discarded,
                 "epoch pool merged_inspections != native records or returned != merged + discarded"):
            surplus = discarded
    elif located["resumed"]:
        if check(type(returned) is int and returned >= expected,
                 "pool returned_inspections < native records + carried earlier-session attempts"):
            surplus = returned - expected
    elif check(returned == expected, "pool returned_inspections != native records + carried earlier-session attempts"):
        surplus = 0
    for field in POOL_ZERO:
        check(pool.get(field) == 0, f"pool {field} must be 0")
    check(pool.get("first_failure") is None and pool.get("non_cancellation_failure") is None, "pool recorded a failure")
    allocation = top.get("worker_allocation")
    allocation = allocation if isinstance(allocation, dict) else {}
    limits = tuple(allocation.get(name) for name in ("inspection_worker_limit", "admission_worker_limit",
                                                     "coordinator_worker_limit", "total_compute_worker_limit"))
    if located["workers"] is not None:
        check(top.get("workers") == located["workers"], "result workers != command --workers")
        check(limits[3] == located["workers"], "worker allocation total != command --workers")
    check(all(type(limit) is int and limit >= 0 for limit in limits) and limits[0] + limits[1] + limits[2] == limits[3],
          "worker allocation does not partition the total")
    check(pool.get("workers") == limits[0], "pool workers != inspection worker limit")
    if located["inspection_workers"] is not None:
        check(limits[0] == located["inspection_workers"], "inspection worker limit != command --inspection-workers")
    if located["lookahead"] is not None:
        scheduling = top.get("scheduling_policy")
        scheduling = scheduling if isinstance(scheduling, dict) else {}
        check(scheduling.get("lookahead") == located["lookahead"], "scheduling lookahead != command lookahead")
    receipt = None
    if located["receipt"] is not None:
        receipt = read_json(located["receipt"])
        # A rescued walk that drained with quarantined frontiers exits 4 (incomplete).
        rescued_drain = rescue and top.get("status") == "incomplete" and (top.get("frontiers") or 0) > 0
        check(receipt.get("exit_status") == (4 if rescued_drain else 0),
              "resource receipt exit_status != 0" if not rescued_drain
              else "rescued walk: resource receipt exit_status != 4 (drained with quarantined frontiers)")
        if "hard_stopped" in receipt:
            check(receipt.get("hard_stopped") is False, "supervisor hard-stopped the native process")
            check(receipt.get("operator_or_resource_stop") is None, "supervisor recorded a stop reason")
        else:
            check(receipt.get("forced") is False, "guard force-stopped the native process")
            check(receipt.get("stop_reason") is None and receipt.get("supervisor_error") is None,
                  "guard recorded a stop reason or error")
    checkpoint = top.get("checkpoint")
    manifest = None
    if isinstance(checkpoint, dict):
        check(checkpoint.get("state") == "saved" and checkpoint.get("paused") is False, "final checkpoint not a saved, unpaused state")
        check(checkpoint.get("pending_domains") == 0 and checkpoint.get("committed_domains") == total,
              "final checkpoint domain counts mismatch")
        check(checkpoint.get("completed_native_inspections") == native_count
              and checkpoint.get("committed_events") == top.get("events"), "final checkpoint native/event counts mismatch")
        if located["checkpoint"] is not None and policy == "epoch":
            export = located["checkpoint"] / "epoch-export.json"
            if check(export.is_file(), "epoch export directory has no epoch-export.json"):
                manifest = read_json(export)
                export_schema = manifest.get("schema")
                export_semantics = {1: 3, 2: 4}.get(export_schema) if type(export_schema) is int else None
                check(manifest.get("format") == "RUSTRED-EPOCH-EXPORT" and manifest.get("resumable") is False
                      and export_semantics is not None
                      and type(manifest.get("walk_semantics_version")) is int
                      and manifest["walk_semantics_version"] == export_semantics,
                      "epoch export manifest format/semantics")
                check(manifest.get("metadata") == checkpoint,
                      "epoch export manifest differs from the result's checkpoint bookkeeping")
        elif located["checkpoint"] is not None:
            latest = located["checkpoint"] / "latest.json"
            if check(latest.is_file(), "checkpoint directory has no latest.json"):
                manifest = read_json(latest)
                durable = manifest.get("metadata", {})
                timing = ("duration_seconds", "saved_unix_time", "save_seconds")
                check({k: v for k, v in durable.items() if k not in timing}
                      == {k: v for k, v in checkpoint.items() if k not in timing},
                      "durable checkpoint manifest differs from the result's checkpoint bookkeeping")
                check(manifest.get("kind") == "state", "checkpoint manifest kind != state")
    return {
        "result_sha256": result_sha, "query_sha256": digest(located["queries"]), "schema": schema,
        "logical_records": total, "native_inspections": native_count, "native_by_phase": dict(native_phases),
        "logical_by_phase": dict(phases), "record_kinds": dict(kind_counts),
        "aliases": kind_counts["delegated_not_inspected"], "partial_initial_inspections": kind_counts["partial_initial_overlap_inspection"],
        "initial_queries": query_count, "distinct_initial_records": initial_count, "aliased_queries": aliased,
        "out_of_order_records": out_of_order,
        "carried_earlier_session_attempts": carried, "resumed_unpublished_returned_inspections": surplus,
        "alias_containment_checks": alias_checks, "partial_anchor_containment_checks": len(partials),
        "g2_residual_anchor_checks": dict(g2_counts) if g2_records else None,
        "parity": dict(parity, input_frontiers=input_frontiers, carried_frontiers=carried_frontiers,
                       top_frontiers=top.get("frontiers"), failed_nodes=top.get("failed_nodes")),
        "certification": certification,
        "rank_histogram": {str(rank): value for rank, value in sorted(ranks.items(), key=lambda item: (item[0] is None, item[0]))},
        "apply_stats": dict(apply_stats), "route_stats": dict(route_stats),
        "events": top.get("events"), "max_scheduled_finite_rank": top.get("max_scheduled_finite_rank"),
        "prepared_seconds": top.get("prepared_seconds"), "traversal_seconds": top.get("traversal_seconds"),
        "native_session_seconds": top.get("elapsed_seconds"), "workers": top.get("workers"),
        "worker_allocation": allocation or None, "ledger": ledger or None, "checkpoint": checkpoint,
        "checkpoint_manifest_schema": None if manifest is None else manifest.get("schema"),
        "receipt": None if receipt is None else {key: receipt.get(key) for key in
                                                  ("exit_status", "elapsed_seconds", "peak_observed_aggregate_rss_bytes",
                                                   "peak_sampled_aggregate_rss_bytes", "child_max_rss_kib",
                                                   "child_user_seconds", "child_system_seconds")},
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("run", type=Path, help="run directory holding result.json (supervisor or bare layout)")
    parser.add_argument("--queries", type=Path, help="query document; default: the command's --queries")
    parser.add_argument("--command", type=Path, help="bare native argv JSON; default: request.json or command.json")
    parser.add_argument("--supervisor-receipt", type=Path, help="default: supervisor-result.json or guard/result.json")
    parser.add_argument("--expect-schema", help="require this exact result schema string")
    parser.add_argument("--require-closure", action="store_true",
                        help="require every root and record descendant-closed (exhausted, frontier- and error-free)")
    parser.add_argument("--brute-force-max-points", type=int, default=256,
                        help="enumerate containment checks whose inner set has at most this many lattice points (0: off)")
    parser.add_argument("--brute-force-point-budget", type=int, default=2_000_000,
                        help="total lattice points the brute-force cross-check may enumerate")
    parser.add_argument("--verify-report", type=Path,
                        help="a `rustred walk-verify-closure` report that must be bound to this very result.json")
    parser.add_argument("--output", type=Path, help="audit report path; default RUN/audit.json")
    parser.add_argument("--no-output", action="store_true", help="print only; do not write audit.json")
    args = parser.parse_args(argv)
    started = time.monotonic()
    report = audit_walk(args.run, args.queries, args.command, args.supervisor_receipt, args.expect_schema,
                        args.require_closure, Containment(args.brute_force_max_points, args.brute_force_point_budget),
                        args.verify_report)
    report["audit_seconds"] = time.monotonic() - started
    text = json.dumps(report, indent=2, sort_keys=True, allow_nan=False)
    if not args.no_output:
        output = args.output or (args.run / "audit.json")
        output.write_text(text + "\n")
    print(text)
    return 0 if report["audit"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
