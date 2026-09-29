#!/usr/bin/env python3
"""Explicit CP6 collection using the UNMODIFIED historical ArmGuard runner."""
import argparse
from contextlib import contextmanager
import os
from pathlib import Path
import sys

from contract import CONTRACT, clean_collection, require
from receipts import ROOT, argv_value, load_module, read, sha, validate_plan, write_new

RUNNER = ROOT / "tools/research/w1_g2prod/run_arm.py"


@contextmanager
def checked_launch(runner, plan):
    original = runner.ArmGuard.launch
    calls = []

    def launch(guard, command, **kwargs):
        require(not calls and command == plan["launcher_argv"], "unexpected/repeated launch")
        require(guard.stop_file == Path(plan["run"]) / "stop-request.json"
                and list(map(str, guard.paths)) == plan["locks"]
                and guard.minimum_start == plan["minimum_start_bytes"]
                and guard.minimum_run == plan["minimum_run_bytes"]
                and guard.time_limit == plan["time_limit"] and guard.grace == plan["grace"],
                "guard policy differs from frozen plan")
        require(set(os.sched_getaffinity(0)) == set(plan["cpus"]), "runner affinity differs")
        require(Path(kwargs["cwd"]) == Path(plan["native_cwd"]), "native working directory differs")
        require(sha(plan["binary"]) == plan["binary_sha256"], "binary changed before launch")
        for key, value in plan["resource_environment"].items():
            require(kwargs["env"].get(key) == value, f"resource environment differs: {key}")
        require(kwargs["env"].get("RUSTRED_EPOCH_LOCKSTEP_B") == plan.get("lockstep_environment"),
                "unregistered lockstep override")
        calls.append(list(command))
        # Same original object/function/kwargs own the session and every drain.
        return original(guard, command, **kwargs)

    runner.ArmGuard.launch = launch
    try:
        yield calls
    finally:
        runner.ArmGuard.launch = original


def collect(plan, runner):
    expected = validate_plan(plan)
    args = plan["runner_argv"]
    require("--perf" not in args and argv_value(args, "--policy") == "epoch"
            and argv_value(args, "--g2") == plan.get("g2", "off"),
            "CP6 timed collection profiling or G2 policy differs")
    require(plan["locks"] and argv_value(args, "--heavy-lock") == plan["locks"][0]
            and plan["minimum_start_bytes"] >= plan["minimum_run_bytes"] > 0
            and plan["time_limit"] > 0 and plan["grace"] > 0, "bounded guard policy required")
    require(plan["cpus"] and all(type(n) is int and n >= 0 for n in plan["cpus"])
            and len(set(plan["cpus"])) == len(plan["cpus"])
            and not set(plan["cpus"]) & set(range(128, 228)), "invalid or protected production CPUs")
    output = Path(argv_value(args, "--out-root")) / argv_value(args, "--label") / argv_value(args, "--family")
    require(output == Path(plan["run"]) and not output.exists()
            and not Path(plan["checkpoint"]).exists(), "fresh exact output/checkpoint required")
    saved_argv = sys.argv
    try:
        sys.argv = [str(RUNNER)] + args
        with checked_launch(runner, plan) as calls:
            code = runner.main()  # Historical return1 for native4 stays intact.
    finally:
        sys.argv = saved_argv
    metrics, summary = read(output / "metrics.json"), read(output / "result.json")
    clean_collection(metrics, summary, expected)
    require(code == 1 and len(calls) == 1, "expected native4/legacy-runner1 transport")
    require(read(output / "command.json") == plan["native_argv"], "actual native argv differs")
    receipt = {"contract": CONTRACT, "status": "COLLECTED_UNACCEPTED",
               "native_exit_code": 4, "legacy_runner_exit_code": code,
               "actual_launcher_argv": calls[0], "runner_sha256": sha(RUNNER),
               "metrics_sha256": sha(output / "metrics.json"),
               "guard_scope": "unchanged original ArmGuard and launcher-through-owned-group-drain boundary"}
    write_new(output / "cp6-collection.json", receipt)
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--collect-cp6-checkpoint-only", required=True, action="store_true")
    args = parser.parse_args()
    plan = read(args.plan)
    require(plan["contract"] == CONTRACT, "CP6 opt-in missing")
    require(sha(RUNNER) == plan["runner_sha256"], "runner changed")
    require(sha(plan["historical_helper"]) == plan["historical_helper_sha256"], "historical helper changed")
    sys.dont_write_bytecode = True
    runner = load_module("_cp6_original_runner", RUNNER)
    require(Path(runner.run_control.__file__).resolve() == Path(plan["historical_helper"]).resolve(),
            "unexpected imported historical helper")
    collect(plan, runner)
    print("CP6_COLLECTED_UNACCEPTED; raw cold All and strict receipt gate still required")


if __name__ == "__main__":
    main()
