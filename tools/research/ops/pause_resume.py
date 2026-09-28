#!/usr/bin/env python
"""Mid-walk pause/resume control (cross-binary and under a frontier policy).

Run 1 starts a control family (run_control.COMMANDS) with FIRST binary and a
checkpoint, touches the native stop file once the events journal reports
`committed_domains >= --stop-at-committed`, and must exit 4 with a paused
receipt (cooperative saved pause). Run 2 resumes that checkpoint with SECOND
binary (`--checkpoint` replaced by `--resume`) to the end. Both runs get the
same --extra native arguments (e.g. `--frontier-policy stop`). Run 2's
result.json is compared with REFERENCE (compare_walk_records --mode strict,
ignoring only the named top-level keys) and, with --oracle, verified by
tools/research/ops/oracle_check.py (drained: --require-closure gate).

Like TMP/fable51-controls/resume_control.py (which takes no --extra), but
the binaries run directly (no `nix develop`), pinned and niced.

usage: pause_resume.py --first BIN --second BIN --family five-finite --label L
         --cpus 128-177 [--workers 50] [--policy ordered] --stop-at-committed N
         [--native-args '--frontier-policy stop'] [--reference REF/result.json] [--ignore-top KEY ...]
         [--oracle --oracle-cpus C --oracle-threads T [--binding-from CKPT]]
Writes TMP/w1-ops/runs/<label>/<family>/{run1,run2,checkpoint,report.json}.
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("/common/dev/rustred")
WT = ROOT / ".claude/worktrees/agent-ade877816b107b1cf"
sys.path.insert(0, str(ROOT / "TMP/fable51-controls"))
import run_control as rc  # noqa: E402

COMMITTED = re.compile(rb'"committed_domains":\s*(\d+)')
OUT_ROOT = ROOT / "TMP/w1-ops/runs"


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def last_committed(events):
    try:
        size = events.stat().st_size
    except OSError:
        return 0
    with events.open("rb") as stream:
        stream.seek(max(0, size - 2_000_000))
        found = COMMITTED.findall(stream.read())
    return int(found[-1]) if found else 0


def read(path):
    try:
        return json.loads(Path(path).read_text())
    except (OSError, ValueError):
        return {}


def launch(argv, out, cpus, nice):
    env = dict(os.environ, **{**rc.ENV_ONE, "OMP_THREAD_LIMIT": "1"}, TMPDIR=str(ROOT / "TMP"), SYMBOLICA_HIDE_BANNER="1")
    out.mkdir(parents=True, exist_ok=True)
    (out / "argv.json").write_text(json.dumps(argv, indent=1))
    return subprocess.Popen(argv, cwd=ROOT, env=env, stdout=open(out / "stdout", "w"),
                            stderr=open(out / "stderr", "w"),
                            preexec_fn=lambda: (os.sched_setaffinity(0, cpus), os.nice(nice)))


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--first", required=True)
    p.add_argument("--second", required=True)
    p.add_argument("--family", required=True, choices=sorted(rc.COMMANDS))
    p.add_argument("--label", required=True)
    p.add_argument("--cpus", required=True)
    p.add_argument("--policy", default="ordered")
    p.add_argument("--workers", type=int)
    p.add_argument("--stop-at-committed", type=int, required=True)
    p.add_argument("--native-args", default="", help="extra native arguments, one shell-quoted string")
    p.add_argument("--reference", type=Path)
    p.add_argument("--ignore-top", nargs="*", default=["uncommitted_inspections"])
    p.add_argument("--nice", type=int, default=5)
    p.add_argument("--oracle", action="store_true")
    p.add_argument("--oracle-cpus")
    p.add_argument("--oracle-threads", type=int, default=16)
    p.add_argument("--binding-from", type=Path)
    args = p.parse_args()
    import shlex
    args.extra = shlex.split(args.native_args)
    out = OUT_ROOT / args.label / args.family
    if out.exists():
        sys.exit(f"refusing to overwrite {out}")
    out.mkdir(parents=True)
    cpus = rc.cpu_list(args.cpus)
    base = json.load(open(rc.COMMANDS[args.family]))
    base = list(base["command"] if isinstance(base, dict) else base)
    checkpoint = out / "checkpoint"
    report = {"family": args.family, "label": args.label, "cpus": args.cpus, "policy": args.policy,
              "workers": args.workers, "extra": args.extra, "stop_at_committed": args.stop_at_committed,
              "first": {"binary": args.first, "sha256": sha256(args.first)},
              "second": {"binary": args.second, "sha256": sha256(args.second)}, "family_closure_claim": False}
    argv1 = rc.rewrite(base, args.first, out / "run1", args.policy, args.workers, None, args.extra,
                       args.family == "five-finite")
    if "--checkpoint" not in argv1:
        argv1 += ["--checkpoint", str(checkpoint)]
    argv1[argv1.index("--checkpoint") + 1] = str(checkpoint)
    started = time.time()
    proc = launch(argv1, out / "run1", cpus, args.nice)
    stopped_at = None
    while proc.poll() is None:
        committed = last_committed(out / "run1" / "events.jsonl")
        if stopped_at is None and committed >= args.stop_at_committed:
            (out / "run1" / "stop-request.json").write_text(json.dumps({"reason": "pause_resume control"}))
            stopped_at = committed
        time.sleep(0.5)
    first = read(out / "run1" / "result.json")
    report["run1"] = {"exit_code": proc.returncode, "seconds": round(time.time() - started, 1),
                      "stop_requested_at_committed": stopped_at, "status": first.get("status"),
                      "committed_domains": first.get("committed_domains"), "frontiers": first.get("frontiers"),
                      "stop_reason": first.get("stop_reason")}
    paused = proc.returncode == 4 and first.get("status") == "paused" and stopped_at is not None
    report["run1_paused_mid_walk"] = paused
    if paused:
        argv2 = rc.rewrite(base, args.second, out / "run2", args.policy, args.workers, None, args.extra,
                           args.family == "five-finite")
        index = argv2.index("--checkpoint") if "--checkpoint" in argv2 else None
        if index is None:
            argv2 += ["--resume", str(checkpoint)]
        else:
            argv2[index:index + 2] = ["--resume", str(checkpoint)]
        started = time.time()
        proc = launch(argv2, out / "run2", cpus, args.nice)
        proc.wait()
        second = read(out / "run2" / "result.json")
        report["run2"] = {"exit_code": proc.returncode, "seconds": round(time.time() - started, 1),
                          "status": second.get("status"), "frontiers": second.get("frontiers"),
                          "completed_nodes": second.get("completed_nodes"),
                          "uncommitted_inspections": len(second.get("uncommitted_inspections") or [])}
        if args.reference:
            command = [sys.executable, str(WT / "examples/python/compare_walk_records.py"), "--mode", "strict",
                       str(args.reference), str(out / "run2" / "result.json"), "--output", str(out / "compare.json")]
            for key in args.ignore_top:
                command += ["--ignore-top", key]
            subprocess.run(command, capture_output=True, text=True,
                           preexec_fn=lambda: os.sched_setaffinity(0, cpus))
            compare = read(out / "compare.json")
            report["compare"] = {"verdict": compare.get("verdict"), "differing_records": compare.get("differing_records"),
                                 "top_level_differences": sorted(compare.get("top_level_differences") or {}),
                                 "ignored_top_level": args.ignore_top}
        if args.oracle:
            command = [sys.executable, str(WT / "tools/research/ops/oracle_check.py"), str(out / "run2"),
                       "--command", str(out / "run2" / "argv.json"), "--checkpoint", str(checkpoint),
                       "--out", str(out / "oracle"), "--cpus", args.oracle_cpus or args.cpus,
                       "--threads", str(args.oracle_threads)]
            if args.binding_from:
                command += ["--binding-from", str(args.binding_from)]
            completed = subprocess.run(command, capture_output=True, text=True)
            lines = completed.stdout.strip().splitlines()
            report["oracle"] = json.loads(lines[-1]) if lines else {"gate": "ERROR", "stderr": completed.stderr[-2000:]}
    ok = (paused and report.get("run2", {}).get("exit_code") == 0
          and (not args.reference or report["compare"]["verdict"] == "PASS")
          and (not args.oracle or report["oracle"].get("gate") == "PASS"))
    report["verdict"] = "PASS" if ok else "FAIL"
    (out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    print(json.dumps(report))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
