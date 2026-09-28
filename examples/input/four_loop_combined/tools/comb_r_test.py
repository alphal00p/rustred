#!/usr/bin/env python
"""C-4L-comb-R decision rule (README section 5, "How to use four-all", item 2): a pre-registered one-sided
comparison of drain fractions, candidate engine against the frozen legacy engine, per width, from runs
interleaved in the same session on the same CPUs.

usage:
  comb_r_test.py --candidate GLOB [--candidate GLOB ...] --legacy GLOB [--legacy GLOB ...]
                 [--verify REPORT.json ...] [--alpha 0.05] [--min-n 13] [--threshold 1000]
                 [--breakdown-dir DIR] [--audit] [--output OUT.json]
  comb_r_test.py --self-test

GLOBs select walk directories <out-root>/<label>/<family> (each with metrics.json, written by
tools/run_c4l.py). Per run:
  drained    = the native finished on its own (exit 0, no stop requested), 0 frontiers, audit PASS
               (examples/python/audit_owner_domain_walk.py; --audit runs it where audit.json is missing);
  not drained = stopped at the cap or at the native early stop (--stop-natives 60000);
  violation  = a frontier, an error exit, or a run that finished on its own with audit FAIL.
Signature of a run that did not drain (tools/records_breakdown.py over its committed-records sidecar;
written to --breakdown-dir as bd-<label>-<family>.json when missing): N993 and N1009 = Apply natives of
owners 0111110010 and 0111111001 at rank anchor + 1 (anchor = the owner's anchor/helper rank in the
queries). KNOWN legacy signature iff max(N993, N1009) >= --threshold (1,000); anchor + 1 natives on other
owners are allowed. Measured baseline [M, README section 5]: drained runs 30 (one run 188) and 0; runs
stopped at 60 k natives 1,500-13,755 on 0111110010, or 2,813-3,050 on 0111111001.

Per width W (from metrics.json "workers"), with d_c/n_c and d_l/n_l drained/total:
  p_regression  = one-sided Fisher exact P(candidate fraction <= observed | margins), H1: p_c < p_l;
  p_improvement = one-sided Fisher exact, H1: p_c > p_l.
Verdict (printed and written to --output):
  BLOCK  if any candidate violation; any candidate non-drain without the known signature; any width with
         p_regression < alpha; or a drained candidate verify-closure report that is not a gate PASS
         (verdict PASS and roots_independently_verified == roots_total >= 1).
  VOID   if a width has n < min-n in either arm, or the two arms of a width used different CPU sets
         (the comparison is repeated; nothing is promoted on a void comparison).
  INCOMPLETE if fewer than two gate-PASS verify-closure reports cover drained candidate runs of a width.
  PASS   otherwise. p_improvement < alpha is reported as "improvement shown"; it is required only when the
         change is claimed to make Rolling/Ready drainage robust.
Family-wise false-BLOCK rate under the null over three widths <= 1 - 0.95^3 = 0.14 [E; Fisher is
conservative]. Stdlib only.
"""
import argparse
import glob
import json
import os
import subprocess
import sys
from math import comb
from pathlib import Path

HERE = Path(__file__).resolve().parent
BREAKDOWN = HERE / "records_breakdown.py"
REPO = HERE.parents[3]
AUDIT = REPO / "examples/python/audit_owner_domain_walk.py"
WALK_CWD = Path("/common/dev/rustred")  # cwd of run_c4l.py natives and of the verifier runs
FLOOD_OWNERS = ("0111110010", "0111111001")


def fisher(a, n1, b, n2):
    """Fisher exact test on drained a/n1 (arm 1) against b/n2 (arm 2).
    Returns (p_less, p_greater, p_two_sided) for the arm-1 fraction."""
    k_tot = a + b
    lo, hi = max(0, k_tot - n2), min(n1, k_tot)
    denom = comb(n1 + n2, k_tot)
    probs = {k: comb(n1, k) * comb(n2, k_tot - k) / denom for k in range(lo, hi + 1)}
    p_less = sum(v for k, v in probs.items() if k <= a)
    p_greater = sum(v for k, v in probs.items() if k >= a)
    p_two = sum(v for v in probs.values() if v <= probs[a] * (1 + 1e-7))
    return min(1.0, p_less), min(1.0, p_greater), min(1.0, p_two)


def binom(n, k, p):
    return comb(n, k) * p ** k * (1 - p) ** (n - k)


def power(n_c, p_c, n_l, p_l, alpha=0.05, side="less"):
    """Exact probability that the one-sided test rejects, both arms binomial."""
    total = 0.0
    for a in range(n_c + 1):
        for b in range(n_l + 1):
            pl, pg, _ = fisher(a, n_c, b, n_l)
            if (pl if side == "less" else pg) < alpha:
                total += binom(n_c, a, p_c) * binom(n_l, b, p_l)
    return total


def gate_pass(report):
    """Oracle gate contract (fable51_w0_oracle note section 1)."""
    try:
        d = json.load(open(report))
    except (OSError, ValueError):
        return False, "unreadable"
    ok = (d.get("verdict") == "PASS" and d.get("family_closure_claim") is False
          and isinstance(d.get("roots_total"), int) and d["roots_total"] >= 1
          and d.get("roots_independently_verified") == d["roots_total"])
    return ok, f"{d.get('verdict')} {d.get('roots_independently_verified')}/{d.get('roots_total')}"


def signature(run, bd_dir):
    label, family = run.parent.name, run.name
    out = Path(bd_dir) / f"bd-{label}-{family}.json"
    if not out.exists():
        recs = sorted(run.glob("checkpoint/records-*.jsonl"))
        argv = json.load(open(run / "command.json"))
        if not recs:
            return None
        subprocess.call([sys.executable, str(BREAKDOWN), str(recs[-1]), argv[argv.index("--queries") + 1],
                         argv[argv.index("--manifest") + 1], str(out), "--procs", "8"], stdout=subprocess.DEVNULL)
    if not out.exists():
        return None
    bd = json.load(open(out))
    counts = {}
    for row in bd.get("per_owner_apply") or []:
        if row.get("owner") in FLOOD_OWNERS and row.get("helper_rank") is not None:
            counts[row["owner"]] = (row.get("apply_native_rank_hist") or {}).get(str(row["helper_rank"] + 1), 0)
    other = sorted(((row["owner"], (row.get("apply_native_rank_hist") or {}).get(str(row["helper_rank"] + 1), 0))
                    for row in bd.get("per_owner_apply") or []
                    if row.get("owner") not in FLOOD_OWNERS and row.get("helper_rank") is not None),
                   key=lambda t: -t[1])[:3]
    return {"breakdown": str(out), "N993": counts.get("0111110010", 0), "N1009": counts.get("0111111001", 0),
            "top_other_anchor_plus_1": other}


def classify(run, args):
    m = json.load(open(run / "metrics.json"))
    apath = run / "audit.json"
    if not apath.exists() and args.audit:
        subprocess.call([sys.executable, str(AUDIT), str(run)], stdout=open(run / "audit.stdout", "w"),
                        stderr=subprocess.STDOUT)
    audit = json.load(open(apath)).get("audit") if apath.exists() else None
    stopped = bool(m.get("cooperative_stop_requested"))
    frontiers = int(float(m.get("frontiers") or 0))
    finished = m.get("exit_code") == 0 and not stopped
    row = {"run": str(run), "workers": m.get("workers"), "cpus": m.get("cpus"), "policy": m.get("policy"),
           "binary": m.get("binary"), "natives": int(float(m.get("completed_nodes") or 0)),
           "stop_reason": m.get("stop_reason"), "audit": audit, "frontiers": frontiers,
           "traversal_seconds": m.get("traversal_seconds"),
           "foreign_fraction": (m.get("foreign_load") or {}).get("foreign_fraction_of_run_cpus")}
    violations = []
    if frontiers:
        violations.append(f"{frontiers} frontiers")
    if m.get("exit_code") not in (0, 4) and not stopped:
        violations.append(f"exit code {m.get('exit_code')}")
    if finished and audit != "PASS":
        violations.append(f"finished on its own with audit {audit}")
    row["violations"] = violations
    row["drained"] = finished and audit == "PASS" and not frontiers
    if not row["drained"] and not violations:
        sig = signature(run, args.breakdown_dir)
        row["signature"] = sig
        row["known_signature"] = bool(sig) and max(sig["N993"], sig["N1009"]) >= args.threshold
    return row


def self_test():
    # values quoted in README section 5 (two-sided, drained/total)
    checks = [((7, 13, 17, 18), 2, 0.0124), ((7, 8, 10, 10), 2, 0.4444), ((7, 8, 7, 13), 2, 0.1736),
              ((9, 11, 7, 13), 2, 0.2108), ((2, 3, 4, 8), 2, 1.0), ((3, 5, 5, 5), 2, 0.4444),
              ((0, 13, 7, 13), 0, 0.0026), ((2, 13, 7, 13), 0, 0.0484), ((3, 13, 7, 13), 0, 0.1131)]
    for args, idx, want in checks:
        got = fisher(*args)[idx]
        assert abs(got - want) < 6e-4, (args, idx, got, want)
    assert abs(power(13, 0.0, 13, 0.54) - 0.98) < 0.01
    print("self-test PASS")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--candidate", action="append", default=[])
    p.add_argument("--legacy", action="append", default=[])
    p.add_argument("--verify", action="append", default=[], help="walk-verify-closure report of a drained candidate run")
    p.add_argument("--alpha", type=float, default=0.05)
    p.add_argument("--min-n", type=int, default=13)
    p.add_argument("--threshold", type=int, default=1000)
    p.add_argument("--breakdown-dir", default=".")
    p.add_argument("--audit", action="store_true")
    p.add_argument("--output", type=Path)
    p.add_argument("--self-test", action="store_true")
    args = p.parse_args()
    if args.self_test:
        self_test()
        return
    arms = {}
    for arm, globs in (("candidate", args.candidate), ("legacy", args.legacy)):
        runs = sorted({Path(r) for g in globs for r in glob.glob(g) if os.path.exists(os.path.join(r, "metrics.json"))})
        arms[arm] = [classify(r, args) for r in runs]
    verify = {}
    for rep in args.verify:
        ok, why = gate_pass(rep)
        try:
            ckpt = json.load(open(rep)).get("checkpoint", {}).get("directory") or ""
        except (OSError, ValueError, AttributeError):
            ckpt = ""
        if ckpt and not os.path.isabs(ckpt):
            ckpt = str(WALK_CWD / ckpt)
        verify[rep] = {"gate_pass": ok, "detail": why, "checkpoint": os.path.realpath(ckpt) if ckpt else ""}
    widths = sorted({r["workers"] for rows in arms.values() for r in rows if r["workers"]})
    per_width, block, void, incomplete = [], [], [], []
    for w in widths:
        c = [r for r in arms["candidate"] if r["workers"] == w]
        l = [r for r in arms["legacy"] if r["workers"] == w]
        dc, dl = sum(r["drained"] for r in c), sum(r["drained"] for r in l)
        row = {"workers": w, "candidate": f"{dc}/{len(c)}", "legacy": f"{dl}/{len(l)}"}
        if len(c) < args.min_n or len(l) < args.min_n:
            void.append(f"W{w}: n below {args.min_n}")
        if {r["cpus"] for r in c} != {r["cpus"] for r in l} or len({r["cpus"] for r in c + l}) != 1:
            void.append(f"W{w}: arms on different CPU sets {sorted({r['cpus'] for r in c + l})}")
        if c and l:
            pl, pg, _ = fisher(dc, len(c), dl, len(l))
            row.update({"p_regression": round(pl, 4), "p_improvement": round(pg, 4),
                        "regression": pl < args.alpha, "improvement_shown": pg < args.alpha})
            if pl < args.alpha:
                block.append(f"W{w}: regression {dc}/{len(c)} vs {dl}/{len(l)}, one-sided Fisher p = {pl:.4f}")
        for r in c:
            if r["violations"]:
                block.append(f"{r['run']}: {'; '.join(r['violations'])}")
            elif not r["drained"] and not r.get("known_signature"):
                block.append(f"{r['run']}: non-drain without the known signature ({r.get('signature')})")
        drained_c = {os.path.realpath(os.path.join(r["run"], "checkpoint")) for r in c if r["drained"]}
        covered = [rep for rep, v in verify.items() if v["gate_pass"] and v["checkpoint"] in drained_c]
        failed = [rep for rep, v in verify.items() if not v["gate_pass"] and v["checkpoint"] in drained_c]
        block.extend(f"{rep}: verify-closure not a gate PASS ({verify[rep]['detail']})" for rep in failed)
        if drained_c and len(covered) < 2:
            incomplete.append(f"W{w}: {len(covered)} gate-PASS verify-closure report(s) on drained candidate runs (need 2)")
        row["natives_drained_candidate"] = sorted(r["natives"] for r in c if r["drained"])
        row["natives_drained_legacy"] = sorted(r["natives"] for r in l if r["drained"])
        per_width.append(row)
    verdict = "BLOCK" if block else "VOID" if void else "INCOMPLETE" if incomplete else "PASS"
    out = {"schema": "c4l.comb-r.v1", "verdict": verdict, "alpha": args.alpha, "min_n": args.min_n,
           "signature_threshold": args.threshold, "per_width": per_width, "block": block, "void": void,
           "incomplete": incomplete, "verify": verify, "runs": arms, "family_closure_claim": False}
    if args.output:
        json.dump(out, open(args.output, "w"), indent=1)
    print(json.dumps({k: out[k] for k in ("verdict", "per_width", "block", "void", "incomplete")}, indent=1))


if __name__ == "__main__":
    main()
