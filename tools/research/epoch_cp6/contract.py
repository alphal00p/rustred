"""Strict opt-in CP6 checkpoint-only receipts; not a replacement for Ready gates."""
import math

CONTRACT = "rustred.epoch-cp6-control.v1"
FORMAT = "RUSTRED-WALK-CP6"
TIMING = "launcher-inclusive nice+nix+native launch through owned process-group drain; lock admission and recorder shutdown excluded"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def integer(value, minimum=0):
    return type(value) is int and value >= minimum


def expected_schedule(plan):
    """Old plans retain their exact three-field lockstep receipt contract."""
    b = plan["b"]
    require(integer(b, 1) and b <= 4096, "invalid saved flight bound")
    if "schedule" not in plan:
        return {"kind": "lockstep", "depth": 1, "b": b}
    schedule = plan["schedule"]
    basic = {"kind", "depth", "b", "window", "cut_size", "publication_order", "dispatch"}
    extra_keys = {"result_escrow_jobs", "result_escrow_bytes"}
    require(isinstance(schedule, dict) and set(schedule) in (basic, basic | extra_keys),
            "explicit schedule shape")
    extra = schedule.get("result_escrow_jobs", 0)
    if extra_keys <= schedule.keys():
        require(integer(extra, 1) and integer(schedule["result_escrow_bytes"], 1)
                and schedule["result_escrow_bytes"] <= (1 << 64) - 1
                and schedule["kind"] == "rolling"
                and schedule["publication_order"] == "oldest_sequence_prefix",
                "explicit escrow schedule policy")
    require(schedule["kind"] in ("lockstep", "rolling")
            and schedule["dispatch"] in ("fifo", "adaptive")
            and schedule["publication_order"] in ("oldest_sequence_prefix", "oldest_ready_sequences")
            and all(integer(schedule[key], 1) for key in ("depth", "b", "window", "cut_size"))
            and schedule["b"] == schedule["window"] + extra == b
            and schedule["cut_size"] <= schedule["window"], "explicit schedule bounds/policy")
    cut = schedule["cut_size"]
    if schedule["kind"] == "rolling":
        require(schedule["depth"] == (b + cut - 1) // cut, "rolling cohort bound")
    else:
        require(schedule["depth"] == 1 and cut == b and schedule["dispatch"] == "fifo"
                and schedule["publication_order"] == "oldest_sequence_prefix",
                "lockstep schedule differs")
    return schedule


def checkpoint_schema(plan):
    schema = plan.get("checkpoint_schema", 1)
    require(type(schema) is int and schema in (1, 2, 3), "unknown CP6 manifest schema")
    return schema


def walk_semantics(plan):
    """Bind receipts to a frozen binary generation, never import old payloads.

    Historical control plans use manifest 1/2 and semantics 3. The typed-record
    implementation uses manifest 3 and semantics 4. Keeping both here permits
    matched comparisons with the frozen baseline executable; each native reader
    remains responsible for its own payload format.
    """
    return 4 if checkpoint_schema(plan) == 3 else 3


def clean_collection(metrics, summary, expected):
    require(expected["contract"] == CONTRACT, "explicit CP6 contract required")
    require(metrics["exit_code"] == 4 and metrics["stop_reason"] is None
            and metrics["runner_error"] is None and metrics["censored"] is False
            and metrics["killed_after_grace"] is False, "native failure or censoring")
    guard = metrics["resource_guard"]
    require(guard["child_started"] is True and guard["stop_reason"] is None
            and integer(guard["process_group"], 1), "native guard did not own a clean arm")
    for key in ("launcher_wait_seconds", "owned_group_drain_seconds"):
        require(type(guard[key]) in (int, float) and math.isfinite(guard[key]) and guard[key] >= 0,
                "missing owned-group drain timing")
    require(metrics["whole_command_timing_scope"] == TIMING
            and type(metrics["whole_command_seconds"]) in (int, float)
            and math.isfinite(metrics["whole_command_seconds"])
            and metrics["whole_command_seconds"] > 0, "timing boundary differs")
    for key, value in {"status": "incomplete", "finalization": "not_evaluated",
                       "recursive_worklist_exhausted": True, "admission_complete": True,
                       "full_result_in_output_document": False, "full_state_in_checkpoint": True,
                       "all_scheduled_domains_resolved": False, "observer_failed": False,
                       "family_closure_claim": False}.items():
        require(summary[key] == value and type(summary[key]) is type(value), f"summary {key}")
    require("domains" not in summary, "summary is not full inventory")
    for key in ("stop_reason", "operational_stop", "admission_failure"):
        require(summary[key] is None, f"summary {key}")
    for key in ("frontiers", "input_frontiers_count", "failed_nodes", "queued_nodes"):
        require(type(summary[key]) is int and summary[key] == 0, f"unfinished {key}")
    for key in ("scheduled_nodes", "native_processed_nodes"):
        require(integer(summary[key], 1), f"invalid {key}")
    require(summary["parallel"]["workers_joined"] is True
            and type(summary["parallel"]["active_workers"]) is int
            and summary["parallel"]["active_workers"] == 0, "workers not joined")
    epoch = summary["epoch"]
    schedule = expected_schedule(expected)
    actual_schedule = epoch["schedule"]
    require(isinstance(actual_schedule, dict) and actual_schedule.keys() == schedule.keys()
            and all(type(actual_schedule[key]) is type(value) and actual_schedule[key] == value
                    for key, value in schedule.items())
            and epoch["engine_certification_void"] is False
            and epoch["inspector_lookup_mode"] == expected["mode"],
            "mode, B or engine authority differs")
    g2 = expected.get("g2", "off")
    require(g2 in ("off", "union"), "unknown G2 control")
    report = summary.get("g2_residual_anchors")
    require((g2 == "off" and report is None)
            or (g2 == "union" and isinstance(report, dict) and report.get("mode") == "union"),
            "G2 mode differs")
    if "g2" in expected:
        require(metrics["g2"] == g2, "runner G2 receipt differs")
    roles = expected["roles"]
    admission = summary["query_admission"]
    for key, value in {"requested": roles["total"], "admitted": roles["total"], "unadmitted": 0,
                       "required": roles["required"], "auxiliary": roles["auxiliary"],
                       "admitted_required": roles["required"], "admitted_auxiliary": roles["auxiliary"]}.items():
        require(type(admission[key]) is int and admission[key] == value, f"query census {key}")
    checkpoint = summary["checkpoint"]
    require(checkpoint["format"] == FORMAT and checkpoint["schema"] == checkpoint_schema(expected)
            and checkpoint["state"] == "saved" and checkpoint["resumable"] is True
            and type(checkpoint["schema"]) is int and checkpoint["saved_this_invocation"] is True
            and checkpoint["paused"] is False and checkpoint["stop_reason"] is None
            and checkpoint["directory"] == expected["checkpoint"]
            and integer(checkpoint["generation"], 1), "saved CP6 authority absent")


def accept(metrics, summary, cold, audit, cold_guard, audit_guard, expected):
    """No I/O: callers must separately bind/freeze files and guard commands."""
    clean_collection(metrics, summary, expected)
    for receipt, exit_code in ((cold_guard, 0), (audit_guard, 1)):
        require(type(receipt["exit_code"]) is int and receipt["exit_code"] == exit_code
                and receipt["reason"] is None, "verification guard failed/censored")
    require(cold["verdict"] == "PASS" and cold["family_closure_claim"] is False
            and cold["mutation"] is None and cold["reference"]["native_levers"] == "Off"
            and cold["result_binding"] is None and cold["violations"] == []
            and type(cold["violations_suppressed"]) is int
            and cold["violations_suppressed"] == 0, "raw independent closure gate")
    total = cold["roots_total"]
    require(integer(total, 1) and type(cold["roots_independently_verified"]) is int
            and cold["roots_independently_verified"] == total, "root scope incomplete")
    reinspect = cold["reinspection"]
    n = reinspect["candidates"]
    require(integer(n, 1) and reinspect["mode"] == "All" and reinspect["complete"] is True
            and type(reinspect["selected"]) is int and reinspect["selected"] == n
            and type(reinspect["tally"]["inspected"]) is int
            and reinspect["tally"]["inspected"] == n, "not full native reinspection")
    require(n == summary["native_processed_nodes"], "native count mismatch")
    cp = cold["checkpoint"]
    require(cp["directory"] == expected["checkpoint"]
            and integer(cp["generation"], 1)
            and cp["generation"] == summary["checkpoint"]["generation"]
            and cp["publication_policy"] == "epoch" and type(cp["walk_semantics_version"]) is int
            and cp["walk_semantics_version"] == walk_semantics(expected)
            and cp["request_binding_matches"] is True and cp["owner_digests_match"] is True,
            "cold checkpoint generation/request binding")
    queries = cold["queries"]
    require(queries["blake3"] == expected["queries_blake3"], "cold query bytes differ")
    for key, value in {"count": expected["roles"]["total"], "admitted_prefix": expected["roles"]["total"],
                       "unadmitted": 0, "unresolved_input_frontiers": 0}.items():
        require(type(queries[key]) is int and queries[key] == value, f"cold query {key}")
    require(queries["unadmitted_ids"] == [], "unadmitted suffix")
    for name, count in (("physics", expected["roles"]["required"]), ("helper", expected["roles"]["auxiliary"])):
        row = cold["certification"]["classes"].get(name, {})
        for key in ("total", "independently_verified", "consistent_closed", "oracle_closed"):
            require(type(row.get(key, 0)) is int and row.get(key, 0) == count, f"{name} {key}")
    require(integer(cold["counts"]["domains"], 1) and integer(cold["counts"]["oracle_closed"], 1)
            and cold["counts"]["domains"] == summary["scheduled_nodes"]
            and cold["counts"]["oracle_closed"] == cold["counts"]["domains"], "unfinished domain inventory")
    require(audit["audit"] == "INCOMPLETE" and audit["violations"] == []
            and type(audit["violations_suppressed"]) is int and audit["violations_suppressed"] == 0
            and audit["all_local_obligations_discharged"] is False
            and audit["incomplete_reason"].startswith("checkpoint-only output has no full record proof;")
            and "verifier_pairing" not in audit, "Python summary contract is not a full-result PASS")
    result = {"contract": CONTRACT, "accepted": True, "authority": "raw_cp6_cold_all",
            "generation": cp["generation"], "mode": expected["mode"], "roles": expected["roles"],
            "native_exit_code": 4, "python_audit": "INCOMPLETE", "full_result_proof": False,
            "whole_command_seconds": metrics["whole_command_seconds"], "family_closure_claim": False}
    for key in ("schedule", "g2", "checkpoint_schema"):
        if key in expected:
            result[key] = expected[key]
    return result
