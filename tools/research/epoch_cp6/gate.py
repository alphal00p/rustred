#!/usr/bin/env python3
"""Freeze before verification, then accept exact read-only CP6 cold receipts."""
import argparse
import json
from pathlib import Path

from contract import CONTRACT, accept, require
from receipts import ROOT, commands, freeze, load_module, read, sha, write_new


def frozen_commands(plan):
    result = commands(plan)
    require(type(plan["verify_threads"]) is int and plan["verify_threads"] > 0
            and type(plan["verification_timeout"]) is int and plan["verification_timeout"] > 0,
            "bounded verifier allocation")
    for stage in result:
        cpus, locks = plan["verification_resources"][stage]["cpus"], plan["verification_resources"][stage]["locks"]
        require(cpus and all(type(n) is int and n >= 0 for n in cpus)
                and not set(cpus) & set(range(128, 228)) and locks, "verification resource allocation")
    return result


def freeze_receipt(plan):
    run = Path(plan["run"])
    value = freeze(plan)
    collection = read(run / "cp6-collection.json")
    require(collection["contract"] == CONTRACT and collection["status"] == "COLLECTED_UNACCEPTED"
            and collection["native_exit_code"] == 4 and collection["legacy_runner_exit_code"] == 1
            and collection["actual_launcher_argv"] == plan["launcher_argv"]
            and collection["runner_sha256"] == plan["runner_sha256"]
            and collection["metrics_sha256"] == sha(run / "metrics.json"), "collection binding")
    value["commands"] = frozen_commands(plan)
    value["collection_sha256"] = sha(run / "cp6-collection.json")
    value["binary_sha256"] = sha(plan["binary"])
    require(value["binary_sha256"] == plan["binary_sha256"], "binary differs")
    value["guard_sha256"] = sha(plan["verification_guard"])
    require(value["guard_sha256"] == plan["verification_guard_sha256"], "guard differs")
    value["audit_source_sha256"] = sha(ROOT / "examples/python/audit_owner_domain_walk.py")
    value["gate_sources_sha256"] = {str(path.relative_to(ROOT)): sha(path) for path in
        [ROOT / "examples/python/assert_oracle_pass.py", ROOT / "examples/python/owner_query_roles.py"]
        + [Path(__file__).with_name(name + ".py") for name in ("contract", "receipts", "collect", "gate")]}
    return value


def prepare_verification(plan):
    run = Path(plan["run"])
    for name in ("cold-verify.json", "cold-audit.json", "cold-verifier", "python-audit",
                 "cp6-before-verification.json", "cp6-accepted.json"):
        require(not (run / name).exists(), f"verification must start fresh: {name}")
    return freeze_receipt(plan)


def acceptance(plan, before):
    run = Path(plan["run"])
    after = freeze_receipt(plan)
    require(after == before, "cold verification changed frozen checkpoint/session/lock/pointers or run identity")
    reports = {}
    for stage, command in before["commands"].items():
        request = read(run / stage / "request.json")
        require(request["command"] == command, f"{stage}: exact guarded command differs")
        resources = plan["verification_resources"][stage]
        require(request["cwd"] == plan["verification_cwd"]
                and request["cpus"] == resources["cpus"] and request["locks"] == resources["locks"]
                and request["minimum_start_bytes"] == plan["minimum_start_bytes"]
                and request["stop_below_bytes"] == plan["minimum_run_bytes"], f"{stage}: guard resources differ")
        reports[stage] = read(run / stage / "result.json")
    cold = read(run / "cold-verify.json")
    oracle = load_module("_cp6_oracle_gate", ROOT / "examples/python/assert_oracle_pass.py")
    require(not oracle.gate(cold), "existing raw oracle PASS gate refused")
    result = accept(read(run / "metrics.json"), read(run / "result.json"), cold,
                    read(run / "cold-audit.json"), reports["cold-verifier"], reports["python-audit"], before["expected"])
    result["receipt_sha256"] = {name: sha(run / name) for name in
                                ("command.json", "metrics.json", "result.json", "cold-verify.json", "cold-audit.json")}
    result["guard_receipt_sha256"] = {stage: {name: sha(run / stage / name)
        for name in ("request.json", "result.json")} for stage in before["commands"]}
    result["checkpoint_read_only"] = True
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("action", choices=("freeze", "accept"))
    parser.add_argument("--plan", type=Path, required=True)
    args = parser.parse_args()
    plan = read(args.plan)
    run = Path(plan["run"])
    if args.action == "freeze":
        value = prepare_verification(plan)
        write_new(run / "cp6-before-verification.json", value)
        print(json.dumps(value["commands"], indent=2))
    else:
        result = acceptance(plan, read(run / "cp6-before-verification.json"))
        write_new(run / "cp6-accepted.json", result)
        print("CP6_RAW_COLD_ALL_ACCEPTED; staged summary remains incomplete; not deployment approval")


if __name__ == "__main__":
    main()
