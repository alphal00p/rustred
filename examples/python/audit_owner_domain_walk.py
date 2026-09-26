#!/usr/bin/env python3
"""Streaming, phase-aware audit of an exhausted Ordered/Ready owner-domain walk.

Every logical record of `result.json` is streamed once with bounded memory:
aliases must resolve to a same-phase, same-owner completed native
representative; Apply/Route native statistics must be internally consistent
(zero problems, zero missing routes, successor sums); the queue, delegation
ledger and worker pool must be drained; frontiers must be zero; initial and
partial-anchor obligations must be discharged; the input queries must be
preserved: each distinct initial record equals the query that admitted it, and
a later query may only alias into an earlier same-owner initial record that
syntactically contains it (helpers-first query documents). Violations are
collected, written to `audit.json` and cause a nonzero exit. This checks
recorded native completion and explicit dependencies only; it does not replay
IBP identities or certify family closure.
"""
from __future__ import annotations

import argparse
from array import array
import codecs
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys
import time

MAX_VALUE_BYTES = 64 * 1024 * 1024
SENTINEL = 2 ** 64 - 1
NATIVE, DELEGATED, PARTIAL = 1, 2, 3
KIND_CODES = {"native_inspection": NATIVE, "delegated_not_inspected": DELEGATED,
              "partial_initial_overlap_inspection": PARTIAL}
NATIVE_AUTHORITY = "same_snapshot_phase_owner_native_summary"
ROUTE_EVENT_PARTS = ("apply_domains", "route_domains", "zero_sectors", "missing_routes")
LEDGER_ZERO = ("native_frontier_blocked", "native_failed", "native_cancelled", "pending_native_publications",
               "delegated_pending", "delegated_frontier_blocked", "delegated_failure_blocked",
               "delegated_cancelled", "partial_initial_blocked")
POOL_ZERO = ("active_workers", "occupied_native_slots", "dispatched_uncommitted_domains",
             "finished_uncommitted_domains", "worker_buffered_events",
             "worker_buffered_logical_bytes", "completed_escrow_entries")
TOP_ZERO = ("queued_nodes", "frontiers", "failed_nodes", "pending_descendant_domains")
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
    return json.loads(Path(path).read_text())


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


def check_native_stats(audit, phase, stats, identity):
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
        audit.check(numeric.get("events") == sum(numeric.get(key, 0) for key in ROUTE_EVENT_PARTS),
                    f"record {identity}: Route event accounting mismatch")
    return numeric


def within(limit, value, sign=1):
    """Rust `limit.is_none_or(|a| value.is_some_and(|b| sign * b <= sign * a))`; None is unbounded."""
    return limit is None or (value is not None and sign * value <= sign * limit)


def alias_contains(record, query):
    """Whether a later initial query may alias into an earlier admitted initial record.

    Mirrors the walker's syntactic `Domain::contains` (walking/queue.rs): same
    owner, `rank_contains`, `DomainPowerBounds::contains` and per-axis lower and
    upper bounds, None meaning unbounded (a missing query power field parses as
    None). Initial queries pass through the full `Queue::admit`: exact key, the
    dominant full orthant (the lower=0, upper=None, unconstrained-power special
    case of this predicate) or general containment, so the target need not be a
    full orthant. The unlimited lane decides with the stronger
    `DomainPowerSummary` inclusion; only this sufficient implication is accepted
    here, so a semantic-only alias is still reported as a changed query.
    """
    owner, powers, other = record.get("owner"), record.get("power_bounds"), query.get("power_bounds")
    if not (isinstance(owner, str) and owner == query.get("owner") and isinstance(powers, dict)
            and set(powers) == set(POWER_FIELDS) and isinstance(other, dict) and set(other) <= set(POWER_FIELDS)):
        return False
    lower, inner_lower, upper, inner_upper = axes = tuple(
        source.get(field) for field in ("lower", "upper") for source in (record, query))
    if not all(isinstance(axis, list) and len(axis) == len(owner) for axis in axes):
        return False
    limits = [record.get("rank"), query.get("max_numerator_rank")]
    limits += [bounds.get(field) for bounds in (powers, other) for field in POWER_FIELDS]
    if not (all(type(value) is int for value in lower + inner_lower)
            and all(value is None or type(value) is int for value in upper + inner_upper + limits)):
        return False
    return (within(record.get("rank"), query.get("max_numerator_rank"))
            and within(powers["max_positive_power"], other.get("max_positive_power"))
            and within(powers["min_power_difference"], other.get("min_power_difference"), -1)
            and within(powers["max_power_difference"], other.get("max_power_difference"))
            and all(outer <= inner for outer, inner in zip(lower, inner_lower))
            and all(within(outer, inner) for outer, inner in zip(upper, inner_upper)))


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
    return {"command": argv, "queries": Path(queries_path), "receipt": receipt_path,
            "policy": flag_value(argv, "--publication-policy", "ordered"),
            "workers": None if workers is None else int(workers),
            "inspection_workers": None if inspectors is None else int(inspectors),
            "lookahead": None if lookahead is None else int(lookahead),
            "checkpoint": None if checkpoint is None else Path(checkpoint),
            "resumed": "--resume" in argv}


def audit_walk(run, queries=None, command=None, receipt=None, expect_schema=None):
    run = Path(run)
    audit = Audit()
    report = {"audit": "FAIL", "run_directory": str(run), "family_closure_claim": False,
              "all_local_obligations_discharged": False, "full_family_closure_claim": False,
              "scope": "recorded native completion and explicit dependencies; not IBP replay or termination"}
    try:
        located = locate(run, queries, command, receipt)
        report.update(native_command=located["command"], queries_path=str(located["queries"]),
                      publication_policy=located["policy"], resumed=located["resumed"],
                      receipt_path=None if located["receipt"] is None else str(located["receipt"]))
        report.update(_audit(run, located, audit, expect_schema))
    except (OSError, ValueError, KeyError, TypeError, IndexError, OverflowError) as error:
        audit.check(False, f"structural: {type(error).__name__}: {error}")
    report["violations"] = audit.violations
    report["violations_suppressed"] = audit.suppressed
    report["audit"] = "PASS" if not audit.violations else "FAIL"
    report["all_local_obligations_discharged"] = report["audit"] == "PASS"
    return report


def _audit(run, located, audit, expect_schema):
    check = audit.check
    policy = located["policy"]
    check(policy in ("ordered", "ready"), f"unsupported publication policy {policy!r}")
    queries_document = read_json(located["queries"])
    queries = queries_document["queries"]
    check(bool(queries) and len({query["id"] for query in queries}) == len(queries),
          "queries must be nonempty with unique ids")
    arity = len(queries[0]["owner"])
    check(all(len(query["owner"]) == arity for query in queries), "queries must share one owner arity")
    check(arity <= 62, "owner arity above 62 is not supported by the bounded audit")
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
    for item in stream_walk(run / "result.json"):
        if item[0] == "sha256":
            result_sha = item[1]
            continue
        if item[0] == "top":
            top[item[1]] = item[2]
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
        owner_phase[identity] = ((int(owner, 2) << 1) | int(phase == "Route")) if valid_owner else SENTINEL
        if identity < query_count:
            initial[identity] = row
        if code == DELEGATED:
            check(row.get("local_inspection_finished") is False, f"record {identity}: alias claims local inspection")
            check(row.get("responsibility_status") == "discharged_by_representative",
                  f"record {identity}: alias responsibility_status")
            check(row.get("containment_authority") == NATIVE_AUTHORITY, f"record {identity}: alias containment authority")
            representative, resolved = row.get("representative_id"), row.get("final_representative_id")
            if check(type(representative) is int and representative > identity and type(resolved) is int and resolved >= 0,
                     f"record {identity}: alias representative must be a later id"):
                direct[identity] = representative
                final[identity] = resolved
            continue
        check(row.get("error") is None, f"record {identity}: native error recorded")
        check(row.get("frontiers") == [], f"record {identity}: nonzero frontiers")
        check(row.get("local_classification_discharged") is True, f"record {identity}: classification not discharged")
        if code == NATIVE:
            check(row.get("local_inspection_finished") is True, f"record {identity}: native inspection unfinished")
        else:
            check(phase == "Apply", f"record {identity}: partial inspection outside Apply")
            check(row.get("local_inspection_finished") is False, f"record {identity}: partial claims full inspection")
            check(row.get("residual_inspection_finished") is True, f"record {identity}: residual inspection unfinished")
            check(row.get("responsibility_status") == "discharged_by_residual_and_initial_anchor",
                  f"record {identity}: partial responsibility_status")
            link = row.get("initial_overlap")
            link = link if isinstance(link, dict) else {}
            check(link.get("coordinates_and_rank_unchanged") is True, f"record {identity}: partial overlap changed coordinates")
            check(link.get("authority") == NATIVE_AUTHORITY, f"record {identity}: partial overlap authority")
            anchor = link.get("anchor_id")
            if check(type(anchor) is int and anchor >= 0, f"record {identity}: partial anchor id missing"):
                final[identity] = anchor
        if phase == "Route":
            check(row.get("conservative_route_overcover") is True, f"record {identity}: Route without conservative overcover")
        numeric = check_native_stats(audit, phase, row.get("stats"), identity)
        target = apply_stats if phase == "Apply" else route_stats
        for field, value in numeric.items():
            target[field] += value
        native_phases[phase] += 1
        if policy == "ready":
            events = row.get("accepted_events")
            if check(type(events) is int and events >= 0, f"record {identity}: ready record lacks accepted_events"):
                accepted += events
    check(top.get("domains") == "<streamed>", "result has no domains array")
    check(count > 0, "no domain records")
    check(count == len(kinds) and all(kinds[index] != 0 for index in range(len(kinds))),
          "record ids are not a contiguous 0..n-1 set")
    total = len(kinds)
    # Queries are admitted in "inputs" order; a later query may alias into an
    # admitted record, so the first query naming a record is the one it admits.
    inputs = top.get("inputs")
    inputs = [(entry["id"], entry["domain"]) for entry in inputs] if isinstance(inputs, list) else []
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
            check(kinds[target] in (NATIVE, PARTIAL), f"record {identity}: final representative is not native")
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
    check(top.get("status") == "locally_resolved" and top.get("error") is None, "walk status is not locally_resolved")
    check(top.get("all_scheduled_domains_resolved") is True, "not all scheduled domains resolved")
    check(top.get("recursive_worklist_exhausted") is True, "recursive worklist not exhausted")
    for field in TOP_ZERO:
        check(top.get(field) == 0, f"{field} must be 0")
    check(top.get("input_frontiers") == [], "input_frontiers must be empty")
    check(top.get("uncommitted_inspections") == [], "uncommitted_inspections must be empty")
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
    if policy == "ready":
        check(accepted == top.get("events"), "ready accepted_events do not sum to events")
        check(top.get("contiguous_publication_watermark") == total, "ready publication watermark not contiguous")
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
    ledger = top.get("delegation")
    ledger = ledger if isinstance(ledger, dict) else {}
    check(ledger.get("all_ledger_obligations_discharged") is True, "ledger obligations not discharged")
    check(ledger.get("logical_publications") == total, "ledger logical_publications != records")
    check(ledger.get("native_publications") == ledger.get("native_discharged") == native_count,
          "ledger native publications/discharges != native records")
    check(ledger.get("delegated_publications") == ledger.get("delegated_resolved")
          == ledger.get("transferred_obligations") == kind_counts["delegated_not_inspected"],
          "ledger delegated counts != alias records")
    check(ledger.get("partial_initial_inspections") == kind_counts["partial_initial_overlap_inspection"],
          "ledger partial_initial_inspections != partial records")
    for field in LEDGER_ZERO:
        check(ledger.get(field) == 0, f"ledger {field} must be 0")
    pool = top.get("parallel")
    pool = pool if isinstance(pool, dict) else {}
    check(pool.get("returned_inspections") == native_count, "pool returned_inspections != native records")
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
        check(receipt.get("exit_status") == 0, "resource receipt exit_status != 0")
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
        if located["checkpoint"] is not None:
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
    parser.add_argument("--output", type=Path, help="audit report path; default RUN/audit.json")
    parser.add_argument("--no-output", action="store_true", help="print only; do not write audit.json")
    args = parser.parse_args(argv)
    started = time.monotonic()
    report = audit_walk(args.run, args.queries, args.command, args.supervisor_receipt, args.expect_schema)
    report["audit_seconds"] = time.monotonic() - started
    text = json.dumps(report, indent=2, sort_keys=True, allow_nan=False)
    if not args.no_output:
        output = args.output or (args.run / "audit.json")
        output.write_text(text + "\n")
    print(text)
    return 0 if report["audit"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
