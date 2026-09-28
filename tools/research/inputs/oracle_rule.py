#!/usr/bin/env python3
"""Oracle rule for the inputs lane's gates (HANDOFF_opus_5_5.md 0.1 item 6).

For each LABEL=RUNDIR: the record audit (RUNDIR/audit.json, `audit` field, if the
run was audited in place, and/or the paired round-2 audit below) and
the independent closure-verifier report VERIFY_DIR/LABEL.json
(`rustred walk-verify-closure --require-closure`, full F10 re-inspection),
gated as assert_oracle_pass.py does: verdict == PASS AND
roots_independently_verified == roots_total >= 1 AND family_closure_claim false.
Optionally VERIFY_DIR/LABEL.audit.json (the round-2 audit paired with the report
by --verify-report) must also PASS.

Overall verdict:
  PASS         every record audit PASS, every oracle gate PASS, oracle branch merged
  PROVISIONAL  record audits PASS, oracle gates PASS or missing, oracle branch not merged
               or some report missing ("record audit only")
  INCOMPLETE   some arm has no record audit at all
  FAIL         any record audit or any present oracle report / paired audit fails

Usage: oracle_rule.py LABEL=RUNDIR [...] --verify-dir DIR [--output OUT.json]
"""
import argparse
import json
from pathlib import Path

from c5f_gate import branch_merged, oracle_gate


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("arms", nargs="+")
    p.add_argument("--verify-dir", type=Path, required=True)
    p.add_argument("--repo", type=Path, default=Path("/common/dev/rustred"))
    p.add_argument("--oracle-branch", default="fable_5_1-v3-oracle")
    p.add_argument("--base-branch", default="fable_5_1")
    p.add_argument("--output", type=Path)
    args = p.parse_args(argv)
    merged = branch_merged(args.repo, args.oracle_branch, args.base_branch)
    rows = []
    for spec in args.arms:
        label, _, run = spec.partition("=")
        run = Path(run)
        audit_path = run / "audit.json"
        audit = json.loads(audit_path.read_text()).get("audit") if audit_path.exists() else None
        oracle, reasons = oracle_gate(args.verify_dir / f"{label}.json")
        report = args.verify_dir / f"{label}.json"
        rep = json.loads(report.read_text()) if report.exists() else {}
        paired_path = args.verify_dir / f"{label}.audit.json"
        paired = json.loads(paired_path.read_text()).get("audit") if paired_path.exists() else None
        present = [a for a in (audit, paired) if a is not None]
        record = "MISSING" if not present else "PASS" if all(a == "PASS" for a in present) else "FAIL"
        rows.append({"label": label, "run": str(run), "run_dir_audit": audit, "record_audit": record,
                     "oracle_gate": oracle,
                     "oracle_reasons": reasons, "paired_audit": paired,
                     "roots_total": rep.get("roots_total"),
                     "roots_independently_verified": rep.get("roots_independently_verified"),
                     "verdict": rep.get("verdict"), "verdict_reason": rep.get("verdict_reason")})
    if any(r["record_audit"] == "FAIL" or r["oracle_gate"] == "FAIL" for r in rows):
        final = "FAIL"
    elif any(r["record_audit"] == "MISSING" for r in rows):
        final = "INCOMPLETE (record audit missing)"
    elif any(r["oracle_gate"] == "MISSING" for r in rows):
        final = "PROVISIONAL (record audit only)"
    elif merged is not True:
        final = "PROVISIONAL (oracle PASS, oracle branch not merged)"
    else:
        final = "PASS"
    out = {"oracle_branch": args.oracle_branch, "base_branch": args.base_branch, "oracle_branch_merged": merged,
           "verify_dir": str(args.verify_dir), "arms": rows, "verdict": final}
    text = json.dumps(out, indent=1, sort_keys=True)
    if args.output:
        args.output.write_text(text + "\n")
    for r in rows:
        print(f"{r['label']}: record audit {r['record_audit']} (run dir {r['run_dir_audit']}); oracle {r['oracle_gate']} "
              f"({r['verdict']}, roots {r['roots_independently_verified']}/{r['roots_total']}); paired audit {r['paired_audit']}")
    print(f"oracle branch {args.oracle_branch} merged into {args.base_branch}: {merged}; verdict {final}")
    return 0 if final == "PASS" else 1 if final == "FAIL" else 3


if __name__ == "__main__":
    raise SystemExit(main())
