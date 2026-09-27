#!/usr/bin/env python3
"""Mutation matrix for the W0.2 closure oracles: every injected defect must FAIL.

Two oracles are exercised on real walk outputs:

* the streaming result audit (`audit_owner_domain_walk.py --require-closure`),
  on mutated copies of `result.json` (the saved checkpoint is only read);
* `rustred walk-verify-closure`, whose `--mutate KIND` injects the defect in
  memory after loading the checkpoint.

`--run` must be a drained walk (audit PASS); `--frontier-run` a walk that
retains frontiers (for the frontier-record mutations). Unmutated baselines
are run first: the drained run must PASS both oracles (with closure
required), the frontier run must PASS the verifier's consistency check and
FAIL once closure is required. A mutation that does not apply to a run is
reported as such, never as a pass. The matrix is written as JSON; the exit
status is 0 only if every baseline and mutation behaved as required.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("audit_owner_domain_walk", HERE / "audit_owner_domain_walk.py")
AUDIT = importlib.util.module_from_spec(spec)
spec.loader.exec_module(AUDIT)

# Rust mutation kind -> run it applies to and the violation class it must raise.
RUST_MUTATIONS = {
    "dropped-edge": ("drained", "successor_uncovered"),
    "retargeted-alias": ("drained", "alias_containment"),
    "dropped-frontier-record": ("frontier", "frontier_parity"),
    "seal-with-frontier": ("frontier", "seal_parity"),
    "seal-with-error": ("drained", "seal_parity"),
    "injected-false-hit": ("drained", "successor_uncovered"),
}


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


def mutate_injected_false_hit(document):
    """Resolve a later query to an earlier same-owner initial record that does not contain it."""
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


PYTHON_MUTATIONS = {
    "retargeted-alias": ("drained", mutate_retargeted_alias, "alias domain not contained in representative"),
    "dropped-frontier-record": ("frontier", mutate_dropped_frontier_record, "frontier parity"),
    "seal-with-frontier": ("drained", mutate_seal_with_frontier, "descendant_closed despite a frontier or error"),
    "seal-with-error": ("drained", mutate_seal_with_error, "descendant_closed despite a frontier or error"),
    "injected-false-hit": ("drained", mutate_injected_false_hit, "changed"),
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
    for name, run in runs.items():
        if run is None:
            continue
        report = AUDIT.audit_walk(run, command=Path(run) / "command.json", require_closure=True)
        expected = "PASS" if name == "drained" else "FAIL"
        rows.append({"oracle": "python-audit", "run": name, "mutation": None, "expected": expected,
                     "verdict": report["audit"], "ok": report["audit"] == expected,
                     "violations": report["violations"][:5]})
    for kind, (target, mutate, fragment) in PYTHON_MUTATIONS.items():
        run = runs.get(target)
        row = {"oracle": "python-audit", "run": target, "mutation": kind, "expected": "FAIL",
               "expected_violation": fragment}
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
        matched = [v for v in report["violations"] if fragment in v]
        rows.append(dict(row, detail=detail, verdict=report["audit"], matched=matched[:3],
                         ok=report["audit"] == "FAIL" and bool(matched), violations=report["violations"][:5]))
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


def rust_matrix(binary, runs, directory, threads):
    rows = []
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    for name, run in runs.items():
        if run is None:
            continue
        for require in (False, True):
            label = f"baseline-{name}{'-require-closure' if require else ''}"
            code, report, seconds, stderr = rust_run(binary, run, directory / f"{label}.json", threads,
                                                     ["--require-closure"] if require else [])
            expected = "FAIL" if (name == "frontier" and require) else "PASS"
            rows.append({"oracle": "walk-verify-closure", "run": name, "mutation": None,
                         "require_closure": require, "expected": expected, "verdict": report.get("verdict"),
                         "exit": code, "seconds": seconds, "ok": report.get("verdict") == expected,
                         "violations_by_class": report.get("violations_by_class"), "stderr_tail": stderr[-300:]})
    for kind, (target, klass) in RUST_MUTATIONS.items():
        run = runs.get(target)
        row = {"oracle": "walk-verify-closure", "run": target, "mutation": kind, "expected": "FAIL",
               "expected_violation": klass}
        if run is None:
            rows.append(dict(row, ok=False, verdict="not-run", reason=f"no {target} run"))
            continue
        code, report, seconds, stderr = rust_run(binary, run, directory / f"mutation-{kind}.json", threads,
                                                 ["--mutate", kind])
        mutation = report.get("mutation") or {}
        classes = report.get("violations_by_class") or {}
        rows.append(dict(row, verdict=report.get("verdict"), exit=code, seconds=seconds,
                         applied=mutation.get("applied"), detail=mutation, violations_by_class=classes,
                         ok=report.get("verdict") == "FAIL" and mutation.get("applied") is True and klass in classes,
                         stderr_tail=stderr[-300:]))
    return rows


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--run", type=Path, required=True, help="drained walk run directory (command.json, result.json)")
    parser.add_argument("--frontier-run", type=Path, help="walk run directory that retains frontiers")
    parser.add_argument("--rustred", type=Path, help="rustred executable with walk-verify-closure")
    parser.add_argument("--threads", type=int, default=1)
    parser.add_argument("--output", type=Path, required=True, help="matrix JSON; Rust reports go next to it")
    parser.add_argument("--skip-python", action="store_true")
    args = parser.parse_args(argv)
    runs = {"drained": args.run, "frontier": args.frontier_run}
    rows = [] if args.skip_python else python_matrix(runs)
    if args.rustred is not None:
        rows += rust_matrix(args.rustred, runs, args.output.with_suffix(""), args.threads)
    matrix = {"schema": "rustred.oracle-mutation-matrix.v1", "runs": {k: str(v) for k, v in runs.items()},
              "rustred": None if args.rustred is None else str(args.rustred),
              "all_ok": all(row["ok"] for row in rows), "rows": rows}
    args.output.write_text(json.dumps(matrix, indent=2) + "\n")
    for row in rows:
        print(f"{'ok ' if row['ok'] else 'BAD'} {row['oracle']:20s} {row['run']:9s} "
              f"{str(row['mutation']):24s} expected {row['expected']} got {row.get('verdict')}")
    return 0 if matrix["all_ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
