#!/usr/bin/env python3
"""Mutation matrix for the W0.2 closure oracles: exact outcomes, not just FAIL.

Two oracles are exercised on real walk outputs:

* `rustred walk-verify-closure`, whose `--mutate KIND` injects one defect in
  memory after loading the checkpoint (the checkpoint is only read);
* the streaming result audit (`audit_owner_domain_walk.py --require-closure`),
  on mutated copies of `result.json`.

`--run` must be a drained walk; `--frontier-run` a walk that retains frontiers
(the unrestricted-helper FG fixture: `--frontier-expect-closed 60/124`).

Every Rust row asserts, beyond the verdict and exit status (PASS 0, FAIL 1,
INCOMPLETE 9):
* the EXACT set of violation classes (a class that is merely present is not
  enough; a missing or an extra class fails the row). Per-node F10 classes
  (`frontier_parity`, `error_parity`, `event_parity`, `successor_parity`,
  `successor_uncovered`) are distinct from the global counter classes
  (`frontier_counter`, `native_counter`, `event_counter`), so the exact set
  proves that the per-node re-inspection check fired;
* per-class counts where the defect is a single node (hidden frontier, hidden
  error, miscounted events/successors: exactly 1);
* the closure effect: every mutation runs with `--require-closure`, and the
  number of re-derived closed roots must stay, drop or rise as the defect
  implies (the frontier fixture must reproduce its exact baseline, e.g. 60/124);
* partial or no re-inspection must never PASS (the blocking gap of the first
  round): `--reinspect none` / `sample` with `--require-closure` is INCOMPLETE,
  even with a dropped edge or an injected false hit, with 0 roots
  independently verified;
* the gate of `assert_oracle_pass.py` (verdict == PASS and
  roots_independently_verified == roots_total, unmutated, full re-inspection,
  reference levers off): it must pass exactly on the unmutated drained PASS
  rows and fail on every other row, including benign mutations that keep a
  PASS verdict and the frontier fixture's plain PASS (consistent, but only
  60/124 roots closed).

G2' residual-anchor rows (`g2-*`) need a drained walk run with
`--g2-residual-anchors union` (`--g2-run`); they must FAIL with their exact
classes: a residual shrunk by one D layer, an anchor merged at or after the
snapshot, an anchor that is neither Native nor a validated G2' record, a
dropped anchor edge and an anchor cycle. The unmutated G2' walk must PASS the
gate like the drained run.

Rows whose mutation needs structure a fixture lacks (partial records, a
second initial record, Route natives that route Apply domains) are decided
from the unmutated baseline report: when the baseline proves the structure
absent, the row is reported as not applicable (ok, not run); when the
structure is present the mutation must apply. Structural rows may run with
`--reinspect none` (e.g. route-partial: the phase rule alone must fire).

The audit sees no edges, so dropped edges and successor-level false hits are
not observable by it (reported as such, never as a pass); its rows assert the
exact set of NEW violation kinds against the unmutated baseline (messages with
numbers normalized). A consistently hidden frontier is a documented blind spot
of the audit (it does not re-inspect): its row asserts that nothing new shows.
The matrix is written as JSON; the exit status is 0 only if every row holds.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("audit_owner_domain_walk", HERE / "audit_owner_domain_walk.py")
AUDIT = importlib.util.module_from_spec(spec)
spec.loader.exec_module(AUDIT)
gate_spec = importlib.util.spec_from_file_location("assert_oracle_pass", HERE / "assert_oracle_pass.py")
GATE = importlib.util.module_from_spec(gate_spec)
gate_spec.loader.exec_module(GATE)

EXIT = {"PASS": 0, "FAIL": 1, "INCOMPLETE": 9}
SKIP_PARTIAL_ROWS = [False]

# kind -> (run, verdict, exact classes with --require-closure, closure effect, exact per-class counts
#          [, required baseline structure, extra verifier arguments])
# closure effect: "same" (roots_oracle_closed unchanged), "fewer", "more", or None (not asserted).
PARTIAL_ANCHOR = {"partial_anchor_kind", "partial_anchor_order"}
RUST_MUTATIONS = {
    "dropped-edge": ("drained", "FAIL", {"successor_uncovered"}, "same", {}),
    "injected-false-hit": ("drained", "FAIL", {"successor_uncovered"}, "same", {}),
    "retargeted-alias": ("drained", "FAIL", {"alias_containment"}, "same", {"alias_containment": 1}),
    "retargeted-anchor": ("drained", "FAIL", {"partial_anchor", "partial_union_cover"}, "same",
                          {"partial_anchor": 1, "partial_union_cover": 1}, ("partials", "multi_initial")),
    # Partial-anchor well-foundedness (the anchor edge moves with the link; only the anchor rules fire).
    "self-anchored-partial": ("drained", "FAIL", PARTIAL_ANCHOR, "same",
                              {"partial_anchor_kind": 1, "partial_anchor_order": 1}, ("partials",)),
    "partial-as-anchor": ("drained", "FAIL", PARTIAL_ANCHOR, "same",
                          {"partial_anchor_kind": 1, "partial_anchor_order": 1}, ("two_partials",)),
    "partial-anchor-cycle": ("drained", "FAIL", PARTIAL_ANCHOR, "same",
                             {"partial_anchor_kind": 2, "partial_anchor_order": 2}, ("two_partials",)),
    "non-initial-anchor": ("drained", "FAIL", {"partial_anchor_order"}, "same", {"partial_anchor_order": 1},
                            ("partials",)),
    "shrunk-residual": ("drained", "FAIL", {"partial_residual", "partial_union_cover"}, "same",
                        {"partial_residual": 1, "partial_union_cover": 1}, ("partials",)),
    # Structural: without re-inspection only the phase rule can see a Route partial.
    "route-partial": ("drained", "FAIL", {"partial_phase"}, "same", {"partial_phase": 1}, ("partials",),
                      ["--reinspect", "none"]),
    # Route natives: a routed Apply domain's only covering edge, a Route native's event count.
    "dropped-routed-edge": ("drained", "FAIL", {"successor_uncovered"}, "same", {}, ("routed",)),
    "routed-false-hit": ("drained", "FAIL", {"successor_uncovered"}, "same", {}, ("routed",)),
    "miscounted-route-events": ("drained", "FAIL", {"event_parity"}, "same", {"event_parity": 1}, ("routed",)),
    "seal-with-error": ("drained", "FAIL",
                        {"seal_parity", "error_parity", "native_counter", "false_closure", "closure_required"},
                        "fewer", {"seal_parity": 1, "error_parity": 1, "native_counter": 1}),
    "hidden-error": ("drained", "FAIL", {"error_parity"}, "same", {"error_parity": 1}),
    "miscounted-events": ("drained", "FAIL", {"event_parity"}, "same", {"event_parity": 1}),
    "miscounted-successors": ("drained", "FAIL", {"successor_parity"}, "same", {"successor_parity": 1}),
    "remapped-query": ("drained", "FAIL", {"root_mapping"}, None, {"root_mapping": 1}, ("multi_initial",)),
    "foreign-request": ("drained", "FAIL", {"binding"}, "same", {"binding": 1}),
    "foreign-owners": ("drained", "FAIL", {"binding"}, "same", {"binding": 1}),
    "mismatched-result": ("drained", "FAIL", {"result_binding"}, "same", {"result_binding": 1}),
    "alias-chain-detour": ("drained", "PASS", set(), "same", {}),
    "dropped-frontier-record": ("frontier", "FAIL", {"frontier_counter", "frontier_parity", "closure_required"},
                                "more", {"frontier_counter": 1, "frontier_parity": 1}),
    "hidden-frontier": ("frontier", "FAIL", {"frontier_parity", "closure_required"}, "more",
                        {"frontier_parity": 1}),
    "seal-with-frontier": ("frontier", "FAIL", {"seal_parity", "closure_required"}, "same", {"seal_parity": 1}),
    # G2' residual anchors (drained G2' walk; exact classes calibrated on C-4L FG Ordered W6 with
    # --g2-residual-anchors union, whose G2' residuals are single D levels: the shrunk residual becomes an
    # empty band, so the F10 full-cover rule also fires as event_parity).
    "g2-shrunk-residual": ("g2", "FAIL", {"g2_union_cover", "event_parity"}, "same",
                           {"g2_union_cover": 1, "event_parity": 1}, ("g2_residual",)),
    "g2-late-anchor": ("g2", "FAIL", {"g2_anchor_order"}, "same", {"g2_anchor_order": 1}, ("g2",)),
    "g2-inadmissible-anchor": ("g2", "FAIL", {"g2_anchor_kind"}, "same", {"g2_anchor_kind": 1}, ("g2",)),
    "g2-dropped-anchor-edge": ("g2", "FAIL", {"missing_edge", "seal_parity", "false_closure", "closure_required"},
                               "fewer", {"missing_edge": 1, "seal_parity": 1}, ("g2",)),
    "g2-anchor-cycle": ("g2", "FAIL", {"g2_anchor_order"}, "same", {}, ("two_g2_residual",)),
}


def normalized(violation):
    """A violation's kind: query ids and numbers replaced."""
    violation = re.sub(r"query [^:]+:", "query Q:", violation)
    return re.sub(r"-?\d+", "N", violation)


def load_result(run):
    return json.loads((Path(run) / "result.json").read_text())


def native_rows(document):
    return [row for row in document["domains"] if row.get("record_kind") == "native_inspection"]


def mutate_retargeted_alias(document):
    rows = {row["id"]: row for row in document["domains"]}
    for row in document["domains"]:
        if row.get("record_kind") != "delegated_not_inspected":
            continue
        inner = AUDIT.box_of(row)
        for candidate in sorted(rows):
            other = rows[candidate]
            if (candidate > row["id"] and candidate != row["representative_id"]
                    and other.get("record_kind") == "native_inspection"
                    and other["owner"] == row["owner"] and other["phase"] == row["phase"]
                    and not AUDIT.box_contains(AUDIT.box_of(other), inner)):
                detail = {"alias": row["id"], "from": row["representative_id"], "to": candidate}
                row["representative_id"] = row["final_representative_id"] = candidate
                return detail
    return None


def mutate_dropped_frontier_record(document):
    for row in native_rows(document):
        if row.get("frontiers"):
            detail = {"record": row["id"], "dropped": len(row["frontiers"])}
            row["frontiers"] = []
            return detail
    return None


def mutate_hidden_frontier(document):
    """Drop a record's frontiers AND the top-level counter: consistent, invisible without re-inspection."""
    detail = mutate_dropped_frontier_record(document)
    if detail is not None:
        document["frontiers"] -= detail["dropped"]
    return detail


def mutate_seal_with_frontier(document):
    for row in native_rows(document):
        if row.get("descendant_closed") is True and row.get("frontiers") == []:
            row["frontiers"] = [{"kind": "local_dispatch_frontier", "injected": True}]
            document["frontiers"] += 1
            return {"record": row["id"]}
    return None


def mutate_seal_with_error(document):
    for row in native_rows(document):
        if row.get("descendant_closed") is True and row.get("error") is None:
            row["error"] = "injected native failure"
            document["failed_nodes"] += 1
            return {"record": row["id"]}
    return None


def partial_rows(document):
    return [row for row in document["domains"] if row.get("record_kind") == "partial_initial_overlap_inspection"]


def mutate_self_anchored_partial(document):
    rows = partial_rows(document)
    if not rows:
        return None
    row = rows[0]
    detail = {"partial": row["id"], "from": row["initial_overlap"]["anchor_id"], "to": row["id"]}
    row["initial_overlap"]["anchor_id"] = row["id"]
    return detail


def mutate_partial_as_anchor(document):
    rows = partial_rows(document)
    if len(rows) < 2:
        return None
    anchor, row = rows[0], rows[1]
    detail = {"partial": row["id"], "from": row["initial_overlap"]["anchor_id"], "to": anchor["id"]}
    row["initial_overlap"]["anchor_id"] = anchor["id"]
    return detail


def mutate_partial_anchor_cycle(document):
    rows = partial_rows(document)
    if len(rows) < 2:
        return None
    first, second = rows[0], rows[1]
    first["initial_overlap"]["anchor_id"], second["initial_overlap"]["anchor_id"] = second["id"], first["id"]
    return {"cycle": [first["id"], second["id"]]}


def mutate_route_partial(document):
    rows = partial_rows(document)
    if not rows:
        return None
    rows[0]["phase"] = "Route"
    return {"partial": rows[0]["id"]}


def g2_rows(document):
    return [row for row in document["domains"] if row.get("record_kind") == "g2_residual_anchor_inspection"]


def mutate_g2_shrunk_residual(document):
    """Drop the highest D level of a G2' residual band (a single-level band becomes a full-cover claim)."""
    rows = [row for row in g2_rows(document) if row["g2_residual_anchors"].get("residual_power_bounds")]
    rows.sort(key=lambda row: (row["g2_residual_anchors"]["residual_power_bounds"]["min_power_difference"]
                               == row["g2_residual_anchors"]["residual_power_bounds"]["max_power_difference"],
                               row["id"]))
    for row in rows:
        block = row["g2_residual_anchors"]
        residual = block["residual_power_bounds"]
        if residual["min_power_difference"] < residual["max_power_difference"]:
            residual["max_power_difference"] -= 1
            block["residual_d_band"] = [residual["min_power_difference"], residual["max_power_difference"]]
        else:
            block["residual_power_bounds"] = block["residual_d_band"] = None
            block["residual_pieces"] = 0
        return {"record": row["id"], "residual": block["residual_power_bounds"]}
    return None


def _positions(document):
    return {row["id"]: index for index, row in enumerate(document["domains"])}


def _replace_anchor(row, other, positions):
    block = row["g2_residual_anchors"]
    detail = {"record": row["id"], "from": block["anchors"][0]["id"], "to": other["id"]}
    block["anchors"][0] = {"id": other["id"], "stamp": positions[other["id"]], "kind": "native", "scope": "domain"}
    block["anchors"].sort(key=lambda a: a["stamp"])
    return detail


def mutate_g2_late_anchor(document):
    """Replace an anchor by a same-owner Apply Native record merged at or after the snapshot."""
    positions = _positions(document)
    for row in g2_rows(document):
        block = row["g2_residual_anchors"]
        for other in document["domains"]:
            if (other.get("record_kind") == "native_inspection" and other["id"] != row["id"]
                    and other["owner"] == row["owner"] and other["phase"] == "Apply"
                    and positions[other["id"]] >= block["snapshot_stamp"]):
                return _replace_anchor(row, other, positions)
    return None


def mutate_g2_inadmissible_anchor(document):
    positions = _positions(document)

    def inadmissible(other, row):
        kind = other.get("record_kind")
        if kind == "delegated_not_inspected":
            return 0 if other["owner"] == row["owner"] else 1
        if kind == "g2_residual_anchor_inspection" and other["g2_residual_anchors"].get("residual_power_bounds") is None:
            return 2
        if other.get("phase") == "Route":
            return 3
        return None

    for row in g2_rows(document):
        block = row["g2_residual_anchors"]
        ranked = sorted((rank, other["id"], other) for other in document["domains"]
                        if other["id"] != row["id"] and positions[other["id"]] < block["snapshot_stamp"]
                        for rank in [inadmissible(other, row)] if rank is not None)
        if ranked:
            return _replace_anchor(row, ranked[0][2], positions)
    return None


def mutate_g2_anchor_cycle(document):
    positions = _positions(document)
    rows = [row for row in g2_rows(document) if row["g2_residual_anchors"].get("residual_power_bounds")]
    if len(rows) < 2:
        return None
    first, second = rows[0], rows[1]
    for x, y in ((first, second), (second, first)):
        x["g2_residual_anchors"]["anchors"].append({"id": y["id"], "stamp": positions[y["id"]],
                                                    "kind": "g2_residual", "scope": "domain"})
        x["g2_residual_anchors"]["anchors"].sort(key=lambda a: a["stamp"])
    return {"cycle": [first["id"], second["id"]]}


def mutate_remapped_query(document):
    """Resolve a later query to an earlier same-owner initial record that does not contain it.

    This is a query-to-root remapping (the Rust `remapped-query`), not a
    successor-level false hit: result.json carries no edges.
    """
    queries = json.loads(Path(document["_queries_path"]).read_text())["queries"]
    rows = {row["id"]: row for row in document["domains"] if row["id"] < len(queries)}
    inputs = document["inputs"]
    for index, query in enumerate(queries):
        box = AUDIT.box_of(query, "max_numerator_rank", "Apply")
        for earlier in inputs[:index]:
            record = rows.get(earlier["domain"])
            if (record is not None and earlier["domain"] != inputs[index]["domain"]
                    and record["owner"] == query["owner"]
                    and not AUDIT.box_contains(AUDIT.box_of(record), box)):
                detail = {"query": query["id"], "from": inputs[index]["domain"], "to": earlier["domain"]}
                inputs[index]["domain"] = earlier["domain"]
                return detail
    return None


# kind -> (run, mutate, exact set of NEW normalized violation kinds vs the unmutated baseline).
# Calibrated on C-4L FG Ordered + the FG frontier fixture (TMP/w0/oracle/RESULTS.md): another
# fixture may legitimately differ (e.g. which query fields a remap changes); such a row must be
# re-calibrated by review, never loosened to "contains".
PARTIAL_ANCHOR_KINDS = {"record N: partial anchor N has no initial row",
                        "record N: partial anchor must be an earlier initial record"}
PYTHON_MUTATIONS = {
    "retargeted-alias": ("drained", mutate_retargeted_alias,
                         {"record N: alias domain not contained in representative N"}),
    "dropped-frontier-record": ("frontier", mutate_dropped_frontier_record,
                                {"frontier parity: top-level frontiers != input + record + carried frontiers"}),
    # Blind spot: consistent record + counter, no re-inspection in the audit.
    "hidden-frontier": ("frontier", mutate_hidden_frontier, set()),
    "seal-with-frontier": ("drained", mutate_seal_with_frontier,
                           {"frontiers must be N", "record N: descendant_closed despite a frontier or error",
                            "record N: nonzero frontiers"}),
    "seal-with-error": ("drained", mutate_seal_with_error,
                        {"error parity: walk error vs record errors", "failed_nodes must be N",
                         "record N: descendant_closed despite a frontier or error", "record N: native error recorded"}),
    # Partial-anchor well-foundedness (calibrated on C-4L FG Ordered, 160 partial records): the audit
    # requires an earlier initial NATIVE anchor and an Apply partial.
    "self-anchored-partial": ("drained", mutate_self_anchored_partial, PARTIAL_ANCHOR_KINDS),
    "partial-as-anchor": ("drained", mutate_partial_as_anchor, PARTIAL_ANCHOR_KINDS),
    "partial-anchor-cycle": ("drained", mutate_partial_anchor_cycle, PARTIAL_ANCHOR_KINDS),
    "route-partial": ("drained", mutate_route_partial,
                      {"record N: partial inspection outside Apply",
                       "record N: partial anchor is not a same-owner Apply native inspection",
                       "record N: partial D>=cut slice not contained in anchor N",
                       "record N: Route without conservative overcover", "record N: Route missing_routes must be N",
                       "record N: Route event accounting mismatch", "routed_domains != native Route inspections",
                       "successors != Apply statistics"}),
    "remapped-query": ("drained", mutate_remapped_query,
                       {"closure initial_total != distinct initial records",
                        "closure required: initial_closed != initial_total != distinct initial records",
                        "initial entry obligations not discharged", "inputs do not map every query to an initial record",
                        "query Q: no initial record", "query Q: power_bounds changed", "query Q: upper changed"}),
}
# Normalized kinds: `normalized` rewrites every number, so "G2'" reads "GN'".
PYTHON_MUTATIONS.update({
    "g2-shrunk-residual": ("g2", mutate_g2_shrunk_residual,
                           {"record N: GN' domain not covered by its residual and anchors (False)"}),
    "g2-late-anchor": ("g2", mutate_g2_late_anchor,
                       {"record N: GN' anchor N not merged before the snapshot"}),
    "g2-inadmissible-anchor": ("g2", mutate_g2_inadmissible_anchor,
                               {"record N: GN' anchor N ('native') is not a same-owner Apply Native, "
                                "initial-D-band or GN' residual record"}),
    "g2-anchor-cycle": ("g2", mutate_g2_anchor_cycle,
                        {"record N: GN' anchor N not merged before the snapshot"}),
})
PYTHON_NOT_OBSERVABLE = {
    "dropped-edge": "result.json carries no dependency edges",
    "injected-false-hit": "result.json carries no dependency edges (successor-level false hit)",
    "hidden-error": "the audit does not re-inspect natives",
    "miscounted-events": "consistent per-record and global counts; the audit does not re-inspect natives",
    "g2-dropped-anchor-edge": "result.json carries no dependency edges",
}


def audit_copy(run, document, require_closure=True):
    run = Path(run)
    with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as temporary:
        copy = Path(temporary)
        (copy / "result.json").write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
        report = AUDIT.audit_walk(copy, command=run / "command.json", require_closure=require_closure)
    return report


def python_matrix(runs):
    rows = []
    baselines = {}
    for name, run in runs.items():
        if run is None:
            continue
        report = AUDIT.audit_walk(run, command=Path(run) / "command.json", require_closure=True)
        baselines[name] = {normalized(v) for v in report["violations"]}
        expected = "PASS" if name in ("drained", "g2") else "FAIL"
        rows.append({"oracle": "python-audit", "run": name, "mutation": None, "expected": expected,
                     "verdict": report["audit"], "ok": report["audit"] == expected,
                     "violations": report["violations"][:5]})
    for kind, reason in PYTHON_NOT_OBSERVABLE.items():
        rows.append({"oracle": "python-audit", "run": None, "mutation": kind, "expected": "n/a",
                     "verdict": "not-observable", "reason": reason, "ok": True, "observable": False})
    for kind, (target, mutate, expected_new) in PYTHON_MUTATIONS.items():
        run = runs.get(target)
        row = {"oracle": "python-audit", "run": target, "mutation": kind,
               "expected_new_violation_kinds": sorted(expected_new)}
        if run is None:
            rows.append(dict(row, ok=False, verdict="not-run", reason=f"no {target} run"))
            continue
        document = load_result(run)
        document["_queries_path"] = AUDIT.locate(run, command=Path(run) / "command.json")["queries"]
        detail = mutate(document)
        del document["_queries_path"]
        if detail is None:
            rows.append(dict(row, ok=False, verdict="not-applicable"))
            continue
        report = audit_copy(run, document)
        new = {normalized(v) for v in report["violations"]} - baselines[target]
        expected_verdict = "FAIL" if (expected_new or target == "frontier") else "PASS"
        ok = report["audit"] == expected_verdict and new == expected_new
        rows.append(dict(row, expected=expected_verdict, detail=detail, verdict=report["audit"],
                         new_violation_kinds=sorted(new), ok=ok,
                         violations=report["violations"][:8]))
    return rows


def rust_run(binary, run, output, threads, extra):
    command = [str(binary), "walk-verify-closure", "--command", str(Path(run) / "command.json"),
               "--output", str(output), "--threads", str(threads), "--force"] + extra
    env = dict(os.environ)
    for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT", "OPENBLAS_NUM_THREADS",
                 "MKL_NUM_THREADS", "BLIS_NUM_THREADS", "SYMBOLICA_HIDE_BANNER"):
        env[name] = "1"
    started = time.monotonic()
    result = subprocess.run(command, env=env, capture_output=True, text=True)
    report = json.loads(Path(output).read_text()) if Path(output).is_file() else {}
    return result.returncode, report, time.monotonic() - started, result.stderr[-2000:]


def closed_roots(report):
    counts = report.get("counts") or {}
    return counts.get("roots_oracle_closed"), counts.get("roots")


def rust_matrix(binary, runs, directory, threads, frontier_expect, jobs=1):
    """Baselines first (their closed-root counts anchor the closure effects),
    then every other row, `jobs` verifier processes at a time."""
    rows = []
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    baseline_closed = {}

    def row_for(label, run_name, mutation, extra, expected_verdict, expected_classes, counts=None,
                closure=None, extra_checks=None):
        code, report, seconds, stderr = rust_run(binary, runs[run_name], directory / f"{label}.json", threads,
                                                 extra)
        classes = report.get("violations_by_class") or {}
        checks = {"verdict": report.get("verdict") == expected_verdict,
                  "exit_status": code == EXIT[expected_verdict],
                  "exact_classes": set(classes) == set(expected_classes)}
        for klass, count in (counts or {}).items():
            checks[f"count:{klass}"] = classes.get(klass) == count
        closed, roots = closed_roots(report)
        if closure is not None and run_name in baseline_closed:
            before = baseline_closed[run_name]
            checks["closure_effect"] = {"same": closed == before, "fewer": closed is not None and closed < before,
                                        "more": closed is not None and closed > before}[closure]
        for name, value in (extra_checks or (lambda r: {}))(report).items():
            checks[name] = value
        if mutation is not None:
            checks["applied"] = (report.get("mutation") or {}).get("applied") is True
        gate_reasons = GATE.gate(report)
        # A mutated report is a negative control: it never passes the gate, even
        # when the mutation is benign and the verdict stays PASS.
        gate_expected = expected_verdict == "PASS" and run_name in ("drained", "g2") and mutation is None
        checks["gate"] = (not gate_reasons) == gate_expected
        return {"oracle": "walk-verify-closure", "run": run_name, "mutation": mutation, "label": label,
                "args": extra, "expected": expected_verdict, "expected_classes": sorted(expected_classes),
                "verdict": report.get("verdict"), "exit": code, "seconds": seconds,
                "violations_by_class": classes, "roots_oracle_closed": closed, "roots": roots,
                "closure_effect_expected": closure, "mutation_detail": report.get("mutation"),
                "gate_expected_pass": gate_expected, "gate_reasons": gate_reasons,
                "checks": checks, "ok": all(checks.values()), "stderr_tail": stderr[-300:]}

    def unverified(report):
        classes = (report.get("certification") or {}).get("classes") or {}
        verified = sum(entry.get("independently_verified", 0) for entry in classes.values())
        return {"no_root_independently_verified": verified == 0}

    names = [name for name in ("drained", "frontier", "g2") if runs.get(name) is not None]
    with ThreadPoolExecutor(max(1, jobs)) as pool:
        plains = list(pool.map(lambda name: row_for(f"baseline-{name}", name, None, [], "PASS", set()), names))
    structure = {}
    for name, plain in zip(names, plains):
        report = json.loads((directory / f"baseline-{name}.json").read_text()) \
            if (directory / f"baseline-{name}.json").is_file() else {}
        counts = report.get("counts") or {}
        records = counts.get("records") or {}
        tally = (report.get("reinspection") or {}).get("tally") or {}
        structure[name] = {"partials": records.get("partials", 0) >= 1,
                           "two_partials": records.get("partials", 0) >= 2,
                           "g2": records.get("g2_records", 0) >= 1,
                           "g2_residual": (counts.get("g2_residual_records") or 0) >= 1,
                           "two_g2_residual": (counts.get("g2_residual_records") or 0) >= 2,
                           "multi_initial": (counts.get("initial_records") or 0) >= 2,
                           "routed": (tally.get("admitted_routed_domains") or 0) >= 1}
    later = []
    for name, plain in zip(names, plains):
        rows.append(plain)
        closed, roots = plain["roots_oracle_closed"], plain["roots"]
        if name == "frontier":
            required = {"closure_required"}
            checks = (lambda r, e=frontier_expect: {"frontier_closed_roots": closed_roots(r) == e}) \
                if frontier_expect else None
            later.append(dict(label=f"baseline-{name}-require-closure", run_name=name, mutation=None,
                              extra=["--require-closure"], expected_verdict="FAIL", expected_classes=required,
                              counts={"closure_required": roots - closed}
                              if roots is not None and closed is not None else None, extra_checks=checks))
        else:
            later.append(dict(label=f"baseline-{name}-require-closure", run_name=name, mutation=None,
                              extra=["--require-closure"], expected_verdict="PASS", expected_classes=set(),
                              extra_checks=lambda r: {"all_roots_closed": closed_roots(r)[0] == closed_roots(r)[1]}))
        # --require-closure does not change the re-derived closure: the plain
        # baseline's count anchors every mutation's closure effect.
        baseline_closed[name] = closed
    if runs.get("drained") is not None and not SKIP_PARTIAL_ROWS[0]:
        # Partial or no re-inspection is never a PASS, even over a real defect F10 alone sees.
        for label, extra in (("partial-none", ["--reinspect", "none"]),
                             ("partial-sample", ["--reinspect", "sample:10:1"]),
                             ("partial-none-dropped-edge", ["--reinspect", "none", "--mutate", "dropped-edge"])):
            mutation = "dropped-edge" if "--mutate" in extra else None
            later.append(dict(label=label, run_name="drained", mutation=mutation, extra=extra + ["--require-closure"],
                              expected_verdict="INCOMPLETE", expected_classes=set(), extra_checks=unverified))
    for kind, spec in RUST_MUTATIONS.items():
        target, verdict, classes, closure, counts = spec[:5]
        requires = spec[5] if len(spec) > 5 else ()
        extra_args = spec[6] if len(spec) > 6 else []
        if runs.get(target) is None:
            rows.append({"oracle": "walk-verify-closure", "run": target, "mutation": kind, "ok": False,
                         "verdict": "not-run", "reason": f"no {target} run"})
            continue
        lacking = [need for need in requires if not structure.get(target, {}).get(need, False)]
        if lacking:
            # The unmutated baseline proves the structure absent: not applicable, not run.
            rows.append({"oracle": "walk-verify-closure", "run": target, "mutation": kind, "ok": True,
                         "verdict": "not-applicable", "expected": verdict, "requires": list(requires),
                         "reason": f"baseline {target} lacks {lacking}"})
            continue

        def chain(report, kind=kind):
            tally = (report.get("reinspection") or {}).get("tally") or {}
            return {"alias_chain_exercised": tally.get("covered_along_alias_chain", 0) >= 1} \
                if kind == "alias-chain-detour" else {}
        later.append(dict(label=f"mutation-{kind}", run_name=target, mutation=kind,
                          extra=["--require-closure", "--mutate", kind] + extra_args, expected_verdict=verdict,
                          expected_classes=classes, counts=counts, closure=closure, extra_checks=chain))
    with ThreadPoolExecutor(max(1, jobs)) as pool:
        rows += list(pool.map(lambda spec: row_for(**spec), later))
    return rows


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--run", type=Path, required=True, help="drained walk run directory (command.json, result.json)")
    parser.add_argument("--frontier-run", type=Path, help="walk run directory that retains frontiers")
    parser.add_argument("--g2-run", type=Path, help="drained walk run with --g2-residual-anchors union")
    parser.add_argument("--frontier-expect-closed", help="exact closed/total roots of the frontier run, e.g. 60/124")
    parser.add_argument("--rustred", type=Path, help="rustred executable with walk-verify-closure")
    parser.add_argument("--threads", type=int, default=1, help="threads per verifier process")
    parser.add_argument("--jobs", type=int, default=1, help="verifier processes run at a time")
    parser.add_argument("--output", type=Path, required=True, help="matrix JSON; Rust reports go next to it")
    parser.add_argument("--skip-python", action="store_true")
    parser.add_argument("--only", nargs="*", help="run only these Rust mutation kinds (baselines always run)")
    parser.add_argument("--skip-partial-rows", action="store_true",
                        help="skip the partial/no re-inspection INCOMPLETE rows (when splitting a matrix in two runs)")
    args = parser.parse_args(argv)
    runs = {"drained": args.run, "frontier": args.frontier_run, "g2": args.g2_run}
    frontier_expect = None
    if args.frontier_expect_closed:
        closed, total = args.frontier_expect_closed.split("/")
        frontier_expect = (int(closed), int(total))
    if args.only:
        for kind in list(RUST_MUTATIONS):
            if kind not in args.only:
                del RUST_MUTATIONS[kind]
    if args.skip_partial_rows:
        SKIP_PARTIAL_ROWS[0] = True
    rows = [] if args.skip_python else python_matrix(runs)
    if args.rustred is not None:
        rows += rust_matrix(args.rustred, runs, args.output.with_suffix(""), args.threads, frontier_expect,
                            args.jobs)
    matrix = {"schema": "rustred.oracle-mutation-matrix.v2", "runs": {k: str(v) for k, v in runs.items()},
              "rustred": None if args.rustred is None else str(args.rustred),
              "frontier_expect_closed": args.frontier_expect_closed,
              "all_ok": all(row["ok"] for row in rows), "rows": rows}
    args.output.write_text(json.dumps(matrix, indent=2) + "\n")
    for row in rows:
        failed = [name for name, value in (row.get("checks") or {}).items() if not value]
        print(f"{'ok ' if row['ok'] else 'BAD'} {row['oracle']:20s} {str(row['run']):9s} "
              f"{str(row.get('label') or row['mutation']):28s} expected {row.get('expected')} got {row.get('verdict')}"
              + (f"  failed checks: {failed}" if failed else ""))
    return 0 if matrix["all_ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
