#!/usr/bin/env python
"""Oracle gate for one walk run directory (handoff 0.1 item 6).

Runs the W0.2 closure oracle on a saved walk, full F10 re-inspection:
  1. `rustred walk-verify-closure --reinspect all` (verifier binary of the
     oracle branch, default TMP/w0/oracle/bin/rustred-46d4dd28, built from
     fable_5_1-v3-oracle 4395ae41) -> <out>/verify.json;
  2. the paired streaming audit `audit_owner_domain_walk.py --verify-report`
     (oracle branch worktree) -> <out>/audit.json;
and asserts the gate:
  - drained runs (--require-closure, default): the verifier report passes
    `assert_oracle_pass.gate` (verdict == PASS, roots_independently_verified
    == roots_total >= 1, family_closure_claim false) and the audit is PASS;
  - runs with explicit frontiers (--expect-reference VERIFY.json AUDIT.json;
    verifier without --require-closure, like the W0.2 reference report):
    verdict == PASS, (roots_independently_verified, roots_total) equal the
    reference report's, and the audit's violation list (audit always with
    --require-closure, like the W0.2 reference audit) equals the reference
    audit's: the same frontiers and open roots, nothing else.

The oracle binary predates A10 (--frontier-policy): for a Stop-policy walk
pass --binding-from RECORD_CHECKPOINT. The checkpoint is then copied, and in
the copy's latest.json/previous.json only the `request` binding digest is
replaced by that of a Record-policy checkpoint of the same argv (A10 binds
Stop as the Record request plus one `frontier_policy` key); the argv given to
the verifier drops `--frontier-policy`. Everything else (sections, edges,
records, ledger, nodes) is verified as saved. The substitution is recorded.

usage: oracle_check.py RUN_DIR [--command ARGV.json] [--checkpoint DIR]
         [--result RESULT.json] [--cpus 80-87] [--threads 16]
         [--expect-reference VERIFY.json AUDIT.json] [--binding-from CKPT]
         [--out DIR] [--verifier BIN]
Exit 0 iff the gate passes; prints a one-line JSON summary.
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path("/common/dev/rustred")
ORACLE_WT = ROOT / ".claude/worktrees/fable51-oracle"
VERIFIER = ROOT / "TMP/w0/oracle/bin/rustred-46d4dd28"
AUDIT = ORACLE_WT / "examples/python/audit_owner_domain_walk.py"
sys.path.insert(0, str(ORACLE_WT / "examples/python"))
import assert_oracle_pass  # noqa: E402

ENV_ONE = {name: "1" for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                                  "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")}


def cpu_list(spec):
    cpus = []
    for part in spec.split(","):
        a, _, b = part.partition("-")
        cpus.extend(range(int(a), int(b or a) + 1))
    return cpus


def load_argv(path):
    value = json.loads(Path(path).read_text())
    if isinstance(value, dict):
        value = value.get("argv") or value.get("command")
    return list(value)


def option(argv, name):
    return argv[argv.index(name) + 1] if name in argv else None


def main():
    p = argparse.ArgumentParser()
    p.add_argument("run_dir", type=Path)
    p.add_argument("--command", type=Path)
    p.add_argument("--checkpoint", type=Path)
    p.add_argument("--result", type=Path)
    p.add_argument("--cpus", default="80-87,336-343")
    p.add_argument("--threads", type=int, default=16)
    p.add_argument("--expect-reference", nargs=2, type=Path, metavar=("VERIFY", "AUDIT"))
    p.add_argument("--binding-from", type=Path)
    p.add_argument("--out", type=Path)
    p.add_argument("--verifier", type=Path, default=VERIFIER)
    p.add_argument("--python", default=sys.executable)
    args = p.parse_args()
    run = args.run_dir.resolve()
    out = (args.out or run / "oracle").resolve()
    out.mkdir(parents=True, exist_ok=True)
    command_path = args.command or next(path for path in (run / "argv.json", run / "command.json") if path.exists())
    argv = load_argv(command_path)
    checkpoint = args.checkpoint or Path(option(argv, "--checkpoint") or option(argv, "--resume"))
    result = args.result or run / "result.json"
    substitution = None
    if "--frontier-policy" in argv:
        index = argv.index("--frontier-policy")
        policy = argv[index + 1]
        del argv[index:index + 2]
        if policy == "stop":
            if args.binding_from is None:
                sys.exit("a Stop-policy walk needs --binding-from RECORD_CHECKPOINT (oracle binary predates A10)")
            record_binding = json.loads((args.binding_from / "latest.json").read_text())["request"]
            copy = out / "checkpoint-binding-substituted"
            if copy.exists():
                shutil.rmtree(copy)
            shutil.copytree(checkpoint, copy, ignore=shutil.ignore_patterns("checkpoint.lock"))
            original = json.loads((checkpoint / "latest.json").read_text())["request"]
            for name in ("latest.json", "previous.json"):
                path = copy / name
                if path.exists():
                    manifest = json.loads(path.read_text())
                    manifest["request"] = record_binding
                    path.write_text(json.dumps(manifest))
            substitution = {"stop_binding": original, "record_binding": record_binding,
                            "record_binding_from": str(args.binding_from), "copy": str(copy),
                            "changed": "latest.json/previous.json `request` only; argv without --frontier-policy"}
            checkpoint = copy
    # The verifier resolves --checkpoint/--resume from the argv; name the checkpoint explicitly.
    for name in ("--checkpoint", "--resume"):
        if name in argv:
            argv[argv.index(name) + 1] = str(checkpoint)
    verify_argv_path = out / "verify-argv.json"
    verify_argv_path.write_text(json.dumps(argv, indent=1))
    closure = args.expect_reference is None
    env = dict(os.environ, **ENV_ONE, SYMBOLICA_HIDE_BANNER="1", TMPDIR=str(ROOT / "TMP"))
    cpus = cpu_list(args.cpus)
    verify = out / "verify.json"
    command = [str(args.verifier), "walk-verify-closure", "--command", str(verify_argv_path),
               "--checkpoint", str(checkpoint), "--threads", str(args.threads), "--reinspect", "all",
               "--output", str(verify), "--force", "--result", str(result)]
    if closure:
        command.append("--require-closure")
    with open(out / "verify.stdout", "w") as so, open(out / "verify.stderr", "w") as se:
        verify_code = subprocess.run(command, env=env, stdout=so, stderr=se,
                                     preexec_fn=lambda: (os.sched_setaffinity(0, cpus), os.nice(5))).returncode
    audit = out / "audit.json"
    # The audit always runs with --require-closure: the W0.2 reference audits
    # of the frontier fixture were made that way (their extra "closure
    # required" violations are part of the compared list).
    audit_command = [args.python, str(AUDIT), str(result.parent), "--command", str(verify_argv_path),
                     "--verify-report", str(verify), "--output", str(audit), "--require-closure"]
    with open(out / "audit.stdout", "w") as so, open(out / "audit.stderr", "w") as se:
        audit_code = subprocess.run(audit_command, env=env, stdout=so, stderr=se,
                                    preexec_fn=lambda: (os.sched_setaffinity(0, cpus), os.nice(5))).returncode
    report = json.loads(verify.read_text()) if verify.exists() else {}
    audited = json.loads(audit.read_text()) if audit.exists() else {}
    reasons = []
    if closure:
        reasons += assert_oracle_pass.gate(report)
        if audited.get("audit") != "PASS":
            reasons.append(f"audit {audited.get('audit')!r} (exit {audit_code})")
    else:
        reference = json.loads(args.expect_reference[0].read_text())
        reference_audit = json.loads(args.expect_reference[1].read_text())
        if report.get("verdict") != "PASS":
            reasons.append(f"verdict {report.get('verdict')!r} ({report.get('verdict_reason')})")
        pair = (report.get("roots_independently_verified"), report.get("roots_total"))
        expected = (reference.get("roots_independently_verified"), reference.get("roots_total"))
        if pair != expected:
            reasons.append(f"roots {pair} != reference {expected}")
        if sorted(audited.get("violations", [None])) != sorted(reference_audit.get("violations", [])):
            reasons.append("audit violations differ from the reference audit")
    summary = {"run": str(run), "gate": "PASS" if not reasons else "FAIL", "reasons": reasons,
               "mode": "require_closure" if closure else "frontier_reference",
               "verdict": report.get("verdict"), "verdict_reason": report.get("verdict_reason"),
               "roots_independently_verified": report.get("roots_independently_verified"),
               "roots_total": report.get("roots_total"), "violations_by_class": report.get("violations_by_class"),
               "audit": audited.get("audit"), "audit_violations": len(audited.get("violations", []) or []),
               "verify_exit": verify_code, "audit_exit": audit_code,
               "verify_seconds": (report.get("timing") or {}).get("total_seconds"),
               "verify_peak_rss_bytes": ((report.get("memory") or {}).get("at_end") or {}).get("peak_rss_bytes"),
               "verifier": str(args.verifier), "audit_script": str(AUDIT), "threads": args.threads,
               "cpus": args.cpus, "binding_substitution": substitution, "family_closure_claim": False}
    (out / "oracle-gate.json").write_text(json.dumps(summary, indent=1) + "\n")
    print(json.dumps(summary))
    return 0 if not reasons else 1


if __name__ == "__main__":
    raise SystemExit(main())
