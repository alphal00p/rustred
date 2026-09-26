#!/usr/bin/env python3
"""Run a matrix of matched owner-domain walk controls, sequentially, with receipts.

Each case is launched through `shared_owner_campaign.py` (so its RAM guard,
affinity pinning and receipts apply) under `nice -n N taskset -c CPUS`. Per
case the harness writes `command.json`, the supervisor `run/` directory and a
`summary.json` with the whole-command wall/CPU/max-RSS from the child's
resource usage, native preparation/traversal seconds, inspections by phase,
aliases, events, max scheduled finite rank, checkpoint seconds, peak sampled
RSS and exit status. `RESULTS.md` tabulates all cases and
`matrix-receipt.json` records executable and input SHA-256 digests. The
harness refuses to start when a requested CPU is outside this process's
affinity mask. All numbers are single-run measurements, not portable timings.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

MATRIX_SCHEMA = "rustred.walk-control-matrix.json.v1"
SUMMARY_SCHEMA = "rustred.walk-control-case-summary.v1"
RECEIPT_SCHEMA = "rustred.walk-control-matrix-receipt.v1"
CASE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")
REQUIRED_CASE_FIELDS = ("name", "executable", "manifest", "queries", "owner_base", "workers", "cpus",
                        "publication_policy", "inspection_workers", "native_options", "max_memory_bytes",
                        "checkpoint_interval_seconds")
RESULT_SCALARS = ("prepared_seconds", "traversal_seconds", "elapsed_seconds", "completed_nodes",
                  "scheduled_nodes", "committed_domains", "processed_nodes", "native_processed_nodes",
                  "routed_domains", "route_masks", "events", "committed_events", "successors",
                  "conditional_successors", "queued_nodes", "frontiers", "max_scheduled_finite_rank",
                  "unbounded_rank_domains", "partial_initial_inspections", "status", "schema", "workers",
                  "deduplication_hits", "containment_checks", "containment_maintenance_checks")
FULL_PARSE_LIMIT = 64 * 1024 * 1024
HEAD_BYTES = 8 * 1024 * 1024
TAIL_BYTES = 1024 * 1024


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


SUPERVISOR = module("shared_owner_campaign")
HEARTBEAT = module("heartbeat_metrics")
AUDIT = module("audit_owner_domain_walk")
MAX_WORKERS = SUPERVISOR.MAX_WORKERS


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def positive_int(value):
    return type(value) is int and value > 0


def load_matrix(path, affinity=None):
    """Validate the matrix document and every case before anything runs."""
    affinity = os.sched_getaffinity(0) if affinity is None else set(affinity)
    document = json.loads(Path(path).read_text())
    if not isinstance(document, dict) or document.get("schema") != MATRIX_SCHEMA:
        raise ValueError(f"matrix schema must be {MATRIX_SCHEMA}")
    cases = document.get("cases")
    if not isinstance(cases, list) or not cases:
        raise ValueError("matrix must list at least one case")
    base = Path(path).resolve().parent
    names = set()
    resolved = []
    for index, case in enumerate(cases):
        if not isinstance(case, dict):
            raise ValueError(f"case {index} must be an object")
        missing = [field for field in REQUIRED_CASE_FIELDS if field not in case]
        if missing:
            raise ValueError(f"case {index} lacks {missing}")
        name = case["name"]
        if not isinstance(name, str) or not CASE_NAME.match(name) or name in names:
            raise ValueError(f"case {index}: name must be unique and match {CASE_NAME.pattern}")
        names.add(name)
        row = dict(case)
        for field in ("executable", "manifest", "queries", "owner_base"):
            value = case[field]
            if not isinstance(value, str) or not value:
                raise ValueError(f"case {name}: {field} must be a path string")
            location = Path(value)
            location = location if location.is_absolute() else base / location
            row[field] = location.resolve()
        for field in ("executable", "manifest", "queries"):
            if not row[field].is_file():
                raise ValueError(f"case {name}: {field} is not a file: {row[field]}")
        if not os.access(row["executable"], os.X_OK):
            raise ValueError(f"case {name}: executable is not executable")
        if not row["owner_base"].is_dir():
            raise ValueError(f"case {name}: owner_base is not a directory")
        if not positive_int(case["workers"]) or case["workers"] > MAX_WORKERS:
            raise ValueError(f"case {name}: workers must be in 1..{MAX_WORKERS}")
        cpus = SUPERVISOR.parse_cpu_set(case["cpus"]) if isinstance(case["cpus"], str) else None
        if cpus is None or len(cpus) != case["workers"]:
            raise ValueError(f"case {name}: cpus must be a CPU list/range with exactly workers IDs")
        if not cpus <= affinity:
            raise ValueError(f"case {name}: CPUs {sorted(cpus - affinity)} are outside this process's affinity mask")
        row["cpu_set"] = cpus
        if case["publication_policy"] not in ("ordered", "ready"):
            raise ValueError(f"case {name}: publication_policy must be ordered or ready")
        inspectors = case["inspection_workers"]
        if inspectors is not None:
            available = 1 if case["workers"] == 1 else case["workers"] - 1
            if not positive_int(inspectors) or inspectors > available:
                raise ValueError(f"case {name}: inspection_workers must be in 1..{available} or null")
        options = case["native_options"]
        if not isinstance(options, list) or not all(isinstance(item, str) for item in options):
            raise ValueError(f"case {name}: native_options must be a list of strings")
        if not positive_int(case["max_memory_bytes"]) or not positive_int(case["checkpoint_interval_seconds"]):
            raise ValueError(f"case {name}: max_memory_bytes and checkpoint_interval_seconds must be positive integers")
        lookahead = case.get("transfer_unreserved_lookahead", 256)
        if not positive_int(lookahead):
            raise ValueError(f"case {name}: transfer_unreserved_lookahead must be a positive integer")
        row["transfer_unreserved_lookahead"] = lookahead
        margin = case.get("ram_guard_margin_percent", 5.0)
        if not isinstance(margin, (int, float)) or isinstance(margin, bool) or not 0 < margin < 100:
            raise ValueError(f"case {name}: ram_guard_margin_percent must be strictly between 0 and 100")
        row["ram_guard_margin_percent"] = margin
        resolved.append(row)
    return {"schema": MATRIX_SCHEMA, "cases": resolved, "description": document.get("description")}


def query_allowances(queries_path):
    document = json.loads(Path(queries_path).read_text())
    queries = document.get("queries") if isinstance(document, dict) else None
    if not isinstance(queries, list) or not queries:
        raise ValueError(f"{queries_path} is not a nonempty query document")
    return len(queries), Path(queries_path).stat().st_size


def case_command(case, case_dir, python, supervisor, nice_level, taskset="taskset", nice="nice"):
    count, size = query_allowances(case["queries"])
    command = [nice, "-n", str(nice_level), taskset, "-c", SUPERVISOR.format_cpu_set(case["cpu_set"]),
               python, str(supervisor),
               "--executable", str(case["executable"]), "--manifest", str(case["manifest"]),
               "--queries", str(case["queries"]), "--owner-base", str(case["owner_base"]),
               "--workers", str(case["workers"]), "--cpus", SUPERVISOR.format_cpu_set(case["cpu_set"]),
               "--max-memory-bytes", str(case["max_memory_bytes"]),
               "--ram-guard-margin-percent", str(case["ram_guard_margin_percent"]),
               "--unbounded-work", "--max-queries", str(count), "--max-query-bytes", str(size),
               "--bounded-refinement-axes", "finite-axes", "--max-guard-univariate-degree", "64",
               "--publication-policy", case["publication_policy"], "--route-domain-overcover",
               "--transfer-unreserved-lookahead", str(case["transfer_unreserved_lookahead"]),
               "--reuse-initial-d-bands",
               "--checkpoint", str(case_dir / "checkpoint"),
               "--checkpoint-interval-seconds", str(case["checkpoint_interval_seconds"])]
    if case["inspection_workers"] is not None:
        command += ["--inspection-workers", str(case["inspection_workers"])]
    command += list(case["native_options"])
    command += ["--run-directory", str(case_dir / "run"), "--no-progress"]
    return command


def scan_result_scalars(path, keys=RESULT_SCALARS, full_parse_limit=FULL_PARSE_LIMIT):
    """Top-level scalar fields of result.json; head/tail regex scan when the file is huge."""
    path = Path(path)
    if not path.is_file():
        return {"present": False}
    size = path.stat().st_size
    result = {"present": True, "bytes": size}
    if size <= full_parse_limit:
        try:
            document = json.loads(path.read_text())
        except ValueError as error:
            result["parse_error"] = str(error)
            return result
        result["complete_parse"] = True
        for key in keys:
            if key in document and not isinstance(document[key], (dict, list)):
                result[key] = document[key]
        return result
    result["complete_parse"] = False
    with path.open("rb") as stream:
        head = stream.read(min(size, HEAD_BYTES)).decode("utf-8", "replace")
        stream.seek(max(0, size - TAIL_BYTES))
        tail = stream.read().decode("utf-8", "replace")
    for key in keys:
        pattern = re.compile(r'^  "%s": (-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?|null|true|false|"[^"\n]*"),?$' % re.escape(key), re.M)
        for blob in (head, tail):
            match = pattern.search(blob)
            if match:
                result[key] = json.loads(match.group(1))
                break
    return result


def summarize_events(events_path):
    """Checkpoint saves and derived heartbeat metrics over the whole session."""
    path = Path(events_path)
    if not path.is_file():
        return {"present": False}
    derived = HEARTBEAT.compute(path)
    return {"present": True, "checkpoint_saves": derived["checkpoint_saves_counted"],
            "checkpoint_seconds": derived["checkpoint_save_seconds"],
            "last_checkpoint": derived["last_checkpoint"], "derived": derived}


def peak_sampled_rss(resources_path):
    path = Path(resources_path)
    if not path.is_file():
        return None
    peak = None
    for record, partial, invalid in HEARTBEAT.stream_events(path):
        if record is None:
            continue
        value = HEARTBEAT.nonnegative_int(record.get("peak_observed_rss_bytes"))
        if value is None:
            value = HEARTBEAT.nonnegative_int(record.get("aggregate_rss_bytes"))
        if value is not None:
            peak = value if peak is None else max(peak, value)
    return peak


def run_case(case, case_dir, command, env=None, audit=False):
    case_dir.mkdir(parents=True, exist_ok=False)
    (case_dir / "command.json").write_text(json.dumps(command, indent=1) + "\n")
    env = dict(os.environ if env is None else env)
    env.setdefault("SYMBOLICA_HIDE_BANNER", "1")
    started_unix = time.time()
    started = time.monotonic()
    interrupted = None
    with (case_dir / "stdout").open("wb") as stdout, (case_dir / "stderr").open("wb") as stderr:
        child = subprocess.Popen(command, cwd=case_dir, env=env, stdout=stdout, stderr=stderr)
        while True:
            try:
                _, status, usage = os.wait4(child.pid, 0)
                break
            except KeyboardInterrupt:
                # Forward the operator stop once; the supervisor saves and exits.
                interrupted = "operator_interrupt"
                child.send_signal(signal.SIGINT)
            except ChildProcessError:
                status, usage = child.wait(), None
                break
    wall = time.monotonic() - started
    exit_status = os.waitstatus_to_exitcode(status) if usage is not None else status
    child.returncode = exit_status
    run = case_dir / "run"
    receipt_path = run / "supervisor-result.json"
    receipt = json.loads(receipt_path.read_text()) if receipt_path.is_file() else None
    summary = {
        "schema": SUMMARY_SCHEMA, "name": case["name"], "case_directory": str(case_dir),
        "publication_policy": case["publication_policy"], "workers": case["workers"],
        "cpus": SUPERVISOR.format_cpu_set(case["cpu_set"]), "inspection_workers": case["inspection_workers"],
        "native_options": list(case["native_options"]),
        "started_unix_time": started_unix, "finished_unix_time": time.time(),
        "exit_status": exit_status, "interrupted": interrupted,
        "whole_command": {
            "wall_seconds": wall,
            "user_seconds": None if usage is None else usage.ru_utime,
            "system_seconds": None if usage is None else usage.ru_stime,
            "cpu_seconds": None if usage is None else usage.ru_utime + usage.ru_stime,
            "max_rss_kib": None if usage is None else usage.ru_maxrss,
            "scope": "wait4 of the supervisor process including its waited-for native child",
        },
        "supervisor": None if receipt is None else {key: receipt.get(key) for key in (
            "exit_status", "state", "elapsed_seconds", "peak_observed_aggregate_rss_bytes",
            "operator_or_resource_stop", "hard_stopped", "effective_hard_memory_bytes",
            "effective_soft_memory_bytes", "rust_result_present")},
        "result": scan_result_scalars(run / "result.json"),
        "events": summarize_events(run / "events.jsonl"),
        "peak_sampled_rss_bytes": peak_sampled_rss(run / "resources.jsonl"),
        "inputs": {"executable_sha256": digest(case["executable"]), "manifest_sha256": digest(case["manifest"]),
                   "queries_sha256": digest(case["queries"])},
        "single_run_measurement": True, "family_closure_claim": False,
    }
    result = summary["result"]
    completed, routed, scheduled = result.get("completed_nodes"), result.get("routed_domains"), result.get("scheduled_nodes")
    summary["native_inspections"] = completed
    summary["native_by_phase"] = (None if completed is None or routed is None
                                  else {"Apply": completed - routed, "Route": routed})
    summary["aliases"] = None if completed is None or scheduled is None else scheduled - completed
    if audit:
        report = AUDIT.audit_walk(run)
        (case_dir / "audit.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        summary["audit"] = {"verdict": report["audit"], "violations": report["violations"][:20],
                            "native_by_phase": report.get("native_by_phase"), "aliases": report.get("aliases")}
        if report["audit"] == "PASS":
            summary["native_by_phase"] = report.get("native_by_phase")
            summary["aliases"] = report.get("aliases")
    (case_dir / "summary.json").write_text(json.dumps(summary, indent=2, sort_keys=True, allow_nan=False) + "\n")
    return summary


def fmt(value, digits=1):
    if value is None:
        return "n/a"
    if isinstance(value, float):
        return f"{value:,.{digits}f}"
    if isinstance(value, int):
        return f"{value:,}"
    return str(value)


def results_table(summaries):
    header = ("| case | policy | W | CPUs | exit | whole wall s | whole CPU s | max RSS GiB | prep s | traversal s "
              "| native Apply/Route | aliases | events | max rank | checkpoint s | peak sampled RSS GB | audit |")
    rows = [header, "|" + "---|" * (header.count("|") - 1)]
    for summary in summaries:
        whole = summary["whole_command"]
        result = summary["result"]
        events = summary["events"]
        phase = summary.get("native_by_phase") or {}
        max_rss = whole.get("max_rss_kib")
        peak = summary.get("peak_sampled_rss_bytes")
        audit = summary.get("audit")
        rows.append("| " + " | ".join([
            summary["name"], summary["publication_policy"], str(summary["workers"]), summary["cpus"],
            str(summary["exit_status"]), fmt(whole.get("wall_seconds")), fmt(whole.get("cpu_seconds")),
            "n/a" if max_rss is None else f"{max_rss / 2 ** 20:.2f}",
            fmt(result.get("prepared_seconds"), 2), fmt(result.get("traversal_seconds"), 2),
            f"{fmt(phase.get('Apply'))} / {fmt(phase.get('Route'))}", fmt(summary.get("aliases")),
            fmt(result.get("events")), fmt(result.get("max_scheduled_finite_rank")),
            fmt(events.get("checkpoint_seconds"), 2), "n/a" if peak is None else f"{peak / 1e9:.2f}",
            "not run" if audit is None else audit["verdict"]]) + " |")
    return "\n".join(rows) + "\n"


def write_results(output, matrix_path, summaries, receipt):
    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
    text = ["# Walk control matrix results", "",
            f"Matrix `{matrix_path}` (sha256 {receipt['matrix_sha256']}), finished {stamp}.",
            "Single-run measurements on the listed CPUs via `shared_owner_campaign.py`; not portable timings",
            "and not a closure or termination claim. Whole-command numbers come from the supervisor's",
            "`wait4` resource usage; preparation/traversal seconds are the native report's own timers.", "",
            results_table(summaries), "",
            "Per-case receipts: `<case>/command.json`, `<case>/run/`, `<case>/summary.json`.", ""]
    (output / "RESULTS.md").write_text("\n".join(text))
    (output / "matrix-receipt.json").write_text(json.dumps(receipt, indent=2, sort_keys=True, allow_nan=False) + "\n")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("matrix", type=Path, help=f"matrix JSON ({MATRIX_SCHEMA})")
    parser.add_argument("--output", type=Path, required=True, help="matrix output directory (created; cases must not exist)")
    parser.add_argument("--case", action="append", default=[], metavar="NAME", help="run only these cases; repeatable")
    parser.add_argument("--nice", type=int, default=5, help="nice level for every case (default 5)")
    parser.add_argument("--python", default=sys.executable, help="interpreter for the supervisor")
    parser.add_argument("--supervisor", type=Path, default=Path(__file__).with_name("shared_owner_campaign.py"))
    parser.add_argument("--audit", action="store_true", help="stream-audit each result.json after the run")
    parser.add_argument("--skip-existing", action="store_true", help="skip cases whose summary.json already exists")
    parser.add_argument("--dry-run", action="store_true", help="validate and write command.json only; run nothing")
    args = parser.parse_args(argv)
    try:
        matrix = load_matrix(args.matrix)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    taskset, nice = shutil.which("taskset"), shutil.which("nice")
    if taskset is None or nice is None:
        parser.error("taskset and nice must be on PATH")
    cases = matrix["cases"]
    if args.case:
        unknown = set(args.case) - {case["name"] for case in cases}
        if unknown:
            parser.error(f"unknown cases: {sorted(unknown)}")
        cases = [case for case in cases if case["name"] in args.case]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    supervisor = args.supervisor.resolve()
    receipt = {"schema": RECEIPT_SCHEMA, "matrix_path": str(args.matrix.resolve()),
               "matrix_sha256": digest(args.matrix), "harness_sha256": digest(Path(__file__)),
               "supervisor_sha256": digest(supervisor), "python": args.python, "nice": args.nice,
               "taskset": taskset, "affinity": sorted(os.sched_getaffinity(0)),
               "started_unix_time": time.time(), "audit_requested": args.audit, "dry_run": args.dry_run,
               "cases": [], "single_run_measurements": True, "family_closure_claim": False}
    summaries = []
    for case in cases:
        case_dir = output / case["name"]
        command = case_command(case, case_dir, args.python, supervisor, args.nice, taskset, nice)
        entry = {"name": case["name"], "executable": str(case["executable"]),
                 "executable_sha256": digest(case["executable"]), "manifest_sha256": digest(case["manifest"]),
                 "queries_sha256": digest(case["queries"]), "owner_base": str(case["owner_base"]),
                 "cpus": SUPERVISOR.format_cpu_set(case["cpu_set"]), "command": command}
        if args.skip_existing and (case_dir / "summary.json").is_file():
            entry["status"] = "skipped_existing"
            summaries.append(json.loads((case_dir / "summary.json").read_text()))
        elif args.dry_run:
            case_dir.mkdir(parents=True, exist_ok=True)
            (case_dir / "command.json").write_text(json.dumps(command, indent=1) + "\n")
            entry["status"] = "dry_run"
            print(" ".join(command))
        else:
            if case_dir.exists():
                parser.error(f"case directory exists: {case_dir} (use --skip-existing or a new --output)")
            print(f"[{case['name']}] {' '.join(command)}", flush=True)
            summary = run_case(case, case_dir, command, audit=args.audit)
            summaries.append(summary)
            entry["status"] = "completed" if summary["exit_status"] == 0 else f"exit_{summary['exit_status']}"
            entry["summary"] = str(case_dir / "summary.json")
            print(f"[{case['name']}] exit {summary['exit_status']} · wall {summary['whole_command']['wall_seconds']:.1f} s"
                  f" · native {summary.get('native_inspections')} · max rank {summary['result'].get('max_scheduled_finite_rank')}", flush=True)
        receipt["cases"].append(entry)
    receipt["finished_unix_time"] = time.time()
    if not args.dry_run:
        write_results(output, args.matrix, summaries, receipt)
    else:
        (output / "matrix-receipt.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    return 0 if all(summary["exit_status"] == 0 for summary in summaries) else 1


if __name__ == "__main__":
    raise SystemExit(main())
