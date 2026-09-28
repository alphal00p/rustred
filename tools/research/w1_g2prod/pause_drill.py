#!/usr/bin/env python
"""Multi-pause resume drill (lane g2prod): one walk interrupted at several
committed-domain counts and resumed in fresh processes to exhaustion.

usage: pause_drill.py --command ARGV.json --binary BIN --out DIR --cpus LIST
                      --stops 250000,500000,750000 [--workers N] [--policy P]
                      [--extra ARG ...] [--resume-extra ARG ...]

Phase 0 runs the command (its --checkpoint replaced by DIR/checkpoint) and
touches the stop file once `committed_domains` in its events reaches the first
stop; every later phase replaces `--checkpoint` by `--resume` of the same
directory and stops at the next count; the last phase runs to exhaustion.
Every interrupted phase must exit 4 (cooperative saved pause) and every
resume must restore (a `checkpoint_restored` event). The final phase's
result.json holds every record (the checkpoint sidecar) and is what the
oracles judge. Writes DIR/phase-K/{command.json,result.json,events.jsonl,
stdout,stderr} and DIR/report.json.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path("/common/dev/rustred")
sys.path.insert(0, str(ROOT / "TMP/fable51-controls"))
import run_control as rc  # noqa: E402

COMMITTED = re.compile(rb'"committed_domains":\s*(\d+)')


def last_committed(events):
    try:
        size = events.stat().st_size
    except OSError:
        return 0
    with events.open("rb") as stream:
        stream.seek(max(0, size - 2_000_000))
        found = COMMITTED.findall(stream.read())
    return int(found[-1]) if found else 0


def set_option(argv, name, value):
    if name in argv:
        argv[argv.index(name) + 1] = str(value)
    else:
        argv.extend([name, str(value)])


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--command", required=True, type=Path)
    p.add_argument("--binary", required=True)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--cpus", required=True)
    p.add_argument("--stops", required=True)
    p.add_argument("--workers", type=int)
    p.add_argument("--policy")
    p.add_argument("--extra", nargs="*", default=[])
    p.add_argument("--resume-extra", action="append", default=[],
                   help="one native argument added to resumed phases only (repeatable; e.g. a G2' activation)")
    p.add_argument("--nice", type=int, default=5)
    p.add_argument("--phase-timeout", type=float, default=3000.0)
    args = p.parse_args()
    if args.out.exists():
        sys.exit(f"refusing to overwrite {args.out}")
    args.out.mkdir(parents=True)
    base = json.load(open(args.command))
    base[0] = args.binary
    for name in ("--checkpoint", "--resume"):
        if name in base:
            i = base.index(name)
            del base[i:i + 2]
    if args.workers:
        set_option(base, "--workers", args.workers)
    if args.policy:
        set_option(base, "--publication-policy", args.policy)
    base += args.extra
    checkpoint = args.out / "checkpoint"
    stops = [int(s) for s in args.stops.split(",") if s]
    cpus = rc.cpu_list(args.cpus)
    env = dict(os.environ)
    env.update(rc.ENV_ONE)
    env["TMPDIR"] = str(ROOT / "TMP")
    phases = []
    for index in range(len(stops) + 1):
        out = args.out / f"phase-{index}"
        out.mkdir()
        argv = list(base)
        set_option(argv, "--output", out / "result.json")
        set_option(argv, "--events", out / "events.jsonl")
        set_option(argv, "--stop-file", out / "stop-request.json")
        set_option(argv, "--resume" if index else "--checkpoint", checkpoint)
        if index:
            argv += args.resume_extra
        json.dump(argv, open(out / "command.json", "w"), indent=1)
        stop = stops[index] if index < len(stops) else None
        started = time.time()
        proc = subprocess.Popen(["nice", "-n", str(args.nice), "nix", "develop", str(ROOT), "--command"] + argv,
                                cwd=ROOT, env=env, stdout=open(out / "stdout", "w"),
                                stderr=open(out / "stderr", "w"), start_new_session=True,
                                preexec_fn=lambda: os.sched_setaffinity(0, cpus))
        requested_at = None
        while True:
            try:
                code = proc.wait(timeout=1.0)
                break
            except subprocess.TimeoutExpired:
                pass
            if stop is not None and requested_at is None and last_committed(out / "events.jsonl") >= stop:
                (out / "stop-request.json").write_text(json.dumps({"reason": "g2prod pause drill",
                                                                   "stop_at_committed": stop}))
                requested_at = time.time() - started
            if time.time() - started > args.phase_timeout:
                (out / "stop-request.json").write_text(json.dumps({"reason": "g2prod drill timeout"}))
                code = proc.wait()
                break
        text = (out / "events.jsonl").read_text() if (out / "events.jsonl").exists() else ""
        phase = {"phase": index, "exit_code": code, "stop_at_committed": stop,
                 "stop_requested_after_seconds": requested_at, "seconds": time.time() - started,
                 "restored": '"checkpoint_restored"' in text, "committed_at_end": last_committed(out / "events.jsonl")}
        phases.append(phase)
        print(json.dumps(phase), flush=True)
        expected = 4 if stop is not None else 0
        if code != expected or (index > 0 and not phase["restored"]):
            break
    ok = (len(phases) == len(stops) + 1
          and all(ph["exit_code"] == (4 if ph["stop_at_committed"] is not None else 0) for ph in phases)
          and all(ph["restored"] for ph in phases[1:]))
    report = {"binary": args.binary, "command": str(args.command), "stops": stops, "cpus": args.cpus,
              "phases": phases, "drained_after_resumes": ok, "final": str(args.out / f"phase-{len(stops)}")}
    (args.out / "report.json").write_text(json.dumps(report, indent=1) + "\n")
    print(json.dumps({"drained_after_resumes": ok}))
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
