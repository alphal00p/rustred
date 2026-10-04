#!/usr/bin/env python3
"""Plan matched shared-domain walks and read their existing native receipts.

This module never launches a process or performs algebra. The existing staging
tool, shared_owner_campaign supervisor, outer resource guard and native cold
verifier remain responsible for execution. Source-proof provenance is recorded,
not authenticated by Python; native cold walk reinspection is a distinct gate.
One request names one complete shared cohort, never independent per-root costs.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import math
from pathlib import Path


SCHEMA = "rustred.rule-optimizer-evaluation.v1"
ARMS = ("baseline", "candidate")
PHASES = ("stage", "walk", "cold")


def supervisor_module():
    path = Path(__file__).resolve().parents[3] / "examples/python/shared_owner_campaign.py"
    spec = importlib.util.spec_from_file_location("optimizer_existing_supervisor", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path):
    return json.loads(Path(path).read_text(), object_pairs_hook=unique_object)


def sha256(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def bound_file(binding):
    path = Path(binding["path"])
    require(path.is_absolute() and path.is_file(), f"not an absolute file: {path}")
    require(sha256(path) == binding["sha256"], f"input digest mismatch: {path}")
    return path


def option(command, flag):
    require(command.count(flag) == 1, f"command needs exactly one {flag}")
    position = command.index(flag) + 1
    require(position < len(command), f"missing value for {flag}")
    return command[position]


def replace_option(command, flag, value):
    option(command, flag)
    command[command.index(flag) + 1] = str(value)


def preferred_rule_fields(row):
    """Preserve the optional native saved-ordinal policy, never select rules here."""
    if "rule_ordinals" not in row:
        return {}
    ordinals = row["rule_ordinals"]
    require(ordinals is None or (isinstance(ordinals, list)
            and all(type(i) is int and 0 <= i < 2 ** 64 for i in ordinals)
            and all(a < b for a, b in zip(ordinals, ordinals[1:]))),
            "preferred rule_ordinals must be null or strictly increasing unsigned integers")
    # Native admission also validates membership in the FULL saved payload.
    return {"rule_ordinals": copy.deepcopy(ordinals)}


def payload_inventory(selection, owner_base):
    """Bind the actual bytes, including older manifests without sha256 fields."""
    result = {}
    for name, key in (("owners", "mask"), ("domain_rule_overlays", "owner_mask")):
        result[name] = []
        for row in selection.get(name, []):
            path = (owner_base / row["path"]).resolve(strict=True)
            require(path.stat().st_size == row["bytes"], f"payload size mismatch: {path}")
            digest = sha256(path)
            require(row.get("sha256", digest) == digest, f"payload digest mismatch: {path}")
            result[name].append({key: row[key], "bytes": row["bytes"], "sha256": digest})
            row["path"] = str(path)
    require(result["owners"], "empty owner pool")
    masks = [row["mask"] for row in result["owners"]]
    require(len(masks) == len(set(masks)), "duplicate owner masks")
    require(selection["owner_count"] == len(masks), "owner count mismatch")
    preferred = selection.get("preferred_owner_programs", [])
    require(isinstance(preferred, list), "preferred programs must be a list")
    seen = set()
    overlaid = {row["owner_mask"] for row in selection.get("domain_rule_overlays", [])}
    for row in preferred:
        mask = row.get("owner_mask")
        require(mask in masks and mask not in seen, "preferred programs need unique existing owners")
        require(mask not in overlaid, "preferred program has a same-owner repair overlay")
        require(row.get("residual_policy") == "defer-to-baseline", "unsupported preferred residual policy")
        seen.add(mask)
        path = (owner_base / row["path"]).resolve(strict=True)
        require(type(row["bytes"]) is int and row["bytes"] > 0
                and path.stat().st_size == row["bytes"], "preferred payload size mismatch")
        digest = sha256(path)
        require(row.get("sha256", digest) == digest, "preferred payload digest mismatch")
        result.setdefault("preferred_owner_programs", []).append({
            "owner_mask": mask, "bytes": row["bytes"], "sha256": digest,
            "residual_policy": row["residual_policy"], **preferred_rule_fields(row)})
        row["path"] = str(path)
    return result


def replace_replayed_overlays(candidate, baseline_inventory, changed_masks, replacements):
    """Transport explicitly re-exported overlays; native replay remains authority.

    Index, original digest and owner jointly select a row. Every overlay bound
    to a changed owner must be supplied exactly once; none is dropped/reordered.
    This never edits an overlay header or interprets a proof receipt as a proof.
    """
    require(isinstance(replacements, list), "overlay replacements must be a list")
    overlays = candidate.get("domain_rule_overlays", [])
    required = {i for i, row in enumerate(overlays) if row["owner_mask"] in changed_masks}
    seen = set()
    for replacement in replacements:
        index = replacement.get("overlay_index")
        require(type(index) is int and index in required and index not in seen,
                "overlay replacement needs a unique affected baseline overlay index")
        old = baseline_inventory[index]
        require(replacement.get("owner_mask") == old["owner_mask"]
                and replacement.get("original_sha256") == old["sha256"],
                "overlay replacement differs from its original owner/digest binding")
        provenance = replacement.get("source_provenance")
        require(isinstance(provenance, list) and provenance,
                "overlay replacement needs explicit native re-export provenance")
        for entry in provenance:
            bound_file(entry)
        path = bound_file(replacement)
        overlays[index].update(path=str(path), bytes=path.stat().st_size,
                               sha256=replacement["sha256"])
        seen.add(index)
    require(seen == required,
            "replacement has a base-bound repair overlay; every affected overlay needs a native re-export")


def plan(request):
    require(request["schema"] == SCHEMA, "unknown evaluation schema")
    require(sorted(request["arm_order"]) == sorted(ARMS), "one baseline and candidate required")
    destination = Path(request["destination"])
    require(destination.is_absolute(), "destination must be absolute")
    baseline = request["baseline"]
    selection = read_json(bound_file(baseline["selection"]))
    query_path = bound_file(baseline["queries"])
    query_bytes = query_path.read_bytes()
    queries = read_json(query_path)
    ids = [row["id"] for row in queries["queries"]]
    require(ids and len(ids) == len(set(ids)), "query IDs must be nonempty and unique")
    roles = queries.get("query_roles", {"required": ids, "auxiliary": []})
    require(sorted(roles["required"] + roles["auxiliary"]) == sorted(ids),
            "query roles must partition every exact query ID")
    owner_base = Path(baseline["owner_base"])
    require(owner_base.is_absolute(), "owner_base must be absolute")
    inventories = {"baseline": payload_inventory(selection, owner_base)}
    candidate = copy.deepcopy(selection)
    replacements = request.get("replacements", [])
    overlay_replacements = request.get("overlay_replacements", [])
    preferred_programs = request.get("preferred_programs", [])
    require(isinstance(preferred_programs, list), "preferred programs must be a list")
    changed_masks = {row["owner_mask"] for row in replacements}
    existing_preferences = {row["owner_mask"] for row in candidate.get("preferred_owner_programs", [])}
    require(not changed_masks.intersection(existing_preferences),
            "replacement of an owner with a preferred program requires a fresh composition")
    require(len({row["owner_mask"] for row in replacements}) == len(replacements),
            "duplicate owner replacement")
    for replacement in replacements:
        path = bound_file(replacement)
        matches = [row for row in candidate["owners"] if row["mask"] == replacement["owner_mask"]]
        require(len(matches) == 1, "replacement must name an existing owner")
        require(replacement["source_provenance"], "replacement needs explicit native source provenance")
        for provenance in replacement["source_provenance"]:
            bound_file(provenance)
        matches[0].update(path=str(path), bytes=path.stat().st_size, sha256=replacement["sha256"])
    replace_replayed_overlays(candidate, inventories["baseline"]["domain_rule_overlays"],
                              changed_masks, overlay_replacements)
    for program in preferred_programs:
        require(program["owner_mask"] not in changed_masks,
                "cannot replace and prefer the same owner in one treatment")
        require(program.get("residual_policy") == "defer-to-baseline", "unsupported preferred residual policy")
        path = bound_file(program)
        require(program["source_provenance"], "preferred program needs explicit native source provenance")
        for provenance in program["source_provenance"]:
            bound_file(provenance)
        candidate.setdefault("preferred_owner_programs", []).append({
            "owner_mask": program["owner_mask"], "path": str(path), "bytes": path.stat().st_size,
            "sha256": program["sha256"], "residual_policy": program["residual_policy"],
            **preferred_rule_fields(program)})
    candidate["total_bundle_bytes"] = sum(row["bytes"] for row in candidate["owners"])
    inventories["candidate"] = payload_inventory(candidate, owner_base)
    executable = bound_file(request["executable"])
    template = read_json(bound_file(request["walk_template"]))["command"]
    require(isinstance(template, list) and all(isinstance(word, str) for word in template),
            "walk template command must be an argv list")
    require(len(template) > 1 and Path(template[1]).name == "shared_owner_campaign.py",
            "use the existing shared_owner_campaign.py supervisor")
    require(not any(flag in template for flag in ("--resume", "--amend-queries", "--auto-rescue", "--targets")),
            "evaluation requires a fresh unchanged symbolic cohort")
    resources = {key: option(template, "--" + key) for key in
                 ("workers", "cpus", "max-memory-bytes", "host-memory-reserve-bytes")}
    resources["cpu_set"] = sorted(supervisor_module().parse_cpu_set(resources["cpus"]))
    native_template = read_json(bound_file(request["native_template"]))["command"]
    criterion = request["criterion"]
    metrics = ("scheduled_nodes", "native_processed_nodes", "events", "traversal_seconds", "complete_arm_seconds")
    require(criterion["primary_metric"] in metrics and 0 < criterion["minimum_relative_gain"] < 1,
            "preregister a supported primary metric and gain")
    require(set(criterion["regression_limits"]) == set(metrics)
            and all(type(value) in (int, float) and math.isfinite(value) and value >= 1
                    for value in criterion["regression_limits"].values()),
            "preregister every reported metric's allowed regression")
    budget = request["budget"]
    require(type(budget["inclusive_seconds"]) is int and 0 < budget["inclusive_seconds"] <= 1800,
            "inclusive pilot budget must be 1..1800 seconds")
    require(len(budget["phase_deadlines"]) == 2, "two arm deadline sets required")
    previous = 0
    for deadlines in budget["phase_deadlines"]:
        for phase in PHASES:
            soft, hard = deadlines[phase]
            require(previous < soft < hard <= budget["inclusive_seconds"], "invalid cumulative phase deadlines")
            previous = hard
    require(budget["work_mode"] in ("complete", "bounded"), "unknown work mode")
    if budget["work_mode"] == "bounded":
        require(type(budget["max_domains"]) is int and budget["max_domains"] > 0,
                "bounded work needs positive max_domains")
    arms = {}
    for name, pool in (("baseline", selection), ("candidate", candidate)):
        directory = destination / name
        walk = list(template)
        for flag, value in {"--executable": executable, "--manifest": directory / "inputs/selection.json",
                            "--queries": directory / "inputs/queries.json", "--owner-base": directory / "inputs",
                            "--run-directory": directory / "run", "--checkpoint": directory / "checkpoints/main",
                            "--max-queries": len(ids), "--max-query-bytes": len(query_bytes)}.items():
            replace_option(walk, flag, value)
        if budget["work_mode"] == "bounded":
            walk = [word for word in walk if word != "--unbounded-work"]
            require("--max-domains" not in walk, "set bounded max_domains only in the evaluation budget")
            walk.extend(("--max-domains", str(budget["max_domains"])))
        else:
            require("--unbounded-work" in walk, "complete-cohort template needs --unbounded-work")
        source = directory / "selection-source.json"
        stage = [template[0], "-B", str(Path(template[1]).with_name("stage_saved_owner_campaign.py")),
                 "--manifest", str(source), "--queries", str(query_path),
                 "--destination", str(directory / "inputs"), "--owner-base", str(owner_base)]
        cold = [str(executable), "walk-verify-closure", "--command", str(directory / "run/request.json"),
                "--checkpoint", str(directory / "checkpoints/main"), "--no-result", "--require-closure",
                "--reinspect", "all", "--certification-scope", "all-roots", "--reference-levers", "off",
                "--threads", resources["workers"], "--output", str(directory / "cold-all.json")]
        native = list(native_template)
        native[0] = str(executable)
        for flag, value in {"--manifest": directory / "inputs/selection.json", "--owner-base": directory / "inputs",
                            "--output": directory / "run/result.json", "--events": directory / "run/events.jsonl",
                            "--stop-file": directory / "run/stop-request.json", "--queries": directory / "inputs/queries.json",
                            "--checkpoint": directory / "checkpoints/main", "--max-queries": len(ids),
                            "--max-query-bytes": len(query_bytes)}.items():
            replace_option(native, flag, value)
        if budget["work_mode"] == "bounded":
            native = [word for word in native if word != "--unbounded-work"]
            require("--max-domains" not in native, "set native max_domains only in evaluation budget")
            native.extend(("--max-domains", str(budget["max_domains"])))
        arms[name] = {"selection": pool, "selection_source": str(source), "inventory": inventories[name],
                      "commands": {"stage": stage, "walk": walk, "cold": cold}, "expected_native_command": native}
    result = {"schema": SCHEMA, "cohort": request["cohort"], "destination": str(destination),
            "executable": request["executable"], "queries": baseline["queries"], "query_count": len(ids),
            "query_roles": roles, "arm_order": request["arm_order"], "resources": resources,
            "budget": budget, "criterion": criterion, "arms": arms, "replacements": replacements,
            "initial_cache": "fresh checkpoint and traversal in each arm",
            "scope": "one shared cohort including parent dispatch, Apply, Route and descendants",
            "source_authority": "provenance only here; independent native source replay remains required",
            "execution": "NOT RUN; existing outer guard must enforce these cumulative deadlines and drain owned groups"}
    if preferred_programs:
        result["preferred_programs"] = preferred_programs
    if overlay_replacements:
        result["overlay_replacements"] = copy.deepcopy(overlay_replacements)
    return result


def semantic_selection(selection):
    result = copy.deepcopy(selection)
    for field in ("owners", "domain_rule_overlays", "preferred_owner_programs"):
        for row in result.get(field, []):
            row.pop("path", None)
    return result


def normalized_native_command(command):
    command = list(command)
    for flag in ("--manifest", "--owner-base", "--output", "--events", "--stop-file", "--queries", "--checkpoint"):
        replace_option(command, flag, "<arm-local>")
    return command


def arm_result(expected, documents, query_count, query_digest):
    """Missing native fields are unresolved evidence, never zero debt or success."""
    issues = []
    stage, supervisor, walk, cold, measured = (documents[name] for name in
                                              ("input_receipt", "supervisor", "walk", "cold", "measurement"))
    def check(ok, message):
        if not ok:
            issues.append(message)
    check(semantic_selection(documents.get("selection", {})) == semantic_selection(expected["selection"]),
          "staged selection differs beyond payload paths")
    check(stage.get("query_bytes_unchanged") is True and stage.get("queries_sha256") == query_digest,
          "query bytes differ or staging receipt is incomplete")
    for name, key in (("owners", "mask"), ("domain_rule_overlays", "owner_mask")):
        actual = [{field: row.get(field) for field in (key, "bytes", "sha256")}
                  for row in stage.get(name, [])]
        check(actual == expected["inventory"][name], f"staged {name} differ")
    preferred_fields = ("owner_mask", "bytes", "sha256", "residual_policy")
    actual_preferred = [{**{field: row.get(field) for field in preferred_fields},
                         **({"rule_ordinals": row["rule_ordinals"]} if "rule_ordinals" in row else {})}
                        for row in stage.get("preferred_owner_programs", [])]
    check(actual_preferred == expected["inventory"].get("preferred_owner_programs", []),
          "staged preferred programs differ")
    for document, label in ((supervisor, "supervisor"), (walk, "walk")):
        checkpoint = document.get("checkpoint", {})
        check(checkpoint.get("pending_domains") == 0 and checkpoint.get("paused") is False,
              f"{label} has pending, paused or unknown debt")
        check(checkpoint.get("abandoned_obligations") == 0, f"{label} abandoned or unknown obligations")
    expected_checkpoint = option(expected["expected_native_command"], "--checkpoint")
    checkpoints = [document.get("checkpoint", {}) for document in (walk, supervisor, cold)]
    check(all(checkpoint.get("directory") == expected_checkpoint for checkpoint in checkpoints),
          "walk/supervisor/cold checkpoint path differs from this arm")
    generation = checkpoints[0].get("generation")
    check(type(generation) is int and generation > 0
          and all(checkpoint.get("generation") == generation for checkpoint in checkpoints),
          "walk/supervisor/cold checkpoint generation differs or is missing")
    check(supervisor.get("operator_or_resource_stop", "missing") is None
          and supervisor.get("native_stop_reason", "missing") is None
          and supervisor.get("hard_stopped") is False, "supervisor stopped or incomplete receipt")
    # Successful checkpoint-only result documents omit optional `error`.
    # Their explicit failure/admission fields, pending state and cold verifier
    # still have to establish success; absent `error` alone proves nothing.
    check(walk.get("error") is None and walk.get("frontiers") == 0
          and walk.get("admission_complete") is True
          and walk.get("admission_failure", "missing") is None
          and walk.get("failed_nodes") == 0 and walk.get("observer_failed") is False
          and walk.get("operational_stop", "missing") is None
          and walk.get("stop_reason", "missing") is None
          and walk.get("input_frontiers_count") == 0,
          "native error, frontier or incomplete admission")
    check(walk.get("parallel", {}).get("workers_joined") is True, "native workers not drained")
    check(cold.get("verdict") == "PASS" and cold.get("violations") == []
          and cold.get("violations_suppressed") == 0, "native cold verification did not pass")
    check(cold.get("closure_required") is True and cold.get("certification_scope") == "all_roots"
          and cold.get("reference", {}).get("native_levers") == "Off", "wrong cold verification scope")
    binding = cold.get("checkpoint", {})
    check(binding.get("owner_digests_match") is True and binding.get("request_binding_matches") is True,
          "native checkpoint input binding missing")
    q = cold.get("queries", {})
    check(q.get("count") == query_count and q.get("unadmitted") == 0
          and q.get("unresolved_input_frontiers") == 0, "cold query obligations incomplete")
    classes = cold.get("certification", {}).get("classes", {})
    check(bool(classes) and all(row.get("total") == row.get("independently_verified")
                               and type(row.get("total")) is int for row in classes.values())
          and sum(row.get("total", 0) for row in classes.values()) == query_count,
          "not every required/helper query independently verified")
    counts = cold.get("counts", {})
    check(type(counts.get("roots")) is int and counts.get("roots", 0) > 0
          and counts.get("roots") == counts.get("roots_independently_verified"), "not all roots cold verified")
    reinspection = cold.get("reinspection", {})
    check(reinspection.get("complete") is True and reinspection.get("mode") == "All"
          and type(reinspection.get("candidates")) is int
          and reinspection.get("selected") == reinspection.get("candidates"), "cold reinspection incomplete")
    tally = reinspection.get("tally", {})
    check(all(tally.get(key) == 0 for key in
              ("errors", "frontiers", "uncovered", "uncovered_after_reference_error")), "cold uncovered or unknown obligations")
    for phase in PHASES:
        receipt = measured.get("phases", {}).get(phase, {})
        check(receipt.get("owned_groups_drained") is True and receipt.get("stop_reason", "missing") is None
              and receipt.get("failure", "missing") is None and receipt.get("hard_stopped") is False
              and receipt.get("exit_code") in ((0, 4) if phase == "walk" else (0,)),
              f"{phase} stopped, failed or groups not drained")
    metrics = {key: walk.get(key) for key in ("scheduled_nodes", "native_processed_nodes", "events", "traversal_seconds")}
    metrics["complete_arm_seconds"] = measured.get("arm_seconds")
    check(all(type(value) in (int, float) and math.isfinite(value) and value > 0 for value in metrics.values()),
          "missing or invalid comparable cohort metrics")
    return {"state": "completed_cold_verified" if not issues else "incomplete_or_unverified",
            "issues": issues, "metrics": metrics,
            "debt": {"pending_domains": supervisor.get("checkpoint", {}).get("pending_domains"),
                     "frontiers": walk.get("frontiers"), "stop": supervisor.get("operator_or_resource_stop"),
                     "native_error": walk.get("error"), "abandoned": walk.get("abandoned_obligations")}}


def compare(evaluation_plan, documents):
    results = {name: arm_result(evaluation_plan["arms"][name], documents[name],
                                evaluation_plan["query_count"], evaluation_plan["queries"]["sha256"])
               for name in ARMS}
    comparison_issues = []
    commands = []
    for name in ARMS:
        native = documents[name].get("native_request", {})
        command = native.get("command", [])
        expected_digest = evaluation_plan["executable"]["sha256"]
        measured = documents[name]["measurement"]
        if (not command or command[0] != evaluation_plan["executable"]["path"]
                or measured.get("executable_sha256_before") != expected_digest
                or measured.get("executable_sha256_after") != expected_digest):
            comparison_issues.append(f"{name}: invoked executable is not bound before and after the run")
        if native.get("resume_requested") is not False or native.get("amend_queries") != []:
            comparison_issues.append(f"{name}: initial cache/request is not fresh")
        if (native.get("workers") != int(evaluation_plan["resources"]["workers"])
                or native.get("cpus") != evaluation_plan["resources"]["cpu_set"]):
            comparison_issues.append(f"{name}: worker/CPU placement differs from plan")
        if (native.get("hard_memory_bytes") != int(evaluation_plan["resources"]["max-memory-bytes"])
                or native.get("effective_hard_memory_bytes") != int(evaluation_plan["resources"]["max-memory-bytes"])
                or native.get("host_memory_reserve_bytes") != int(evaluation_plan["resources"]["host-memory-reserve-bytes"])):
            comparison_issues.append(f"{name}: memory allowance differs from plan")
        try:
            normalized = normalized_native_command(command)
            commands.append(normalized)
            if command != evaluation_plan["arms"][name]["expected_native_command"]:
                comparison_issues.append(f"{name}: native settings differ from preregistered command")
        except ValueError:
            comparison_issues.append(f"{name}: incomplete native invocation")
    if len(commands) != 2 or commands[0] != commands[1]:
        comparison_issues.append("native settings differ between arms")
    complete = not comparison_issues and all(result["state"] == "completed_cold_verified" for result in results.values())
    ratios = None
    target = False
    unchanged = evaluation_plan["arms"]["baseline"]["inventory"] == evaluation_plan["arms"]["candidate"]["inventory"]
    if complete:
        ratios = {key: results["candidate"]["metrics"][key] / value
                  for key, value in results["baseline"]["metrics"].items()}
        criterion = evaluation_plan["criterion"]
        target = (not unchanged and ratios[criterion["primary_metric"]] <= 1 - criterion["minimum_relative_gain"]
                  and all(value <= criterion["regression_limits"][key] for key, value in ratios.items()))
    return {"schema": SCHEMA, "cohort": evaluation_plan["cohort"], "arms": results,
            "completed_comparison": complete, "comparison_issues": comparison_issues, "candidate_over_baseline": ratios,
            "cohort_target_observed": target, "retain_baseline": True,
            "treatment": "unchanged_payload_aa_control" if unchanged else "changed_owner_payload",
            "promotion_authorized": False,
            "remaining_gates": "native original-source authority, unchanged terminal inventory, two counterbalanced pairs, full controls, held-outs and independent audit",
            "scope": evaluation_plan["scope"]}


def plan_summary(evaluation_plan, written=False):
    """Keep routing/family payloads on disk rather than flooding the console."""
    return {"schema": SCHEMA, "cohort": evaluation_plan["cohort"],
            "destination": evaluation_plan["destination"],
            "plan_file": str(Path(evaluation_plan["destination"]) / "plan.json") if written else None,
            "query_count": evaluation_plan["query_count"],
            "query_role_counts": {key: len(value) for key, value in evaluation_plan["query_roles"].items()},
            "arms": {name: {"owners": len(arm["inventory"]["owners"]),
                            "routes": len(arm["selection"].get("initial_frontier_routes", [])),
                            "overlays": len(arm["inventory"]["domain_rule_overlays"]),
                            **({"preferred_programs": len(arm["inventory"]["preferred_owner_programs"])}
                               if arm["inventory"].get("preferred_owner_programs") else {})}
                     for name, arm in evaluation_plan["arms"].items()},
            "resources": evaluation_plan["resources"], "budget": evaluation_plan["budget"],
            "execution": evaluation_plan["execution"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("mode", choices=("plan", "compare"))
    parser.add_argument("request", type=Path)
    parser.add_argument("--write-plan", action="store_true", help="create fresh plan and selection-source files; never launch")
    parser.add_argument("--full-plan", action="store_true", help="print full planning JSON, including potentially large routing payloads")
    args = parser.parse_args()
    request = read_json(args.request)
    if args.mode == "plan":
        result = plan(request)
        if args.write_plan:
            destination = Path(result["destination"])
            destination.mkdir(parents=True, exist_ok=False)
            for arm in result["arms"].values():
                path = Path(arm["selection_source"])
                path.parent.mkdir()
                path.write_text(json.dumps(arm["selection"], indent=2) + "\n")
            (destination / "plan.json").write_text(json.dumps(result, indent=2) + "\n")
        if not args.full_plan:
            result = plan_summary(result, written=args.write_plan)
    else:
        require(not args.write_plan, "--write-plan is only valid for planning")
        require(not args.full_plan, "--full-plan is only valid for planning")
        evaluation_plan = read_json(request["plan"])
        documents = {name: {key: read_json(path) for key, path in request["arms"][name].items()} for name in ARMS}
        result = compare(evaluation_plan, documents)
    print(json.dumps(result, indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
