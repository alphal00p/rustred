#!/usr/bin/env python3
"""Compare full/bootstrap/reused publication; never modify the source checkpoint.

Run inside the development environment, using release executables. All outputs
must go to a fresh directory. The warm case repeats the same scope; test actual
rank extensions separately with the public saved_campaign.py lifecycle.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import time


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run(args, name, executable, previous=None):
    output = args.directory / name
    output.mkdir()
    command = ["taskset", "-c", args.cpus, str(executable), "walk-publish",
               "--command", str(args.command), "--checkpoint", str(args.checkpoint),
               "--directory", str(output / "artifact"), "--no-deep-verification",
               "--threads", str(args.threads), "--events", str(output / "events.jsonl")]
    if previous is not None:
        command += ["--previous-artifact", str(previous)]
    environment = dict(os.environ, SYMBOLICA_HIDE_BANNER="1", TMPDIR=str(args.directory))
    for key in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "OMP_THREAD_LIMIT",
                "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "BLIS_NUM_THREADS"):
        environment[key] = "1"
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    peak_rss_kib = 0
    with (output / "stdout.json").open("wb") as out, (output / "stderr.log").open("wb") as err:
        process = subprocess.Popen(command, env=environment, stdout=out, stderr=err)
        try:
            while process.poll() is None:
                try:
                    fields = Path(f"/proc/{process.pid}/status").read_text().splitlines()
                    for line in fields:
                        if line.startswith(("VmRSS:", "VmHWM:")):
                            peak_rss_kib = max(peak_rss_kib, int(line.split()[1]))
                except (FileNotFoundError, ProcessLookupError):
                    pass
                time.sleep(0.1)
        except BaseException:
            # Only this harness-owned scratch publisher; never another run.
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
                try:
                    process.wait(timeout=60)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            raise
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    receipt = dict(command=command, binary_sha256=sha(executable), exit_code=process.returncode,
                   wall_seconds=time.monotonic() - start,
                   cpu_seconds=after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime,
                   peak_rss_kib_sampled=peak_rss_kib, compilation_excluded=True)
    artifact = output / "artifact/artifact.json"
    if artifact.exists():
        report = json.loads(artifact.read_text())
        receipt.update(status=report["status"], inventory=report["inventory"],
                       cache=report.get("inventory_census_cache"))
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2))
    print(json.dumps({key: receipt[key] for key in
                      ("exit_code", "wall_seconds", "cpu_seconds", "peak_rss_kib_sampled")}), flush=True)
    if process.returncode:
        raise RuntimeError(f"{name} failed; see {output / 'stderr.log'}")
    return output / "artifact"


def compare(paths):
    reports = [json.loads((path / "artifact.json").read_text()) for path in paths]
    first = reports[0]
    for path, report in zip(paths, reports):
        assert report["status"] == "published_unrefined", path
        assert report["inventory"]["complete"] is True, path
        assert report["inventory"]["authority"] == "trusted_saved_scope", path
        for key in ("raw_terminals", "normalized_terminals", "remaining_terminals", "relation_rows"):
            assert first[key] == report[key], (path, key)
        for key in ("rules", "terminals", "selected_rule_events", "terminal_events", "zero_sector_events"):
            assert first["inventory"]["encountered"][key] == report["inventory"]["encountered"][key], (path, key)
        assert first["inventory"]["installed"] == report["inventory"]["installed"], path
        for state in ("native_state", "collection_state"):
            assert (paths[0] / first[state]["file"]).read_bytes() == (path / report[state]["file"]).read_bytes(), (path, state)
    return dict(exact_native_payloads_equal=True, encountered_counts_equal=True,
                assurance_unchanged=True,
                caveat="Exact per-rule identity/count comparisons also run in native tests; "
                       "this receipt compares aggregate inventory and exact terminal-session payloads.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("command", "checkpoint", "directory", "baseline", "candidate"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument("--cpus", default="32-35")
    args = parser.parse_args()
    for name in ("command", "checkpoint", "directory", "baseline", "candidate"):
        setattr(args, name, getattr(args, name).resolve())
    if args.threads < 1:
        parser.error("threads must be positive")
    args.directory.mkdir(parents=True, exist_ok=False)
    pointer = args.checkpoint / "latest.json"
    digest = sha(pointer)
    try:
        baseline = run(args, "baseline", args.baseline)
        cold = run(args, "candidate-cold", args.candidate)
        warm = run(args, "candidate-warm", args.candidate, cold)
        result = compare([baseline, cold, warm])
        result["source_checkpoint_sha256"] = digest
        result["source_checkpoint_unchanged"] = sha(pointer) == digest
        assert result["source_checkpoint_unchanged"]
        (args.directory / "comparison.json").write_text(json.dumps(result, indent=2))
    finally:
        assert sha(pointer) == digest, "source checkpoint advanced during the comparison"


if __name__ == "__main__":
    main()
