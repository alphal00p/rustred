#!/usr/bin/env python
"""W1.4 A10 frontier drill on the FG frontier fixture (TMP/w0/oracle/runs/frontier-fixture/fg).

The fixture is 4a17f9c7's Ordered W6 walk of fg-unbounded.queries.json: 124 domains, 85 frontiers in
13 records, exit 4. This drill runs BIN with the fixture argv (binary, output paths and checkpoint
rewritten), in one of three modes:

  stop      --frontier-policy stop: session 1 must stop at the first frontier with exit 4, a paused
            receipt with stop_reason frontier_policy, a frontier_stop journal event and a saved
            checkpoint labelled stop_reason; every later session resumes (--resume, same policy) and
            stops at the next new frontier, until a session ends without pausing. The final report is
            compared strictly with the fixture's record-policy result.
  record    no flag (the historical default): one fresh walk, strict compare with the fixture.
  binding   copies the fixture's own 4a17f9c7 CP5 checkpoint and resumes it with BIN under record
            (must be accepted: identical request binding) and under stop (must be refused).

Never writes into the fixture directory. Usage:
  frontier_drill.py --binary BIN --mode stop|record|binding --label L [--cpus 80-85]
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("/common/dev/rustred")
FIXTURE = ROOT / "TMP/w0/oracle/runs/frontier-fixture/fg"
OUT_ROOT = ROOT / "TMP/w1-ops/runs"
COMPARE = ROOT / ".claude/worktrees/agent-ade877816b107b1cf/examples/python/compare_walk_records.py"
ENV_ONE = {name: "1" for name in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                                  "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS")}


def cpu_list(spec):
    cpus = []
    for part in spec.split(","):
        a, _, b = part.partition("-")
        cpus.extend(range(int(a), int(b or a) + 1))
    return cpus


def argv_for(binary, session, checkpoint, resume, policy):
    argv = list(json.load(open(FIXTURE / "command.json")))
    argv[0] = str(binary)

    def set_opt(name, value):
        argv[argv.index(name) + 1] = str(value)

    set_opt("--output", session / "result.json")
    set_opt("--events", session / "events.jsonl")
    set_opt("--stop-file", session / "stop-request.json")
    index = argv.index("--checkpoint")
    argv[index:index + 2] = ["--resume" if resume else "--checkpoint", str(checkpoint)]
    if policy is not None:
        argv += ["--frontier-policy", policy]
    return argv


def run(argv, session, cpus):
    session.mkdir(parents=True)
    json.dump(argv, open(session / "argv.json", "w"), indent=1)
    env = dict(os.environ, **ENV_ONE, TMPDIR=str(ROOT / "TMP"), SYMBOLICA_HIDE_BANNER="1")
    started = time.time()
    with open(session / "stdout", "w") as out, open(session / "stderr", "w") as err:
        code = subprocess.run(argv, env=env, stdout=out, stderr=err,
                              preexec_fn=lambda: (os.sched_setaffinity(0, cpus), os.nice(5))).returncode
    return code, time.time() - started


def events(session):
    path = session / "events.jsonl"
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def strict(reference, candidate, output, ignore_top=()):
    """compare_walk_records --mode strict; returns verdict, record counts, differing records and the
    top-level keys that differ (after the explicitly ignored ones, which are listed)."""
    command = [sys.executable, str(COMPARE), "--mode", "strict", str(reference), str(candidate),
               "--output", str(output)]
    for key in ignore_top:
        command += ["--ignore-top", key]
    subprocess.run(command, capture_output=True, text=True)
    report = json.loads(output.read_text()) if output.exists() else {}
    return {"verdict": report.get("verdict"), "records": [report.get("a", {}).get("records"),
            report.get("b", {}).get("records")], "differing_records": report.get("differing_records"),
            "top_level_differences": sorted(report.get("top_level_differences", {})),
            "explicitly_ignored_top_level": list(ignore_top)}


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--binary", required=True)
    p.add_argument("--mode", required=True, choices=("stop", "record", "binding"))
    p.add_argument("--label", required=True)
    p.add_argument("--cpus", default="80-85")
    args = p.parse_args()
    cpus = cpu_list(args.cpus)
    out = OUT_ROOT / args.label
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    reference = FIXTURE / "result.json"
    summary = {"binary": args.binary, "mode": args.mode, "fixture": str(FIXTURE), "cpus": args.cpus,
               "sessions": [], "family_closure_claim": False}
    if args.mode == "binding":
        checkpoint = out / "checkpoint-4a17f9c7-copy"
        shutil.copytree(FIXTURE / "checkpoint", checkpoint)
        (checkpoint / "checkpoint.lock").unlink(missing_ok=True)
        for policy, name in ((None, "resume-record"), ("stop", "resume-stop")):
            session = out / name
            code, seconds = run(argv_for(args.binary, session, checkpoint, True, policy), session, cpus)
            document = json.loads((session / "result.json").read_text())
            summary["sessions"].append({"session": name, "policy": policy or "record(default)",
                                        "exit_code": code, "seconds": round(seconds, 3),
                                        "status": document.get("status"), "error": document.get("error"),
                                        "frontiers": document.get("frontiers")})
        record, stop = summary["sessions"]
        summary["record_resume_accepted"] = record["status"] not in (None, "preparation_error")
        summary["stop_resume_refused"] = (stop["status"] == "preparation_error"
                                          and "request or policy differs" in str(stop["error"]))
        if summary["record_resume_accepted"]:
            summary["record_resume_strict"] = strict(reference, out / "resume-record/result.json",
                                                     out / "resume-record/strict-vs-fixture.json",
                                                     ("uncommitted_inspections",))
    elif args.mode == "record":
        session = out / "session-01"
        code, seconds = run(argv_for(args.binary, session, out / "checkpoint", False, None), session, cpus)
        document = json.loads((session / "result.json").read_text())
        summary["sessions"].append({"session": 1, "exit_code": code, "seconds": round(seconds, 3),
                                    "status": document.get("status"), "frontiers": document.get("frontiers"),
                                    "frontier_policy_key_present": "frontier_policy" in document})
        summary["strict_vs_fixture"] = strict(reference, session / "result.json", session / "strict-vs-fixture.json")
    else:
        checkpoint = out / "checkpoint"
        previous = 0
        for number in range(1, 201):
            session = out / f"session-{number:02d}"
            code, seconds = run(argv_for(args.binary, session, checkpoint, number > 1, "stop"), session, cpus)
            document = json.loads((session / "result.json").read_text())
            journal = events(session)
            fired = [event for event in journal if event.get("event") == "frontier_stop"]
            manifest = json.loads((checkpoint / "latest.json").read_text())
            row = {"session": number, "exit_code": code, "seconds": round(seconds, 3),
                   "status": document.get("status"), "stop_reason": document.get("stop_reason"),
                   "frontiers": document.get("frontiers"), "committed_domains": document.get("committed_domains"),
                   "frontier_stop_events": len(fired),
                   "session_start_frontiers": fired[0].get("session_start_frontiers") if fired else None,
                   "stop_event_frontiers": fired[0].get("frontiers") if fired else None,
                   "checkpoint_generation": manifest.get("generation"),
                   "checkpoint_paused": manifest["metadata"].get("paused"),
                   "checkpoint_stop_reason": manifest["metadata"].get("stop_reason")}
            summary["sessions"].append(row)
            print(json.dumps(row), flush=True)
            if document.get("status") != "paused":
                break
            assert code == 4 and row["stop_reason"] == "frontier_policy" and fired, row
            assert row["checkpoint_paused"] is True and row["checkpoint_stop_reason"] == "frontier_policy", row
            assert row["session_start_frontiers"] == previous and row["frontiers"] > previous, row
            previous = row["frontiers"]
        summary["stops"] = sum(1 for row in summary["sessions"] if row["status"] == "paused")
        # Session receipts (frontier_policy, stop_reason) and resume bookkeeping
        # (uncommitted_inspections) are the only admitted top-level differences.
        summary["final_strict_vs_fixture"] = strict(reference, session / "result.json",
                                                    session / "strict-vs-fixture.json",
                                                    ("frontier_policy", "stop_reason", "uncommitted_inspections"))
    json.dump(summary, open(out / "summary.json", "w"), indent=1)
    print(json.dumps({key: value for key, value in summary.items() if key != "sessions"}, indent=1))


if __name__ == "__main__":
    main()
