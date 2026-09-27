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
  roots_independently_verified == roots_total): it must pass exactly on the
  drained PASS rows and fail on every other row, including the frontier
  fixture's plain PASS (consistent, but only 60/124 roots closed).

The audit sees no edges, so dropped edges and successor-level false hits are
not observable by it (reported as such, never as a pass); its rows assert the
exact set of NEW violation kinds against the unmutated baseline (messages with
numbers normalized). A consistently hidden frontier is a documented blind spot
of the audit (it does not re-inspect): its row asserts that nothing new shows.
The matrix is written as JSON; the exit status is 0 only if every row holds.
"""
from __future__ import annotations

import argparse
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

# kind -> (run, verdict, exact classes with --require-closure, closure effect, exact per-class counts)
# closure effect: "same" (roots_oracle_closed unchanged), "fewer", "more", or None (not asserted).
RUST_MUTATIONS = {
    "dropped-edge": ("drained", "FAIL", {"successor_uncovered"}, "same", {}),
    "injected-false-hit": ("drained", "FAIL", {"successor_uncovered"}, "same", {}),
    "retargeted-alias": ("drained", "FAIL", {"alias_containment"}, "same", {"alias_containment": 1}),
    "retargeted-anchor": ("drained", "FAIL", {"partial_anchor"}, "same", {"partial_anchor": 1}),
    "seal-with-error": ("drained", "FAIL",
                        {"seal_parity", "error_parity", "native_counter", "false_closure", "closure_required"},
                        "fewer", {"seal_parity": 1, "error_parity": 1, "native_counter": 1}),
    "hidden-error": ("drained", "FAIL", {"error_parity"}, "same", {"error_parity": 1}),
    "miscounted-events": ("drained", "FAIL", {"event_parity"}, "same", {"event_parity": 1}),
    "miscounted-successors": ("drained", "FAIL", {"successor_parity"}, "same", {"successor_parity": 1}),
    "remapped-query": ("drained", "FAIL", {"root_mapping"}, None, {"root_mapping": 1}),
    "foreign-request": ("drained", "FAIL", {"binding"}, "same", {"binding": 1}),
    "foreign-owners": ("drained", "FAIL", {"binding"}, "same", {"binding": 1}),
    "mismatched-result": ("drained", "FAIL", {"result_binding"}, "same", {"result_binding": 1}),
    "alias-chain-detour": ("drained", "PASS", set(), "same", {}),
    "dropped-frontier-record": ("frontier", "FAIL", {"frontier_counter", "frontier_parity", "closure_required"},
                                "more", {"frontier_counter": 1, "frontier_parity": 1}),
    "hidden-frontier": ("frontier", "FAIL", {"frontier_parity", "closure_required"}, "more",
                        {"frontier_parity": 1}),
    "seal-with-frontier": ("frontier", "FAIL", {"seal_parity", "closure_required"}, "same", {"seal_parity": 1}),
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
    "remapped-query": ("drained", mutate_remapped_query,
                       {"closure initial_total != distinct initial records",
                        "closure required: initial_closed != initial_total != distinct initial records",
                        "initial entry obligations not discharged", "inputs do not map every query to an initial record",
                        "query Q: no initial record", "query Q: power_bounds changed", "query Q: upper changed"}),
}
PYTHON_NOT_OBSERVABLE = {
    "dropped-edge": "result.json carries no dependency edges",
    "injected-false-hit": "result.json carries no dependency edges (successor-level false hit)",
    "hidden-error": "the audit does not re-inspect natives",
    "miscounted-events": "consistent per-record and global counts; the audit does not re-inspect natives",
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
        expected = "PASS" if name == "drained" else "FAIL"
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


def rust_matrix(binary, runs, directory, threads, frontier_expect):
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
        gate_expected = expected_verdict == "PASS" and run_name == "drained"
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

    for name in ("drained", "frontier"):
        if runs.get(name) is None:
            continue
        plain = row_for(f"baseline-{name}", name, None, [], "PASS", set())
        rows.append(plain)
        closed, roots = plain["roots_oracle_closed"], plain["roots"]
        if name == "frontier":
            required = {"closure_required"}
            checks = (lambda r, e=frontier_expect: {"frontier_closed_roots": closed_roots(r) == e}) \
                if frontier_expect else None
            row = row_for(f"baseline-{name}-require-closure", name, None, ["--require-closure"], "FAIL", required,
                          counts={"closure_required": roots - closed} if roots is not None and closed is not None
                          else None, extra_checks=checks)
        else:
            row = row_for(f"baseline-{name}-require-closure", name, None, ["--require-closure"], "PASS", set(),
                          extra_checks=lambda r: {"all_roots_closed": closed_roots(r)[0] == closed_roots(r)[1]})
        rows.append(row)
        baseline_closed[name] = row["roots_oracle_closed"]
    if runs.get("drained") is not None:
        # Partial or no re-inspection is never a PASS, even over a real defect F10 alone sees.
        for label, extra in (("partial-none", ["--reinspect", "none"]),
                             ("partial-sample", ["--reinspect", "sample:10:1"]),
                             ("partial-none-dropped-edge", ["--reinspect", "none", "--mutate", "dropped-edge"])):
            mutation = "dropped-edge" if "--mutate" in extra else None
            rows.append(row_for(label, "drained", mutation, extra + ["--require-closure"], "INCOMPLETE", set(),
                                extra_checks=unverified))
    for kind, (target, verdict, classes, closure, counts) in RUST_MUTATIONS.items():
        if runs.get(target) is None:
            rows.append({"oracle": "walk-verify-closure", "run": target, "mutation": kind, "ok": False,
                         "verdict": "not-run", "reason": f"no {target} run"})
            continue

        def chain(report, kind=kind):
            tally = (report.get("reinspection") or {}).get("tally") or {}
            return {"alias_chain_exercised": tally.get("covered_along_alias_chain", 0) >= 1} \
                if kind == "alias-chain-detour" else {}
        rows.append(row_for(f"mutation-{kind}", target, kind, ["--require-closure", "--mutate", kind], verdict,
                            classes, counts, closure, chain))
    return rows


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--run", type=Path, required=True, help="drained walk run directory (command.json, result.json)")
    parser.add_argument("--frontier-run", type=Path, help="walk run directory that retains frontiers")
    parser.add_argument("--frontier-expect-closed", help="exact closed/total roots of the frontier run, e.g. 60/124")
    parser.add_argument("--rustred", type=Path, help="rustred executable with walk-verify-closure")
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--output", type=Path, required=True, help="matrix JSON; Rust reports go next to it")
    parser.add_argument("--skip-python", action="store_true")
    parser.add_argument("--only", nargs="*", help="run only these Rust mutation kinds (baselines always run)")
    args = parser.parse_args(argv)
    runs = {"drained": args.run, "frontier": args.frontier_run}
    frontier_expect = None
    if args.frontier_expect_closed:
        closed, total = args.frontier_expect_closed.split("/")
        frontier_expect = (int(closed), int(total))
    if args.only:
        for kind in list(RUST_MUTATIONS):
            if kind not in args.only:
                del RUST_MUTATIONS[kind]
    rows = [] if args.skip_python else python_matrix(runs)
    if args.rustred is not None:
        rows += rust_matrix(args.rustred, runs, args.output.with_suffix(""), args.threads, frontier_expect)
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
