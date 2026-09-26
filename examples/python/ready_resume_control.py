#!/usr/bin/env python3
"""Fresh-process Ready multi-prefix resume-to-exhaustion gate for the saved-owner walk.

Three native processes run one owner-domain-match command (an argv JSON list,
as recorded by the controls) under the Ready publication policy:

1. baseline  uninterrupted, to exhaustion;
2. paused    with RUSTRED_WALK_DIAGNOSTIC_PAUSE=ready-multi-prefix: the walk
             force-saves and pauses (exit 4) the first time its state holds
             at least two unfinished accepted source prefixes plus a finished
             hole beyond the contiguous publication watermark;
3. resumed   `--resume` of that checkpoint in a new process, without the
             variable, to exhaustion.

Pass criteria (design `fable51_design_scheduler_admission_2026-09-26.md`
section 1(a)): the paused receipt, its checkpoint manifest and the journaled
`diagnostic_pause` event show the label, `ready_accepted_source_prefixes >= 2`
and `ready_published_holes > 0`; the streaming audit
(`audit_owner_domain_walk.py`) passes on the resumed and baseline runs (zero
pending, zero frontiers, all inputs published, all ledger obligations
discharged); native inspections and committed events of the resumed run are
within the relative tolerance (default 2%) of the baseline. Ready admission
order depends on readiness, so exact equality is not a criterion here; the
in-process test `ready_multi_inspector_multi_prefix_disk_resume_matches_gated_baseline`
supplies exact equality under a fixed schedule.

If the walk drains before the trigger fires, the paused process exits 0: the
harness then reports `trigger_fired: false` with the largest prefix and hole
counts seen in the journaled heartbeats, and FAILS (a smaller lookahead or a
different worker count may be tried). Nothing here certifies family closure.

Writes OUT/{baseline,paused,resumed}/{command.json,result.json,events.jsonl,
stdout,stderr,audit.json} and OUT/report.json; refuses an existing OUT.

`--baseline DIR` reuses the baseline phase of an earlier run of this
harness (DIR = OLDOUT/baseline): its argv must equal this request apart from
transport options, and OLDOUT/report.json must record a 0 exit and the
SHA-256 of the binary under test. Anything else is refused unless
`--allow-foreign-baseline` is given; the verdict then cannot PASS. The reused
baseline's audit is written to OUT/baseline-audit.json, never into DIR.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time

SCHEMA = "rustred.ready-resume-control.v1"
PAUSE_VARIABLE = "RUSTRED_WALK_DIAGNOSTIC_PAUSE"
PAUSE_LABEL = "ready-multi-prefix"
PAUSED_EXIT = 4
# The historical controls pin every native inner pool to one thread.
ENV_ONE = {"RAYON_NUM_THREADS": "1", "OMP_NUM_THREADS": "1", "OMP_THREAD_LIMIT": "1",
           "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1", "BLIS_NUM_THREADS": "1"}
PHASES = ("baseline", "paused", "resumed")
# Options that name where a phase writes, not what it computes (the CP5 store
# binding excludes the checkpoint location, interval and resume mode too).
TRANSPORT_OPTIONS = ("--output", "--events", "--stop-file", "--checkpoint", "--resume",
                     "--checkpoint-interval-seconds")
TRANSPORT_FLAGS = ("--no-progress",)


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


AUDIT = module("audit_owner_domain_walk")
SUPERVISOR = module("shared_owner_campaign")


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def set_option(argv, name, value):
    """Replace the value of `name` or append the pair."""
    if name in argv:
        argv[argv.index(name) + 1] = str(value)
    else:
        argv.extend([name, str(value)])


def rewrite_command(argv, binary, run_directory, checkpoint, resume=False, workers=None,
                    lookahead=None, inspection_workers=None, replacements=()):
    """The control argv for one phase: same request, new executable and paths, Ready policy.

    `checkpoint` becomes `--checkpoint DIR`, or `--resume DIR` when `resume`;
    the request binding (workers, lookahead, inputs) must be identical for the
    paused and resumed phases, so only transport options differ between them.
    """
    if not isinstance(argv, list) or len(argv) < 2 or not all(isinstance(item, str) for item in argv):
        raise ValueError("command must be a JSON list of strings")
    if argv[1] != "owner-domain-match":
        raise ValueError("command must run owner-domain-match")
    if "--follow-successors" not in argv:
        raise ValueError("command must follow successors (a walk, not a matching diagnostic)")
    argv = list(argv)
    for old, new in replacements:
        argv = [item.replace(old, new) for item in argv]
    argv[0] = str(binary)
    run_directory = Path(run_directory)
    set_option(argv, "--output", run_directory / "result.json")
    set_option(argv, "--events", run_directory / "events.jsonl")
    set_option(argv, "--stop-file", run_directory / "stop-request.json")
    set_option(argv, "--publication-policy", "ready")
    if workers is not None:
        set_option(argv, "--workers", workers)
    if lookahead is not None:
        set_option(argv, "--transfer-unreserved-lookahead", lookahead)
    if inspection_workers is not None:
        set_option(argv, "--inspection-workers", inspection_workers)
    for flag in ("--checkpoint", "--resume"):
        if flag in argv:
            index = argv.index(flag)
            del argv[index:index + 2]
    argv.extend(["--resume" if resume else "--checkpoint", str(checkpoint)])
    if "--checkpoint-interval-seconds" not in argv:
        argv.extend(["--checkpoint-interval-seconds", "3600"])
    if "--no-progress" not in argv:
        argv.append("--no-progress")
    return argv


def request_argv(argv):
    """The argv without its executable and transport options: what binds the request."""
    kept, skip = [], False
    for item in argv[1:]:
        if skip:
            skip = False
        elif item in TRANSPORT_OPTIONS:
            skip = True
        elif item not in TRANSPORT_FLAGS:
            kept.append(item)
    return kept


def baseline_provenance(directory, base, options, tested_sha256):
    """What is known about a reused baseline directory, and every reason it cannot stand in.

    The digest and exit status are taken only from the report this harness
    wrote when it ran that baseline: hashing the recorded executable path now
    could hash a binary rebuilt since.
    """
    directory = Path(directory)
    recorded = read_json(directory / "command.json")
    provenance = {"reused": str(directory), "command": recorded, "binary_sha256": None, "exit": None}
    problems = []
    if not (isinstance(recorded, list) and len(recorded) >= 2 and all(isinstance(item, str) for item in recorded)):
        problems.append("baseline command.json is missing or not an argv list")
    else:
        try:
            expected = rewrite_command(base, recorded[0], directory, directory / "checkpoint", **options)
        except ValueError as error:
            problems.append(f"baseline argv cannot be rebuilt: {error}")
        else:
            if request_argv(recorded) != request_argv(expected):
                problems.append("baseline argv differs from this request")
    parent = read_json(directory.parent / "report.json")
    parent = parent if isinstance(parent, dict) else {}
    phase = parent.get("phases", {}).get("baseline") if isinstance(parent.get("phases"), dict) else None
    if (parent.get("schema") == SCHEMA and directory.name == "baseline" and isinstance(phase, dict)
            and "reused" not in phase):
        provenance["binary_sha256"] = parent.get("binary_sha256")
        provenance["exit"] = phase.get("exit")
    if provenance["binary_sha256"] is None:
        problems.append("no binary digest was recorded when the baseline ran")
    elif provenance["binary_sha256"] != tested_sha256:
        problems.append("baseline binary differs from the binary under test")
    if provenance["exit"] != 0:
        problems.append("baseline exit status is not a recorded 0")
    provenance["problems"] = problems
    return provenance


def journal(path, names):
    """Records of the named events (top-level or heartbeat progress) in one events.jsonl."""
    found = {name: [] for name in names}
    path = Path(path)
    if not path.is_file():
        return found
    with path.open("rb") as stream:
        for line in stream:
            if not any(name.encode() in line for name in names):
                continue
            try:
                record = json.loads(line)
            except ValueError:
                continue
            if not isinstance(record, dict):
                continue
            name = record.get("event")
            if name == "heartbeat":
                progress = record.get("progress")
                name = progress.get("event") if isinstance(progress, dict) else None
                record = progress if isinstance(progress, dict) else {}
            if name in found:
                found[name].append(record)
    return found


def ready_maxima(events_path):
    """Largest Ready prefix/hole/context counts over journaled heartbeats (sampled, ~1 s)."""
    maxima = {"heartbeats": 0, "ready_accepted_source_prefixes": None,
              "ready_published_holes": None, "ready_stream_contexts": None}
    path = Path(events_path)
    if not path.is_file():
        return maxima
    with path.open("rb") as stream:
        for line in stream:
            if b'"heartbeat"' not in line:
                continue
            try:
                record = json.loads(line)
            except ValueError:
                continue
            progress = record.get("progress") if isinstance(record, dict) else None
            if record.get("event") != "heartbeat" or not isinstance(progress, dict):
                continue
            maxima["heartbeats"] += 1
            for key in ("ready_accepted_source_prefixes", "ready_published_holes", "ready_stream_contexts"):
                value = progress.get(key)
                if type(value) is int and (maxima[key] is None or value > maxima[key]):
                    maxima[key] = value
    return maxima


def pause_evidence(receipt, manifest, pause_events):
    """Checks on the paused phase: receipt, durable manifest metadata, journaled trigger."""
    receipt = receipt if isinstance(receipt, dict) else {}
    metadata = manifest.get("metadata") if isinstance(manifest, dict) else None
    metadata = metadata if isinstance(metadata, dict) else {}
    checkpoint = receipt.get("checkpoint") if isinstance(receipt.get("checkpoint"), dict) else {}
    trigger = pause_events[-1] if pause_events else {}
    prefixes = receipt.get("ready_accepted_source_prefixes")
    holes = receipt.get("ready_published_holes")
    checks = {
        "receipt_paused": receipt.get("status") == "paused" and receipt.get("resume_supported") is True,
        "receipt_label": checkpoint.get("diagnostic_pause") == PAUSE_LABEL,
        "receipt_two_prefixes": type(prefixes) is int and prefixes >= 2,
        "receipt_hole": type(holes) is int and holes > 0,
        "manifest_label": metadata.get("diagnostic_pause") == PAUSE_LABEL,
        "manifest_paused": metadata.get("paused") is True and manifest.get("kind") == "state",
        "trigger_journaled": len(pause_events) == 1,
        "trigger_two_prefixes": type(trigger.get("ready_accepted_source_prefixes")) is int
        and trigger["ready_accepted_source_prefixes"] >= 2,
        "trigger_hole": type(trigger.get("ready_published_holes")) is int and trigger["ready_published_holes"] > 0,
        "family_closure_claim_false": receipt.get("family_closure_claim") is False,
    }
    return {"checks": checks, "passed": all(checks.values()),
            "ready_accepted_source_prefixes": prefixes, "ready_published_holes": holes,
            "ready_stream_contexts": receipt.get("ready_stream_contexts"),
            "committed_domains": receipt.get("committed_domains"),
            "contiguous_publication_watermark": receipt.get("contiguous_publication_watermark"),
            "completed_nodes": receipt.get("completed_nodes"), "events": receipt.get("events"),
            "checkpoint_generation": metadata.get("generation"), "checkpoint_bytes": metadata.get("bytes"),
            "trigger": {key: trigger.get(key) for key in
                        ("ready_accepted_source_prefixes", "ready_published_holes", "ready_stream_contexts",
                         "committed_domains", "contiguous_publication_watermark", "completed_native_inspections",
                         "committed_events")} if trigger else None}


def relative_difference(baseline, value):
    if type(baseline) is not int or type(value) is not int or baseline <= 0:
        return None
    return abs(value - baseline) / baseline


def compare_counts(baseline_audit, resumed_audit, tolerance):
    """Native inspections and committed events of the resumed run versus the baseline."""
    rows = {}
    for key in ("native_inspections", "events", "logical_records", "aliases"):
        base = baseline_audit.get(key)
        value = resumed_audit.get(key)
        difference = relative_difference(base, value)
        rows[key] = {"baseline": base, "resumed": value, "relative_difference": difference}
    gated = ("native_inspections", "events")
    passed = all(rows[key]["relative_difference"] is not None
                 and rows[key]["relative_difference"] <= tolerance for key in gated)
    return {"tolerance": tolerance, "gated": list(gated), "rows": rows, "passed": passed}


def verdict(phases, pause, audits, counts):
    """Overall PASS only when every stage met its criterion; reasons otherwise."""
    reasons = []
    baseline = phases.get("baseline", {})
    if baseline.get("problems"):
        reasons.append("reused baseline unverified: " + "; ".join(baseline["problems"]))
    elif baseline.get("exit") != 0:
        reasons.append("baseline did not exit 0")
    if audits.get("baseline") is not None and audits["baseline"].get("publication_policy") != "ready":
        reasons.append("baseline is not a Ready walk")
    paused_exit = phases.get("paused", {}).get("exit")
    if paused_exit == 0:
        reasons.append("trigger never fired: the paused run drained to exhaustion")
    elif paused_exit != PAUSED_EXIT:
        reasons.append(f"paused run exit {paused_exit} != {PAUSED_EXIT}")
    if pause is not None and not pause["passed"]:
        failed = sorted(name for name, ok in pause["checks"].items() if not ok)
        reasons.append("pause evidence failed: " + ", ".join(failed))
    if paused_exit == PAUSED_EXIT and phases.get("resumed", {}).get("exit") != 0:
        reasons.append("resumed run did not exit 0")
    for name, audit in audits.items():
        if audit is not None and audit.get("audit") != "PASS":
            reasons.append(f"{name} audit failed")
    if counts is not None and not counts["passed"]:
        reasons.append(f"native inspections/events outside the {counts['tolerance']:.0%} tolerance")
    if paused_exit == PAUSED_EXIT and (audits.get("resumed") is None or counts is None):
        reasons.append("resumed run was not audited")
    return ("PASS" if not reasons else "FAIL"), reasons


def run_phase(argv, directory, cpus, pause):
    directory.mkdir(parents=True)
    (directory / "command.json").write_text(json.dumps(argv, indent=1) + "\n")
    env = dict(os.environ)
    env.update(ENV_ONE)
    env.pop(PAUSE_VARIABLE, None)
    if pause:
        env[PAUSE_VARIABLE] = PAUSE_LABEL
    started = time.monotonic()
    with (directory / "stdout").open("w") as stdout, (directory / "stderr").open("w") as stderr:
        process = subprocess.run(argv, env=env, stdout=stdout, stderr=stderr,
                                 preexec_fn=lambda: os.sched_setaffinity(0, cpus))
    return {"exit": process.returncode, "whole_command_seconds": round(time.monotonic() - started, 3),
            "diagnostic_pause_variable": PAUSE_LABEL if pause else None}


def read_json(path):
    path = Path(path)
    return json.loads(path.read_text()) if path.is_file() else None


def audit(directory, output=None):
    report = AUDIT.audit_walk(directory)
    output = output or directory / "audit.json"
    output.write_text(json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n")
    return report


def summary(report):
    if report is None:
        return None
    return {key: report.get(key) for key in
            ("audit", "publication_policy", "native_inspections", "logical_records", "aliases", "events",
             "violations", "result_sha256", "prepared_seconds", "traversal_seconds", "native_session_seconds")}


def parse_replacement(text):
    old, separator, new = text.partition("=")
    if not separator or not old:
        raise argparse.ArgumentTypeError("replacement must be OLD=NEW")
    return old, new


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--command", required=True, type=Path, help="control argv JSON (owner-domain-match)")
    parser.add_argument("--binary", required=True, type=Path, help="rustred executable under test")
    parser.add_argument("--output", required=True, type=Path, help="new directory for all phases")
    parser.add_argument("--cpus", required=True, help="taskset-style CPU list, e.g. 206-211")
    parser.add_argument("--workers", type=int, help="override --workers (default: the command's)")
    parser.add_argument("--lookahead", type=int, help="override --transfer-unreserved-lookahead")
    parser.add_argument("--inspection-workers", type=int, help="override the automatic Ready split")
    parser.add_argument("--replace", action="append", default=[], type=parse_replacement,
                        help="OLD=NEW substring replacement in the argv (e.g. relocated inputs)")
    parser.add_argument("--tolerance", type=float, default=0.02, help="relative tolerance (default 0.02)")
    parser.add_argument("--baseline", type=Path,
                        help="reuse OLDOUT/baseline of an earlier run of this harness (same argv and binary)")
    parser.add_argument("--allow-foreign-baseline", action="store_true",
                        help="run against a --baseline that fails those checks; the verdict cannot PASS")
    args = parser.parse_args(argv)
    cpus = SUPERVISOR.parse_cpu_set(args.cpus)
    if not cpus <= os.sched_getaffinity(0):
        parser.error(f"CPUs {sorted(cpus - os.sched_getaffinity(0))} are outside this process's affinity mask")
    if not os.access(args.binary, os.X_OK):
        parser.error("binary is not executable")
    if not 0 < args.tolerance < 1:
        parser.error("tolerance must be in (0, 1)")
    if args.output.exists():
        parser.error(f"refusing to overwrite {args.output}")
    if args.allow_foreign_baseline and args.baseline is None:
        parser.error("--allow-foreign-baseline needs --baseline")
    base = json.loads(args.command.read_text())
    options = dict(workers=args.workers, lookahead=args.lookahead,
                   inspection_workers=args.inspection_workers, replacements=args.replace)
    binary_sha256 = digest(args.binary)
    reused = None
    if args.baseline is not None:
        reused = baseline_provenance(args.baseline.resolve(), base, options, binary_sha256)
        if reused["problems"] and not args.allow_foreign_baseline:
            parser.error("refusing --baseline: " + "; ".join(reused["problems"])
                         + " (--allow-foreign-baseline runs anyway, without a PASS verdict)")
    out = args.output.resolve()
    out.mkdir(parents=True)
    report = {"schema": SCHEMA, "binary": str(args.binary.resolve()), "binary_sha256": binary_sha256,
              "command_source": str(args.command.resolve()), "cpus": SUPERVISOR.format_cpu_set(cpus),
              "tolerance": args.tolerance, "family_closure_claim": False, "phases": {},
              "scope": "fresh-process pause/resume of one Ready walk; audited local completion only"}
    phases = report["phases"]
    audits = {"baseline": None, "resumed": None}

    def save():
        (out / "report.json").write_text(json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n")

    if reused is not None:
        baseline_dir = Path(reused["reused"])
        phases["baseline"] = reused
        save()
        if (baseline_dir / "result.json").is_file():
            audits["baseline"] = audit(baseline_dir, out / "baseline-audit.json")
    else:
        baseline_dir = out / "baseline"
        command = rewrite_command(base, args.binary.resolve(), baseline_dir, baseline_dir / "checkpoint", **options)
        phases["baseline"] = run_phase(command, baseline_dir, cpus, pause=False)
        save()
        if phases["baseline"]["exit"] == 0:
            audits["baseline"] = audit(baseline_dir)
    paused_dir, resumed_dir = out / "paused", out / "resumed"
    checkpoint = paused_dir / "checkpoint"
    command = rewrite_command(base, args.binary.resolve(), paused_dir, checkpoint, **options)
    phases["paused"] = run_phase(command, paused_dir, cpus, pause=True)
    save()
    pause = None
    if phases["paused"]["exit"] == PAUSED_EXIT:
        pause = pause_evidence(read_json(paused_dir / "result.json"), read_json(checkpoint / "latest.json"),
                               journal(paused_dir / "events.jsonl", ("diagnostic_pause",))["diagnostic_pause"])
        report["pause"] = pause
        command = rewrite_command(base, args.binary.resolve(), resumed_dir, checkpoint, resume=True, **options)
        phases["resumed"] = run_phase(command, resumed_dir, cpus, pause=False)
        restored = journal(resumed_dir / "events.jsonl", ("checkpoint_restored",))["checkpoint_restored"]
        report["restore"] = restored[-1].get("restore") if restored else None
        if phases["resumed"]["exit"] == 0:
            audits["resumed"] = audit(resumed_dir)
    report["trigger_fired"] = pause is not None
    report["paused_run_ready_maxima"] = ready_maxima(paused_dir / "events.jsonl")
    counts = None
    if audits["baseline"] is not None and audits["resumed"] is not None:
        counts = compare_counts(audits["baseline"], audits["resumed"], args.tolerance)
        report["counts"] = counts
    report["audits"] = {name: summary(value) for name, value in audits.items()}
    report["verdict"], report["reasons"] = verdict(phases, pause, audits, counts)
    save()
    print(json.dumps({key: report.get(key) for key in
                      ("verdict", "reasons", "binary_sha256", "trigger_fired", "phases", "counts")},
                     indent=1, sort_keys=True))
    return 0 if report["verdict"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
