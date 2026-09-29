#!/usr/bin/env python3
"""Gate on `rustred walk-verify-closure` reports: exit 0 only on a certified PASS.

A report passes the gate when, and only when:

* `schema` is the verifier schema (`rustred.walk-verify-closure.v*`);
* `verdict == "PASS"` (not FAIL, not INCOMPLETE);
* `roots_total >= 1` and `roots_independently_verified == roots_total`
  (every root record the queries map to is closed and every native of its
  dependency cone was re-inspected by the F10 reference; the verifier reports
  0 verified roots whenever any violation was found);
* `family_closure_claim` is false (the oracle never claims family closure);
* the report certifies the run as it is: `mutation` is null (a `--mutate`
  report is a negative control, never a certificate), `reinspection.complete`
  is true (full F10 re-inspection of every candidate native), and the F10
  reference ran with its native levers off (`reference.native_levers ==
  "Off"`). A report made with `--reference-levers as-run` passes only when the
  caller declares it with `--allow-as-run-reference`.

`roots_total` counts distinct root records (several queries can share one
root), not queries. For a walk with frontier-rescue amendments the verifier
certifies per physics query (`certification_scope`
`physics_queries_through_closed_containing_roots`): `roots_total` then counts
the certifying roots (the first closed input root containing each physics
query); helper roots are reported under `helper_roots`, not required. Both fields sit at the top level of the report. A report
run without `--require-closure` can still pass the gate: the equality above
already implies that every root is closed.

usage: assert_oracle_pass.py REPORT.json [REPORT.json ...] [--quiet] [--allow-as-run-reference]
Exit status: 0 all gates pass, 1 some report fails the gate, 2 unreadable.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

SCHEMA_PREFIX = "rustred.walk-verify-closure."


def gate(report, allow_as_run_reference=False):
    """Return the list of reasons the report fails the gate (empty: pass)."""
    reasons = []
    schema = report.get("schema")
    if not isinstance(schema, str) or not schema.startswith(SCHEMA_PREFIX):
        reasons.append(f"schema {schema!r} is not a walk-verify-closure report")
    verdict = report.get("verdict")
    if verdict != "PASS":
        reasons.append(f"verdict {verdict!r} != 'PASS' ({report.get('verdict_reason')})")
    total = report.get("roots_total")
    verified = report.get("roots_independently_verified")
    if not isinstance(total, int) or not isinstance(verified, int):
        reasons.append(f"missing gate fields roots_total={total!r} roots_independently_verified={verified!r}")
    else:
        if total < 1:
            reasons.append("roots_total is 0 (nothing was verified)")
        if verified != total:
            reasons.append(f"roots_independently_verified {verified} != roots_total {total}")
    if report.get("family_closure_claim") is not False:
        reasons.append("family_closure_claim is not false")
    if "mutation" not in report:
        reasons.append("missing field mutation (cannot tell a certificate from a negative control)")
    elif report.get("mutation") is not None:
        reasons.append(f"mutated report (mutation {report.get('mutation')!r}): a negative control, not a certificate")
    reinspection = report.get("reinspection")
    if not isinstance(reinspection, dict) or reinspection.get("complete") is not True:
        reasons.append("reinspection.complete is not true (partial or no F10 re-inspection)")
    reference = report.get("reference")
    levers = reference.get("native_levers") if isinstance(reference, dict) else None
    if levers != "Off" and not (allow_as_run_reference and levers == "AsRun"):
        reasons.append(f"reference.native_levers {levers!r} != 'Off' (as-run references need "
                       "--allow-as-run-reference)")
    return reasons


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("reports", nargs="+", type=Path)
    parser.add_argument("--quiet", action="store_true")
    parser.add_argument("--allow-as-run-reference", action="store_true",
                        help="accept reports whose F10 reference kept the run's native levers (declared use only)")
    args = parser.parse_args(argv)
    status = 0
    for path in args.reports:
        try:
            report = json.loads(path.read_text())
        except (OSError, ValueError) as error:
            print(f"UNREADABLE {path}: {error}", file=sys.stderr)
            status = max(status, 2)
            continue
        reasons = gate(report, allow_as_run_reference=args.allow_as_run_reference)
        if reasons:
            status = max(status, 1)
            print(f"GATE-FAIL {path}: " + "; ".join(reasons), file=sys.stderr)
        elif not args.quiet:
            scope = report.get("certification_scope") or "all_roots"
            print(f"GATE-PASS {path}: {report['roots_independently_verified']}/{report['roots_total']} "
                  f"roots independently verified (scope {scope})")
    return status


if __name__ == "__main__":
    raise SystemExit(main())
