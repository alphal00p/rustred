#!/usr/bin/env python
"""A10 Ready frontier-stop drill through the production launcher and the supervisor.

Production path under test: `production_saved_owner_campaign.py --start`
(steering v3: Ready publication, frontier policy stop, host RAM guard) execs
`shared_owner_campaign.py`, which supervises the native walk. The fixture is
the FG frontier fixture's inputs (TMP/four-loop-region-control.eazKG2/fg,
queries fg-unbounded.queries.json; 124 initial domains, frontiers under
every policy), staged into a fresh campaign directory under the run label
(never under campaigns/).

  1. Session 1: `--executable BIN --workers W --cpus C --start` (fresh);
     sessions 2..: `--resume --start` until a session's native result is no
     longer paused. Every paused session must exit 4 with supervisor state
     `paused`, native stop reason `frontier_policy`, a `frontier_stop` journal
     event, a saved checkpoint labelled `stop_reason`, and a frontier count
     above the previous session's.
  2. A Ready Record walk of the same native argv (no --frontier-policy, own
     checkpoint) gives the Record binding and the Record frontier total.
  3. Oracle on the final session (tools/research/ops/oracle_check.py, binding
     substituted from the Record checkpoint): verdict PASS with full F10
     re-inspection, roots_total equal to the Ordered fixture reference, audit
     violation kinds within the reference's (Ready record indices differ).
     The audit also reads the supervisor receipt, which the bare W0.2
     reference run lacks: a walk that ends with explicit frontiers exits 4
     (as the Ordered fixture itself did), so "resource receipt exit_status
     != 0" is admitted exactly when the final native status is incomplete
     with frontiers > 0 and the receipt's exit status is 4.

usage: ready_launcher_drill.py --binary BIN --label L [--cpus 80-85] [--workers 6]
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("/common/dev/rustred")
WT = ROOT / ".claude/worktrees/agent-ade877816b107b1cf"
INPUTS = ROOT / "TMP/four-loop-region-control.eazKG2/fg"
QUERIES = ROOT / "TMP/bounded-helper-control.SsjeHf/fg-unbounded.queries.json"
FIXTURE = ROOT / "TMP/w0/oracle/runs/frontier-fixture/fg"
REFERENCE = (ROOT / "TMP/w0/oracle/r2/verify/frontier-fixture-fg.json",
             ROOT / "TMP/w0/oracle/r2/audits/frontier-fixture-fg.json")
OUT_ROOT = ROOT / "TMP/w1-ops/runs"
LAUNCHER = WT / "examples/python/production_saved_owner_campaign.py"
ORACLE = WT / "tools/research/ops/oracle_check.py"
ENV_ONE = {name: "1" for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                                  "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def stage(campaign):
    inputs = campaign / "inputs"
    inputs.mkdir(parents=True)
    shutil.copy2(INPUTS / "selection.json", inputs / "selection.json")
    shutil.copytree(INPUTS / "owners", inputs / "owners")
    shutil.copy2(QUERIES, inputs / "queries.json")
    selection = json.loads((inputs / "selection.json").read_text())
    owners = [{"mask": row["mask"], "path": row["path"], "bytes": (inputs / row["path"]).stat().st_size,
               "sha256": digest(inputs / row["path"])} for row in selection["owners"]]
    (inputs / "input-receipt.json").write_text(json.dumps({
        "selection_sha256": digest(inputs / "selection.json"), "queries_sha256": digest(inputs / "queries.json"),
        "owners": owners, "source": {"inputs": str(INPUTS), "queries": str(QUERIES)},
        "note": "W1.4 fix-round Ready frontier-stop drill fixture (not a campaign)"}, indent=1))


def newest_run(campaign, before):
    runs = [path for path in (campaign / "runs").iterdir() if path.is_dir() and path.name not in before]
    return max(runs, key=lambda path: path.stat().st_mtime) if runs else None


def read(path):
    try:
        return json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return {}


def kinds(violations):
    return {re.sub(r"\(\d+ of \d+\)", "(n of m)", re.sub(r"^record \d+: ", "record: ", v)) for v in violations}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--binary", required=True, type=Path)
    p.add_argument("--label", required=True)
    p.add_argument("--cpus", default="80-85")
    p.add_argument("--workers", type=int, default=6)
    args = p.parse_args()
    out = OUT_ROOT / args.label
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    campaign = out / "campaign"
    stage(campaign)
    env = dict(os.environ, **ENV_ONE, TMPDIR=str(ROOT / "TMP"), SYMBOLICA_HIDE_BANNER="1")
    summary = {"binary": str(args.binary), "binary_sha256": digest(args.binary), "campaign": str(campaign),
               "cpus": args.cpus, "workers": args.workers, "sessions": [], "family_closure_claim": False}
    previous_frontiers = 0
    final_run = None
    for number in range(1, 201):
        launch = ([sys.executable, str(LAUNCHER), "--campaign-directory", str(campaign), "--executable",
                   str(args.binary), "--workers", str(args.workers), "--cpus", args.cpus, "--start"]
                  if number == 1 else
                  [sys.executable, str(LAUNCHER), "--campaign-directory", str(campaign), "--resume", "--start"])
        before = {path.name for path in (campaign / "runs").iterdir()} if (campaign / "runs").exists() else set()
        started = time.time()
        with open(out / f"launcher-{number:02d}.stdout", "w") as so, open(out / f"launcher-{number:02d}.stderr", "w") as se:
            code = subprocess.run(launch, env=env, stdout=so, stderr=se, cwd=ROOT).returncode
        run = newest_run(campaign, before)
        assert run is not None, f"session {number}: no run directory (exit {code})"
        supervisor = read(run / "supervisor-result.json")
        result = read(run / "result.json")
        request = read(run / "request.json")
        events = [json.loads(line) for line in (run / "events.jsonl").read_text().splitlines()] \
            if (run / "events.jsonl").exists() else []
        fired = [event for event in events if event.get("event") == "frontier_stop"]
        manifest = read(Path(request.get("checkpoint_directory", "")) / "latest.json")
        row = {"session": number, "run": run.name, "exit_code": code, "seconds": round(time.time() - started, 2),
               "supervisor_state": supervisor.get("state"), "native_stop_reason": supervisor.get("native_stop_reason"),
               "operator_or_resource_stop": supervisor.get("operator_or_resource_stop"),
               "status": result.get("status"), "frontiers": result.get("frontiers"),
               "committed_domains": result.get("committed_domains"), "frontier_stop_events": len(fired),
               "frontier_policy_in_command": request.get("frontier_policy"),
               "publication_policy": request.get("publication_policy"),
               "checkpoint_generation": manifest.get("generation"),
               "checkpoint_paused": (manifest.get("metadata") or {}).get("paused"),
               "checkpoint_stop_reason": (manifest.get("metadata") or {}).get("stop_reason"),
               "memory_admission_capped": (request.get("memory_admission") or {}).get("hard_capped_by_available_memory")}
        summary["sessions"].append(row)
        print(json.dumps(row), flush=True)
        assert row["frontier_policy_in_command"] == "stop" and row["publication_policy"] == "ready", row
        if result.get("status") != "paused":
            final_run = run
            break
        assert code == 4 and row["supervisor_state"] == "paused", row
        assert row["native_stop_reason"] == "frontier_policy" and fired, row
        assert row["operator_or_resource_stop"] is None, row
        assert row["checkpoint_paused"] is True and row["checkpoint_stop_reason"] == "frontier_policy", row
        assert row["frontiers"] > previous_frontiers, row
        previous_frontiers = row["frontiers"]
    assert final_run is not None, "no final session within 200"
    summary["stops"] = sum(1 for row in summary["sessions"] if row["status"] == "paused")
    final = read(final_run / "result.json")
    summary["final"] = {key: final.get(key) for key in ("status", "frontiers", "recursive_worklist_exhausted",
                                                         "completed_nodes", "committed_domains", "scheduled_nodes")}
    assert final.get("recursive_worklist_exhausted") is True and final.get("stop_reason") is None, summary["final"]
    # Ready Record walk of the same native argv: the Record binding and frontier total.
    native = list(read(final_run / "request.json")["command"])
    index = native.index("--frontier-policy")
    del native[index:index + 2]
    record = out / "ready-record"
    record.mkdir()

    def set_opt(name, value):
        native[native.index(name) + 1] = str(value)
    set_opt("--output", record / "result.json")
    set_opt("--events", record / "events.jsonl")
    set_opt("--stop-file", record / "stop-request.json")
    flag = "--resume" if "--resume" in native else "--checkpoint"
    native[native.index(flag)] = "--checkpoint"
    set_opt("--checkpoint", record / "checkpoint")
    (record / "argv.json").write_text(json.dumps(native, indent=1))
    cpus = [int(c) for part in args.cpus.split(",") for c in
            range(int(part.partition("-")[0]), int(part.partition("-")[2] or part.partition("-")[0]) + 1)]
    with open(record / "stdout", "w") as so, open(record / "stderr", "w") as se:
        code = subprocess.run(native, env=env, stdout=so, stderr=se,
                              preexec_fn=lambda: (os.sched_setaffinity(0, cpus), os.nice(5))).returncode
    recorded = read(record / "result.json")
    summary["ready_record"] = {"exit_code": code, "status": recorded.get("status"),
                               "frontiers": recorded.get("frontiers"),
                               "recursive_worklist_exhausted": recorded.get("recursive_worklist_exhausted"),
                               "completed_nodes": recorded.get("completed_nodes")}
    # Oracle on the final Stop session (binding substituted from the Record checkpoint).
    oracle = subprocess.run([sys.executable, str(ORACLE), str(final_run), "--command", str(final_run / "request.json"),
                             "--binding-from", str(record / "checkpoint"), "--expect-reference", *map(str, REFERENCE),
                             "--out", str(out / "oracle-stop-final"), "--cpus", args.cpus, "--threads",
                             str(len(cpus))], capture_output=True, text=True, env=env)
    gate = json.loads(oracle.stdout.strip().splitlines()[-1]) if oracle.stdout.strip() else {"gate": "ERROR"}
    reference_audit = read(REFERENCE[1])
    audit = read(out / "oracle-stop-final" / "audit.json")
    extra = sorted(kinds(audit.get("violations", [])) - kinds(reference_audit.get("violations", [])))
    receipt = read(final_run / "supervisor-result.json")
    admitted = []
    if (final.get("status") == "incomplete" and (final.get("frontiers") or 0) > 0
            and receipt.get("exit_status") == 4 and "resource receipt exit_status != 0" in extra):
        extra.remove("resource receipt exit_status != 0")
        admitted.append("resource receipt exit_status != 0 (final native status incomplete with frontiers: exit 4)")
    reference = read(REFERENCE[0])
    summary["oracle_stop_final"] = {
        "verdict": gate.get("verdict"), "verdict_reason": gate.get("verdict_reason"),
        "roots_independently_verified": gate.get("roots_independently_verified"),
        "roots_total": gate.get("roots_total"),
        "ordered_reference_roots": [reference.get("roots_independently_verified"), reference.get("roots_total")],
        "audit": gate.get("audit"), "audit_violation_kinds_outside_reference": extra,
        "admitted_audit_violation_kinds": admitted,
        "exact_ordered_reference_gate": gate.get("gate"), "binding_substitution": gate.get("binding_substitution")}
    ok = (gate.get("verdict") == "PASS" and "every native re-inspected" in (gate.get("verdict_reason") or "")
          and gate.get("roots_total") == reference.get("roots_total") and not extra)
    summary["oracle_gate_ready"] = "PASS" if ok else "FAIL"
    summary["frontier_total_equals_ready_record"] = final.get("frontiers") == recorded.get("frontiers")
    (out / "summary.json").write_text(json.dumps(summary, indent=1) + "\n")
    print(json.dumps({key: value for key, value in summary.items() if key != "sessions"}, indent=1))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
