#!/usr/bin/env python3
"""Independently check a planned renormalization-entry query document.

Role: offline input validation; native admission remains the coverage
authority. This checker does not import the planner. From the receipt's
physics block and each owner's (class, V4min, t) it re-derives every bound,
then verifies the query document byte-for-byte against the receipt:

- exact `rustred.owner-domain-queries.json.v2` row shape (six fields, three
  power-bound fields, nothing else), unique IDs of at most 128 bytes, byte
  size and SHA-256 equal to the receipt;
- per owner in receipt order: exactly one full-orthant helper first (zero for
  owners without roots), then roots by descending R_max, with the root count
  implied by the owner's class and the planner options;
- every root semantically contained in its helper under the walker's
  `Domain::contains` predicate (lower <=, upper none-or->=, rank, power bounds);
- closed-form tuple counts sum_A C(A-1,t-1) * sum_R C(R+m-1,m-1) over the
  admitted (A, R) band equal to the receipt's Rust `entry-domain-plan` counts
  (the closed form is first validated on the 1,324-tuple five-loop control);
- random membership probes agreeing between the box predicate (coordinates,
  rank, power bounds) and the physical predicate (A, R, D totals).

Exit status is nonzero on any mismatch; a JSON summary goes to stdout.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import sys

QUERY_SCHEMA = "rustred.owner-domain-queries.json.v2"
RECEIPT_SCHEMA = "rustred.renormalization-entry-plan.json.v1"
ENTRY_PLAN_SCHEMA = "rustred.entry-domain-plan.json.v1"
HELPER_PREFIX = "owner-anchor-"
ROW_FIELDS = {"id", "owner", "lower", "upper", "max_numerator_rank", "power_bounds"}
POWER_FIELDS = {"max_positive_power", "min_power_difference", "max_power_difference"}
CONTROL = {"owner": "011101110111000", "A_max": 11, "R_max": 2, "D_min": 9, "D_max": None, "count": 1324}


class Failure(Exception):
    pass


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def closed_form_count(active, inactive, a_max, r_max, d_min, d_max):
    total = 0
    for a in range(max(active, d_min if d_min is not None else 0), a_max + 1):
        r_low = 0 if d_max is None else max(0, a - d_max)
        r_high = r_max if d_min is None else min(r_max, a - d_min)
        if inactive == 0:
            r_high = min(r_high, 0)
        if r_high < r_low:
            continue
        positive = 1 if active == 0 else math.comb(a - 1, active - 1)
        ranks = sum(1 if inactive == 0 else math.comb(r + inactive - 1, inactive - 1) for r in range(r_low, r_high + 1))
        total += positive * ranks
    return total


def expected_bounds(klass, excess, loops, powers, difference_set, factorized_roots, non_entry_roots):
    """(prefix, A_max, R_max, D_min, D_max) per expected root, descending R_max."""

    def connected(prefix, d, v4):
        rank = d - loops + 1 - v4 + powers
        return (prefix, rank + d, rank, d, d)

    if klass == "connected":
        rows = [connected("phys", d, excess) for d in difference_set]
    elif klass == "factorized":
        if factorized_roots == "nested":
            rows = [("nested", 5 * loops - 1 - excess + powers, 3 * loops - excess + powers, 2 * loops - 1, None)]
        elif factorized_roots == "box":
            rows = [connected("fact", d, excess) for d in difference_set]
        else:
            rows = []
    elif klass == "non_entry":
        rows = [connected("conv", d, 0) for d in difference_set] if non_entry_roots == "widest" else []
    else:
        raise Failure(f"unknown owner class {klass!r}")
    return sorted(rows, key=lambda r: (-r[2], -r[1], r[0]))


def root_id(prefix, mask, a_max, r_max, d_min, d_max):
    label = f"d{d_min}" if d_max == d_min else f"d{d_min}p"
    return f"{prefix}-{label}-a{a_max}-r{r_max}-{mask}"


def contains(container, candidate):
    """Port of walking/queue.rs Domain::contains and DomainPowerBounds::contains."""
    if container["owner"] != candidate["owner"]:
        return False
    r1, r2 = container["max_numerator_rank"], candidate["max_numerator_rank"]
    if r1 is not None and (r2 is None or r2 > r1):
        return False
    p1, p2 = container["power_bounds"], candidate["power_bounds"]
    if p1["max_positive_power"] is not None and (p2["max_positive_power"] is None or p2["max_positive_power"] > p1["max_positive_power"]):
        return False
    if p1["min_power_difference"] is not None and (p2["min_power_difference"] is None or p2["min_power_difference"] < p1["min_power_difference"]):
        return False
    if p1["max_power_difference"] is not None and (p2["max_power_difference"] is None or p2["max_power_difference"] > p1["max_power_difference"]):
        return False
    if any(a > b for a, b in zip(container["lower"], candidate["lower"])):
        return False
    return all(a is None or (b is not None and b <= a) for a, b in zip(container["upper"], candidate["upper"]))


def box_predicate(query, point):
    if any(x < lo for x, lo in zip(point, query["lower"])):
        return False
    if any(hi is not None and x > hi for x, hi in zip(point, query["upper"])):
        return False
    mask = query["owner"]
    a = sum(1 + x for x, bit in zip(point, mask) if bit == "1")
    r = sum(x for x, bit in zip(point, mask) if bit == "0")
    rank = query["max_numerator_rank"]
    if rank is not None and r > rank:
        return False
    powers = query["power_bounds"]
    if powers["max_positive_power"] is not None and a > powers["max_positive_power"]:
        return False
    if powers["min_power_difference"] is not None and a - r < powers["min_power_difference"]:
        return False
    return powers["max_power_difference"] is None or a - r <= powers["max_power_difference"]


def physical_predicate(mask, a_max, r_max, d_min, d_max, point):
    a = sum(1 + x for x, bit in zip(point, mask) if bit == "1")
    r = sum(x for x, bit in zip(point, mask) if bit == "0")
    if a > a_max or r > r_max:
        return False
    if d_min is not None and a - r < d_min:
        return False
    return d_max is None or a - r <= d_max


def random_composition(rng, total, parts, positive):
    """Random weak (or positive) composition of total into parts."""
    if parts == 0:
        return []
    if positive:
        return [1 + x for x in random_composition(rng, total - parts, parts, False)]
    cuts = sorted(rng.randint(0, total) for _ in range(parts - 1))
    return [b - a for a, b in zip([0] + cuts, cuts + [total])]


def probe_root(rng, query, mask, a_max, r_max, d_min, d_max, probes):
    t = mask.count("1")
    m = len(mask) - t
    disagreements = 0
    inside = 0
    for index in range(probes):
        if index % 2 == 0:
            point = [rng.randint(0, (hi if hi is not None else 0) + 2) for hi in query["upper"]]
        else:
            a_low = max(t, d_min if d_min is not None else t)
            if a_low > a_max:
                continue
            a = rng.randint(a_low, a_max)
            r_low = 0 if d_max is None else max(0, a - d_max)
            r_high = r_max if d_min is None else min(r_max, a - d_min)
            if m == 0:
                r_high = min(r_high, 0)
            if r_high < r_low:
                continue
            r = rng.randint(r_low, r_high)
            active = iter([x - 1 for x in random_composition(rng, a, t, True)])
            inactive = iter(random_composition(rng, r, m, False))
            point = [next(active) if bit == "1" else next(inactive) for bit in mask]
        box = box_predicate(query, point)
        physical = physical_predicate(mask, a_max, r_max, d_min, d_max, point)
        inside += physical
        disagreements += box != physical
    return disagreements, inside


def check(queries_path, receipt_path, entry_plans, probes, seed, allow_closed_form_only):
    failures = []

    def expect(condition, message):
        if not condition:
            failures.append(message)

    control_count = closed_form_count(CONTROL["owner"].count("1"), CONTROL["owner"].count("0"), CONTROL["A_max"],
                                      CONTROL["R_max"], CONTROL["D_min"], CONTROL["D_max"])
    if control_count != CONTROL["count"]:
        raise Failure(f"closed form gives {control_count} on the known control, expected {CONTROL['count']}")

    receipt = json.loads(Path(receipt_path).read_bytes(), object_pairs_hook=unique_object)
    expect(receipt.get("schema") == RECEIPT_SCHEMA, "receipt schema")
    expect(receipt.get("family_closure_claim") is False, "receipt must keep family_closure_claim false")
    expect(receipt.get("descendant_clipping") is False, "receipt must keep descendant_clipping false")
    physics = receipt.get("physics", {})
    loops = physics.get("loops")
    powers = physics.get("gauge_parameter_powers")
    difference_set = physics.get("difference_set")
    factorized_roots = physics.get("factorized_roots")
    non_entry_roots = physics.get("non_entry_roots")
    positive_power_owners = set(physics.get("helper_positive_power_owners", []))
    if (type(loops) is not int or type(powers) is not int or not isinstance(difference_set, list)
            or factorized_roots not in ("nested", "box", "omit") or non_entry_roots not in ("widest", "omit")):
        raise Failure("receipt physics block is incomplete")
    difference_set = sorted(difference_set, reverse=True)

    queries_bytes = Path(queries_path).read_bytes()
    summary = receipt.get("summary", {})
    expect(hashlib.sha256(queries_bytes).hexdigest() == summary.get("queries_sha256"), "queries sha256 differs from the receipt")
    expect(len(queries_bytes) == summary.get("queries_bytes"), "queries byte size differs from the receipt")
    document = json.loads(queries_bytes, object_pairs_hook=unique_object)
    expect(isinstance(document, dict) and set(document) == {"schema", "queries"}, "query document keys")
    expect(document.get("schema") == QUERY_SCHEMA, "query schema")
    rows = document.get("queries") if isinstance(document, dict) else None
    if not isinstance(rows, list) or not rows:
        raise Failure("queries must be a nonempty list")
    arity = None
    ids = set()
    for row in rows:
        expect(isinstance(row, dict) and set(row) == ROW_FIELDS, f"row field set {sorted(row) if isinstance(row, dict) else row}")
        if not isinstance(row, dict):
            continue
        query_id = row.get("id")
        expect(isinstance(query_id, str) and 0 < len(query_id.encode("utf-8")) <= 128, f"id length {query_id!r}")
        expect(query_id not in ids, f"duplicate id {query_id!r}")
        ids.add(query_id)
        mask = row.get("owner")
        expect(isinstance(mask, str) and mask and not set(mask) - {"0", "1"}, f"{query_id}: owner mask")
        arity = arity or len(mask)
        expect(len(mask) == arity, f"{query_id}: arity")
        expect(row.get("lower") == [0] * arity, f"{query_id}: lower must be all zero")
        upper = row.get("upper")
        expect(isinstance(upper, list) and len(upper) == arity and all(u is None or (type(u) is int and u >= 0) for u in upper),
               f"{query_id}: upper shape")
        rank = row.get("max_numerator_rank")
        expect(rank is None or (type(rank) is int and rank >= 0), f"{query_id}: rank")
        power = row.get("power_bounds")
        expect(isinstance(power, dict) and set(power) == POWER_FIELDS, f"{query_id}: power_bounds field set")
        if isinstance(power, dict):
            expect(power.get("max_positive_power") is None or (type(power["max_positive_power"]) is int and power["max_positive_power"] >= 0),
                   f"{query_id}: max_positive_power")
            for key in ("min_power_difference", "max_power_difference"):
                expect(power.get(key) is None or type(power[key]) is int, f"{query_id}: {key}")
    if failures:
        return failures, {}

    owners = receipt.get("owners")
    if not isinstance(owners, list) or not owners:
        raise Failure("receipt must list owners")
    order = {row["owner"]: index for index, row in enumerate(owners)}
    expect(len(order) == len(owners), "receipt owners must be unique")
    expect(positive_power_owners <= set(order), "helper positive-power owners must be receipt owners")
    by_owner = {}
    for row in rows:
        by_owner.setdefault(row["owner"], []).append(row)
    expect(set(by_owner) <= set(order), "query owners must be receipt owners")
    sequence = [row["owner"] for row in rows]
    blocks = [sequence[0]] + [b for a, b in zip(sequence, sequence[1:]) if a != b]
    expect(len(blocks) == len(set(blocks)), "queries of one owner must be contiguous")
    expect(blocks == sorted(blocks, key=order.get), "owner blocks must follow receipt owner order")

    rng = random.Random(seed)
    total_closed = 0
    total_rust = 0
    rust_counts = True
    group_expect = {}
    root_count = helper_count = 0
    probe_stats = {"points": 0, "inside": 0, "disagreements": 0}
    class_counts = {}
    for owner_row in owners:
        mask = owner_row["owner"]
        klass = owner_row.get("class")
        class_counts[klass] = class_counts.get(klass, 0) + 1
        t = mask.count("1")
        expect(owner_row.get("t") == t, f"{mask}: receipt t")
        excess = owner_row.get("V4min")
        if klass == "non_entry":
            expect(excess is None and owner_row.get("entry_capable") is False, f"{mask}: non-entry row shape")
        else:
            expect(type(excess) is int and excess >= 0 and owner_row.get("entry_capable") is True, f"{mask}: entry row shape")
            expect((klass == "factorized") == bool(owner_row.get("factorized")), f"{mask}: factorized flag vs class")
        expected = expected_bounds(klass, excess if excess is not None else 0, loops, powers, difference_set,
                                   factorized_roots, non_entry_roots)
        expected_roots = []
        for prefix, a_max, r_max, d_min, d_max in expected:
            expect(a_max - t >= 0 and r_max >= 0, f"{mask}: derived bounds negative ({a_max}, {r_max})")
            expected_roots.append({"id": root_id(prefix, mask, a_max, r_max, d_min, d_max), "A_max": a_max, "R_max": r_max,
                                   "D_min": d_min, "D_max": d_max, "active_upper": a_max - t, "inactive_upper": r_max})
        receipt_roots = owner_row.get("roots", [])
        expect([r.get("id") for r in receipt_roots] == [r["id"] for r in expected_roots], f"{mask}: receipt root ids/order")
        block = by_owner.get(mask, [])
        if not expected_roots:
            expect(owner_row.get("helper") is None and block == [], f"{mask}: owner without roots must have no queries")
            continue
        helper_rank = max(r["R_max"] for r in expected_roots)
        helper_power = max(r["A_max"] for r in expected_roots) if mask in positive_power_owners else None
        helper_id = f"{HELPER_PREFIX}r{helper_rank}-a{'none' if helper_power is None else helper_power}-{mask}"
        expect(owner_row.get("helper") == {"id": helper_id, "max_numerator_rank": helper_rank, "max_positive_power": helper_power},
               f"{mask}: receipt helper")
        ordered = [q["id"] for q in block] == [helper_id] + [r["id"] for r in expected_roots]
        expect(ordered, f"{mask}: query ids/order")
        helpers = [q for q in block if q["id"].startswith(HELPER_PREFIX)]
        expect(len(helpers) == 1 and block and block[0] is helpers[0], f"{mask}: exactly one helper, first")
        if not ordered:
            continue
        helper = block[0]
        helper_count += 1
        expect(helper == {"id": helper_id, "owner": mask, "lower": [0] * arity, "upper": [None] * arity,
                          "max_numerator_rank": helper_rank,
                          "power_bounds": {"max_positive_power": helper_power, "min_power_difference": None,
                                           "max_power_difference": None}}, f"{mask}: helper row")
        for query, root, receipt_root in zip(block[1:], expected_roots, receipt_roots):
            root_count += 1
            upper = [root["active_upper"] if bit == "1" else root["inactive_upper"] for bit in mask]
            expect(query == {"id": root["id"], "owner": mask, "lower": [0] * arity, "upper": upper,
                             "max_numerator_rank": root["R_max"],
                             "power_bounds": {"max_positive_power": root["A_max"], "min_power_difference": root["D_min"],
                                              "max_power_difference": root["D_max"]}}, f"{root['id']}: root row")
            for key in ("A_max", "R_max", "D_min", "D_max", "active_upper", "inactive_upper"):
                expect(receipt_root.get(key) == root[key], f"{root['id']}: receipt {key}")
            expect(contains(helper, query), f"{root['id']}: not contained in its helper")
            count = closed_form_count(t, arity - t, query["power_bounds"]["max_positive_power"], query["max_numerator_rank"],
                                      query["power_bounds"]["min_power_difference"], query["power_bounds"]["max_power_difference"])
            expect(count > 0, f"{root['id']}: empty band")
            expect(receipt_root.get("closed_form_count") == count, f"{root['id']}: receipt closed_form_count")
            total_closed += count
            rust = receipt_root.get("target_count")
            if rust is None:
                rust_counts = False
            else:
                expect(isinstance(rust, str) and rust.isdecimal() and int(rust) == count, f"{root['id']}: Rust count {rust} != {count}")
                total_rust += count
            key = (root["A_max"], root["R_max"], root["D_min"], root["D_max"])
            group_expect.setdefault(key, []).append((mask, root["id"], count))
            disagreements, inside = probe_root(rng, query, mask, root["A_max"], root["R_max"], root["D_min"], root["D_max"], probes)
            probe_stats["points"] += probes
            probe_stats["inside"] += inside
            probe_stats["disagreements"] += disagreements
            expect(disagreements == 0, f"{root['id']}: box and physical predicates disagree on {disagreements} probes")

    expect(summary.get("query_count") == len(rows) == helper_count + root_count, "summary query_count")
    expect(summary.get("helper_count") == helper_count and summary.get("root_count") == root_count, "summary helper/root counts")
    expect(summary.get("class_counts") == class_counts, "summary class counts")
    expect(summary.get("total_target_count") == str(total_closed), "summary total_target_count vs closed form")
    if not rust_counts:
        expect(allow_closed_form_only, "receipt lacks Rust target counts (use --allow-closed-form-only to accept)")
    groups = summary.get("budget_groups", [])
    seen_groups = set()
    for group in groups:
        budget = group.get("budget", {})
        key = (budget.get("max_positive_power"), budget.get("max_numerator_rank"), budget.get("min_power_difference"),
               budget.get("max_power_difference"))
        seen_groups.add(key)
        members = group_expect.get(key, [])
        expect(group.get("sectors") == [m for m, _, _ in members], f"group {group.get('name')}: sectors")
        expect(group.get("root_ids") == [i for _, i, _ in members], f"group {group.get('name')}: root ids")
        expect(group.get("total_target_count") == str(sum(c for _, _, c in members)), f"group {group.get('name')}: total")
        plan_path = group.get("plan_path")
        if plan_path is not None:
            plan = json.loads((Path(entry_plans) / Path(plan_path).name).read_bytes(), object_pairs_hook=unique_object)
            expect(plan.get("schema") == ENTRY_PLAN_SCHEMA and plan.get("counts_exact") is True, f"{plan_path}: schema")
            expect(plan.get("budget") == budget, f"{plan_path}: budget")
            expect([(s.get("sector"), s.get("target_count")) for s in plan.get("sectors", [])] == [(m, str(c)) for m, _, c in members],
                   f"{plan_path}: per-sector counts")
            expect(plan.get("total_target_count") == group.get("total_target_count"), f"{plan_path}: total")
        elif rust_counts:
            expect(False, f"group {group.get('name')}: Rust counts without a plan path")
    expect(seen_groups == set(group_expect), "budget groups must cover exactly the planned roots")
    report = {"queries": str(queries_path), "receipt": str(receipt_path), "query_count": len(rows),
              "helper_count": helper_count, "root_count": root_count, "class_counts": class_counts,
              "closed_form_total": str(total_closed), "rust_total": str(total_rust) if rust_counts else None,
              "rust_counts_verified": rust_counts, "probes": probe_stats, "control_count": control_count,
              "queries_sha256": hashlib.sha256(queries_bytes).hexdigest()}
    return failures, report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--queries", type=Path, required=True)
    parser.add_argument("--receipt", type=Path, required=True)
    parser.add_argument("--entry-plans", type=Path, help="directory with per-group plans (default: next to the receipt)")
    parser.add_argument("--probes", type=int, default=200, help="membership probes per root")
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--allow-closed-form-only", action="store_true")
    args = parser.parse_args(argv)
    entry_plans = args.entry_plans or args.receipt.parent / "entry-plans"
    try:
        failures, report = check(args.queries, args.receipt, entry_plans, args.probes, args.seed, args.allow_closed_form_only)
    except (Failure, OSError, ValueError, KeyError, TypeError) as error:
        print(json.dumps({"status": "error", "error": f"{type(error).__name__}: {error}"}, sort_keys=True))
        return 2
    report["status"] = "pass" if not failures else "fail"
    report["failures"] = failures
    report["family_closure_claim"] = False
    print(json.dumps(report, sort_keys=True, indent=2))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
