"""Audit tests on a tiny synthetic walk; no native executable, algebra or license.

`walk_document`/`write_walk`/`build_run` are also imported by the comparison
and control-matrix tests to produce consistent fake result.json files.
"""
import copy
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


AUDIT = module("audit_owner_domain_walk")
POWER = {"max_positive_power": 3, "min_power_difference": None, "max_power_difference": None}
AUTHORITY = "same_snapshot_phase_owner_native_summary"


def queries_document():
    return {"schema": "rustred.owner-domain-queries.json.v2", "query_roles": {"required":["root-a"],"auxiliary":["helper-b"]}, "queries": [
        {"id": "root-a", "owner": "10", "lower": [1, 0], "upper": [3, 0], "max_numerator_rank": 2, "power_bounds": POWER},
        {"id": "helper-b", "owner": "01", "lower": [0, 1], "upper": [0, None], "max_numerator_rank": 1, "power_bounds": POWER}]}


def apply_stats(events, successors):
    return {"problems": 0, "unsupported_support_successors": 0, "conditional_unsupported_support_successors": 0,
            "same_support_successors": successors, "strict_subsupport_successors": 0, "successors": successors,
            "conditional_successors": 0, "events": events, "optional_coefficient_refusals": 0,
            "optional_original_refusals": 0, "optional_coalesced_refusals": 0, "native_operations": 10,
            "matching": {"cells": 1, "rules": 2}}


def route_stats():
    return {"events": 2, "apply_domains": 1, "route_domains": 0, "zero_sectors": 1, "missing_routes": 0,
            "masks_examined": 3, "masks_pruned": 1, "coordinate_cells": 4}


def native(identity, phase, owner, lower, upper, rank, stats, seconds=0.01):
    return {"id": identity, "record_kind": "native_inspection", "phase": phase, "owner": owner,
            "lower": lower, "upper": upper, "rank": rank, "power_bounds": dict(POWER), "error": None,
            "frontiers": [], "local_classification_discharged": True, "local_inspection_finished": True,
            "descendant_closed": True, "optional_refusals": [], "seconds": seconds, "stats": stats}


def walk_document(policy="ordered", checkpoint_dir="/checkpoint", workers=2):
    """A consistent exhausted walk: 6 logical records, 5 native (4 Apply + 1 Route), 1 alias, 1 partial."""
    records = [
        native(0, "Apply", "10", [1, 0], [3, 0], 2, apply_stats(3, 2)),
        native(1, "Apply", "01", [0, 1], [0, None], 1, apply_stats(2, 1)),
        {"id": 2, "record_kind": "delegated_not_inspected", "phase": "Apply", "owner": "10", "lower": [2, 0],
         "upper": [3, 0], "rank": 2, "power_bounds": dict(POWER), "representative_id": 3, "final_representative_id": 3,
         "responsibility_status": "discharged_by_representative", "containment_authority": AUTHORITY,
         "local_inspection_finished": False, "descendant_closed": True},
        native(3, "Apply", "10", [2, 0], [4, 0], 2, apply_stats(1, 0)),
        dict(native(4, "Route", "01", [0, 2], [0, 2], 1, route_stats()), conservative_route_overcover=True),
        dict(native(5, "Apply", "10", [1, 0], [1, 0], 2, apply_stats(1, 0)),
             record_kind="partial_initial_overlap_inspection", local_inspection_finished=False,
             residual_inspection_finished=True, responsibility_status="discharged_by_residual_and_initial_anchor",
             native_inspection_scope="low_D_residual_only",
             initial_overlap={"anchor_id": 0, "authority": AUTHORITY, "coordinates_and_rank_unchanged": True,
                              "covered_slice": "original_intersect_D_ge_cut", "cut": 2,
                              "residual_power_bounds": dict(POWER, max_power_difference=1)}),
    ]
    if policy == "ready":
        for record, accepted in zip((records[0], records[1], records[3], records[4], records[5]), (3, 2, 1, 2, 1)):
            record["accepted_events"] = accepted
    checkpoint = {"state": "saved", "paused": False, "pending_domains": 0, "committed_domains": 6,
                  "completed_native_inspections": 5, "committed_events": 9, "generation": 2, "bytes": 1234,
                  "duration_seconds": 0.5, "save_seconds": 0.49, "saved_unix_time": 1790000000,
                  "started_unix_time": 1789999999, "directory": str(checkpoint_dir),
                  "state_path": str(Path(checkpoint_dir) / "state-00000000000000000002.bin"),
                  "contiguous_publication_watermark": 6}
    inspectors = 1 if workers == 1 else workers - 1
    top = {
        "schema": "rustred.owner-domain-walk.json.v5" if policy == "ready" else "rustred.owner-domain-walk.json.v3",
        "status": "locally_resolved", "error": None, "all_scheduled_domains_resolved": True,
        "recursive_worklist_exhausted": True, "queued_nodes": 0, "frontiers": 0, "failed_nodes": 0,
        "pending_descendant_domains": 0, "input_frontiers": [], "uncommitted_inspections": [],
        "scheduled_nodes": 6, "processed_nodes": 6, "committed_domains": 6, "completed_nodes": 5,
        "native_processed_nodes": 5, "events": 9, "committed_events": 9, "routed_domains": 1, "route_masks": 3,
        "contiguous_publication_watermark": 6, "successors": 3, "conditional_successors": 0,
        "optional_coefficient_refusals": 0, "optional_original_refusals": 0, "optional_coalesced_refusals": 0,
        "unbounded_rank_domains": 0, "max_scheduled_finite_rank": 2, "initial_entry_domains_total": 2,
        "initial_entry_domains_inspected": 2, "initial_entry_domains_published": 2,
        "inputs": [{"id": "root-a", "domain": 0}, {"id": "helper-b", "domain": 1}],
        "delegation": {"all_ledger_obligations_discharged": True, "logical_publications": 6,
                       "native_publications": 5, "native_discharged": 5, "delegated_publications": 1,
                       "delegated_resolved": 1, "transferred_obligations": 1, "partial_initial_inspections": 1,
                       **{name: 0 for name in AUDIT.LEDGER_ZERO}},
        "parallel": {"workers": inspectors, "returned_inspections": 5, "first_failure": None,
                     "non_cancellation_failure": None, "backpressure_seconds": 0.0,
                     "admission_preparation": {"preparation_wall_seconds": 0.01, "ordered_commit_wall_seconds": 0.02,
                                               "inspection_worker_limit": inspectors},
                     **{name: 0 for name in AUDIT.POOL_ZERO}},
        "worker_allocation": {"inspection_worker_limit": inspectors, "admission_worker_limit": workers - inspectors - (1 if workers > 1 else 0),
                              "coordinator_worker_limit": 1 if workers > 1 else 0, "total_compute_worker_limit": workers,
                              "requested_inspection_workers": None, "requested_worker_budget": workers},
        "workers": workers, "requested_inspection_workers": None,
        "scheduling_policy": {"kind": "transfer_unreserved", "lookahead": 256},
        "checkpoint": checkpoint, "prepared_seconds": 0.1, "traversal_seconds": 0.2, "elapsed_seconds": 0.4,
        "partial_initial_inspections": 1, "publication_policy": policy,
        "descendant_closure": {"available": True, "initial_total": 2, "initial_closed": 2, "total_domains": 6,
                               "total_closed": 6, "unresolved_domains": 0},
        "timing_scope": "this_process_session", "domains": records,
    }
    return queries_document(), top


def heartbeat(elapsed, completed, scheduled, queued, rss, checkpoint=None, computing=None):
    parallel = {"active_workers": 1, "backpressured_workers": 0, "finished_uncommitted_domains": 0,
                "backpressure_seconds": 0.0,
                "admission_preparation": {"preparation_wall_seconds": elapsed * 0.1,
                                          "ordered_commit_wall_seconds": elapsed * 0.05}}
    if computing is not None:
        parallel["computing_workers"] = computing
    progress = {"event": "domain_progress", "completed_nodes": completed, "scheduled_nodes": scheduled,
                "queued_nodes": queued, "max_scheduled_finite_rank": 2, "parallel": parallel,
                "descendant_closure": {"available": True, "initial_total": 2, "initial_closed": min(completed, 2),
                                       "total_domains": scheduled, "total_closed": completed,
                                       "unresolved_domains": scheduled - completed}}
    if checkpoint is not None:
        progress["checkpoint"] = checkpoint
    return {"event": "heartbeat", "elapsed_seconds": elapsed, "process_rss_bytes": rss,
            "currently_discovered_nodes": scheduled, "progress": progress}


def write_walk(result_path, events_path, checkpoint_dir, queries, top, order=None):
    """Write result.json (pretty, sorted keys), events.jsonl and checkpoint/latest.json."""
    result_path, events_path, checkpoint_dir = Path(result_path), Path(events_path), Path(checkpoint_dir)
    document = dict(top)
    if order is not None:
        document["domains"] = [top["domains"][index] for index in order]
    result_path.parent.mkdir(parents=True, exist_ok=True)
    result_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    checkpoint = top["checkpoint"]
    checkpoint_dir.mkdir(parents=True, exist_ok=True)
    (checkpoint_dir / "latest.json").write_text(json.dumps({
        "schema": 2 if top["publication_policy"] == "ready" else 1, "kind": "state",
        "metadata": dict(checkpoint, duration_seconds=0.45, saved_unix_time=1789999999)}) + "\n")
    bootstrap = {"state": "saved", "generation": 1, "bootstrap": True, "bytes": 25, "saved_unix_time": 1789999990}
    events = [heartbeat(0.5, 2, 4, 2, 100_000, checkpoint=bootstrap),
              {"event": "checkpoint_saved", "checkpoint": bootstrap},
              heartbeat(1.0, 5, 6, 0, 120_000),
              {"event": "checkpoint_saved", "checkpoint": checkpoint},
              {"event": "heartbeat", "elapsed_seconds": 1.2, "process_rss_bytes": 120_000,
               "progress": {"event": "finished", "completed_nodes": 5, "scheduled_nodes": 6, "queued_nodes": 0,
                            "checkpoint": checkpoint}}]
    events_path.write_text("".join(json.dumps(event) + "\n" for event in events))


def build_run(directory, policy="ordered", mutate=None, order=None, receipt=True, workers=2):
    """A supervisor-style run directory: request.json, result.json, events.jsonl, checkpoint, receipt."""
    directory = Path(directory)
    run = directory / "run"
    run.mkdir(parents=True)
    checkpoint = directory / "checkpoint"
    queries, top = walk_document(policy, checkpoint, workers)
    if mutate is not None:
        mutate(top)
    queries_path = directory / "queries.json"
    queries_path.write_text(json.dumps(queries, indent=1) + "\n")
    write_walk(run / "result.json", run / "events.jsonl", checkpoint, queries, top, order)
    command = ["/fake/rustred", "owner-domain-match", "--manifest", str(directory / "selection.json"),
               "--owner-base", str(directory), "--output", str(run / "result.json"),
               "--events", str(run / "events.jsonl"), "--stop-file", str(run / "stop-request.json"),
               "--workers", str(workers), "--queries", str(queries_path), "--follow-successors",
               "--max-queries", "2", "--max-query-bytes", str(queries_path.stat().st_size),
               "--transfer-unreserved-lookahead", "256", "--publication-policy", policy,
               "--route-domain-overcover", "--reuse-initial-d-bands", "--checkpoint", str(checkpoint),
               "--checkpoint-interval-seconds", "3600", "--unbounded-work", "--no-progress"]
    (run / "request.json").write_text(json.dumps({"command": command, "cpus": [0]}, indent=1) + "\n")
    if receipt:
        (run / "supervisor-result.json").write_text(json.dumps({
            "exit_status": 0, "state": "completed", "elapsed_seconds": 1.0,
            "peak_observed_aggregate_rss_bytes": 120_000, "operator_or_resource_stop": None,
            "hard_stopped": False}) + "\n")
    return run


def alias_query(identity, owner="10", lower=(2, 0), upper=(3, 0), rank=1, power=None):
    """A later query that fits inside initial record 0 by default (helpers-first documents)."""
    power = {"max_positive_power": 2, "min_power_difference": 0, "max_power_difference": 1} if power is None else power
    return {"id": identity, "owner": owner, "lower": list(lower), "upper": list(upper),
            "max_numerator_rank": rank, "power_bounds": power}


def build_aliased_run(directory, aliases, inputs=None, mutate=None):
    """build_run plus `aliases` [(query, record)] admitted into existing records; `inputs` overrides the order."""
    def extend(top):
        top["inputs"] = inputs if inputs is not None else top["inputs"] + [
            {"id": query["id"], "domain": record} for query, record in aliases]
        if mutate is not None:
            mutate(top)
    run = build_run(directory, mutate=extend)
    queries = queries_document()
    queries["queries"] += [query for query, _ in aliases]
    queries["query_roles"]["required"] += [query["id"] for query, _ in aliases]
    (Path(directory) / "queries.json").write_text(json.dumps(queries, indent=1) + "\n")
    return run


class SyntheticWalkAuditTests(unittest.TestCase):
    def test_consistent_ordered_and_ready_walks_pass(self):
        for policy in ("ordered", "ready"):
            with self.subTest(policy=policy), tempfile.TemporaryDirectory() as temporary:
                run = build_run(Path(temporary), policy)
                report = AUDIT.audit_walk(run)
                self.assertEqual(report["violations"], [])
                self.assertEqual(report["audit"], "PASS")
                self.assertEqual(report["logical_records"], 6)
                self.assertEqual(report["native_inspections"], 5)
                self.assertEqual(report["native_by_phase"], {"Apply": 4, "Route": 1})
                self.assertEqual(report["aliases"], 1)
                self.assertEqual(report["partial_initial_inspections"], 1)
                self.assertEqual((report["initial_queries"], report["distinct_initial_records"],
                                  report["aliased_queries"]), (2, 2, 0))
                self.assertEqual(report["apply_stats"]["events"], 7)
                self.assertEqual(report["route_stats"]["masks_examined"], 3)
                self.assertEqual(report["max_scheduled_finite_rank"], 2)
                self.assertEqual(report["rank_histogram"], {"1": 2, "2": 4})
                self.assertEqual(report["publication_policy"], policy)
                self.assertFalse(report["family_closure_claim"])
                self.assertFalse(report["full_family_closure_claim"])
                self.assertEqual(report["receipt"]["exit_status"], 0)
                self.assertEqual(report["checkpoint_manifest_schema"], 2 if policy == "ready" else 1)

    def test_epoch_walk_passes_and_its_export_and_pool_are_checked(self):
        def epoch_run(temporary, edit=None, order=None):
            def mutate(top):
                for record in top["domains"]:
                    if record["record_kind"] != "delegated_not_inspected":
                        record["accepted_events"] = record["stats"]["events"]
                top["schema"] = "rustred.owner-domain-walk.json.v6"
                top["parallel"].update(merged_inspections=5, discarded_inspections=0, returned_inspections=5)
                top["checkpoint"] = {"format": "RUSTRED-EPOCH-EXPORT", "generation": 1, "state": "saved",
                                     "paused": False, "resumable": False, "pending_domains": 0,
                                     "committed_domains": 6, "completed_native_inspections": 5,
                                     "committed_events": 9, "stop_reason": None}
                if edit is not None:
                    edit(top)
            run = build_run(Path(temporary), "epoch", mutate=mutate, order=order)
            result = json.loads((run / "result.json").read_text())
            (Path(temporary) / "checkpoint" / "epoch-export.json").write_text(json.dumps({
                "format": "RUSTRED-EPOCH-EXPORT", "schema": 1, "resumable": False, "walk_semantics_version": 3,
                "metadata": result["checkpoint"]}))
            return run
        with tempfile.TemporaryDirectory() as temporary:
            report = AUDIT.audit_walk(epoch_run(temporary, order=[1, 0, 3, 2, 5, 4]))
            self.assertEqual(report["violations"], [])
            self.assertEqual(report["publication_policy"], "epoch")
        for schema, semantics, valid in ((2, 4, True), (2, 3, False), (1, 4, False),
                                         (True, 3, False), (2, True, False), (9, 4, False)):
            with self.subTest(schema=schema, semantics=semantics), tempfile.TemporaryDirectory() as temporary:
                run = epoch_run(temporary)
                path = Path(temporary) / "checkpoint" / "epoch-export.json"
                manifest = json.loads(path.read_text())
                manifest.update(schema=schema, walk_semantics_version=semantics)
                path.write_text(json.dumps(manifest))
                report = AUDIT.audit_walk(run)
                self.assertEqual(report["violations"] == [], valid, report)
        for fragment, edit in (
            ("epoch pool merged_inspections", lambda top: top["parallel"].update(merged_inspections=4)),
            ("lacks accepted_events", lambda top: top["domains"][0].pop("accepted_events")),
            ("epoch export manifest differs", lambda top: top["checkpoint"].update(committed_events=9, generation=2)),
        ):
            with self.subTest(fragment=fragment), tempfile.TemporaryDirectory() as temporary:
                run = epoch_run(temporary, edit=edit)
                if fragment == "epoch export manifest differs":
                    manifest = Path(temporary) / "checkpoint" / "epoch-export.json"
                    document = json.loads(manifest.read_text())
                    document["metadata"]["generation"] = 1
                    manifest.write_text(json.dumps(document))
                report = AUDIT.audit_walk(run)
                self.assertEqual(report["audit"], "FAIL")
                self.assertTrue(any(fragment in v for v in report["violations"]), report["violations"])

    def test_ready_walk_accepts_out_of_order_records_but_ordered_does_not(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = build_run(Path(temporary) / "ready", "ready", order=[1, 0, 3, 2, 5, 4])
            report = AUDIT.audit_walk(run)
            self.assertEqual(report["audit"], "PASS", report["violations"])
            self.assertEqual(report["out_of_order_records"], 3)
            run = build_run(Path(temporary) / "ordered", "ordered", order=[1, 0, 2, 3, 4, 5])
            report = AUDIT.audit_walk(run)
            self.assertIn("ordered records published out of order", report["violations"])

    def test_each_violation_class_is_reported(self):
        def cross_owner_alias(top):
            top["domains"][2]["owner"] = "01"

        def missing_route(top):
            top["domains"][4]["stats"]["missing_routes"] = 1
            top["domains"][4]["stats"]["events"] = 3
            top["events"] = top["committed_events"] = top["checkpoint"]["committed_events"] = 10

        def successor_sum(top):
            top["domains"][0]["stats"]["same_support_successors"] = 1

        def queue_not_drained(top):
            top["queued_nodes"] = 1

        def frontier(top):
            top["domains"][3]["frontiers"] = [{"reason": "unsupported"}]

        def pool_busy(top):
            top["parallel"]["active_workers"] = 1

        def ledger_pending(top):
            top["delegation"]["delegated_pending"] = 1

        def input_changed(top):
            top["domains"][1]["upper"] = [0, 7]

        def anchor_not_initial(top):
            top["domains"][5]["initial_overlap"]["anchor_id"] = 3

        def alias_to_alias(top):
            top["domains"][2]["representative_id"] = 5
            top["domains"][2]["final_representative_id"] = 5
            top["domains"][5]["record_kind"] = "delegated_not_inspected"

        def watermark(top):
            top["contiguous_publication_watermark"] = 5

        def ready_accepted(top):
            top["domains"][0]["accepted_events"] = 4

        cases = [("cross-phase/owner alias", cross_owner_alias, "ordered"),
                 ("missing_routes must be 0", missing_route, "ordered"),
                 ("successor sum mismatch", successor_sum, "ordered"),
                 ("queued_nodes must be 0", queue_not_drained, "ordered"),
                 ("nonzero frontiers", frontier, "ordered"),
                 ("pool active_workers must be 0", pool_busy, "ordered"),
                 ("ledger delegated_pending must be 0", ledger_pending, "ordered"),
                 ("upper changed", input_changed, "ordered"),
                 ("partial anchor must be an earlier initial record", anchor_not_initial, "ordered"),
                 ("final representative is not native", alias_to_alias, "ordered"),
                 ("watermark not contiguous", watermark, "ready"),
                 ("accepted_events do not sum", ready_accepted, "ready")]
        for fragment, mutate, policy in cases:
            with self.subTest(fragment=fragment), tempfile.TemporaryDirectory() as temporary:
                run = build_run(Path(temporary), policy, mutate=mutate)
                report = AUDIT.audit_walk(run)
                self.assertEqual(report["audit"], "FAIL")
                self.assertTrue(any(fragment in violation for violation in report["violations"]),
                                (fragment, report["violations"]))

    def test_oracle_checks_detect_each_injected_defect(self):
        def retargeted_alias(top):
            # Record 4 is a Route native: same owner check fails first; use a
            # same-bucket Apply native that does not contain the alias instead.
            top["domains"][3]["upper"] = [2, 0]
            top["domains"][3]["lower"] = [2, 0]
            top["domains"][2]["upper"] = [3, 0]
            top["domains"][2]["power_bounds"] = dict(POWER, max_positive_power=4)

        def seal_with_frontier(top):
            top["domains"][3]["frontiers"] = [{"kind": "local_dispatch_frontier"}]
            top["frontiers"] = 1

        def seal_with_error(top):
            top["domains"][1]["error"] = "injected"
            top["failed_nodes"] = 1

        def dropped_frontier_record(top):
            top["frontiers"] = 1

        def accepted_mismatch(top):
            top["domains"][1]["accepted_events"] = 1
            top["domains"][0]["accepted_events"] = 4

        def partial_outside_anchor(top):
            top["domains"][5]["initial_overlap"]["cut"] = 1
            top["domains"][5]["initial_overlap"]["residual_power_bounds"] = dict(POWER, max_power_difference=0)
            top["domains"][0]["lower"] = [2, 0]
            top["domains"][0]["upper"] = [3, 0]

        def unclosed_record(top):
            top["domains"][4]["descendant_closed"] = False

        cases = [("alias domain not contained in representative", retargeted_alias, "ordered", False),
                 ("descendant_closed despite a frontier or error", seal_with_frontier, "ordered", False),
                 ("descendant_closed despite a frontier or error", seal_with_error, "ordered", False),
                 ("frontier parity", dropped_frontier_record, "ordered", False),
                 ("accepted_events != stats.events", accepted_mismatch, "ready", False),
                 ("partial D>=cut slice not contained in anchor", partial_outside_anchor, "ordered", False),
                 ("closure counters != per-record descendant_closed claims", unclosed_record, "ordered", False),
                 ("closure required: a record is not descendant_closed", unclosed_record, "ordered", True)]
        for fragment, mutate, policy, require in cases:
            with self.subTest(fragment=fragment), tempfile.TemporaryDirectory() as temporary:
                run = build_run(Path(temporary), policy, mutate=mutate)
                report = AUDIT.audit_walk(run, require_closure=require)
                self.assertEqual(report["audit"], "FAIL")
                self.assertTrue(any(fragment in violation for violation in report["violations"]),
                                (fragment, report["violations"]))

    def test_require_closure_passes_and_reports_helper_and_physics_roots(self):
        aliases = [(alias_query("phys-c"), 0), (alias_query("phys-d", "01", (0, 4), (0, 9), 0, dict(POWER)), 1)]
        with tempfile.TemporaryDirectory() as temporary:
            run = build_aliased_run(Path(temporary), aliases)
            report = AUDIT.audit_walk(run, require_closure=True)
            self.assertEqual(report["audit"], "PASS", report["violations"])
            certification = report["certification"]
            self.assertTrue(certification["engine_closure_consistent"])
            self.assertFalse(certification["independently_verified"])
            self.assertEqual(certification["roots"], {"total": 2, "closed": 2})
            self.assertEqual(certification["queries"]["helper"], {"total": 1, "admitting": 1, "closed": 1})
            self.assertEqual(certification["queries"]["physics"], {"total": 3, "admitting": 1, "absorbed": 2, "closed": 3})
            self.assertEqual(report["alias_containment_checks"], 1)
            self.assertEqual(report["partial_anchor_containment_checks"], 1)
            self.assertEqual(report["containment_oracle"]["exact_vs_brute_force_disagreements"], 0)

    def test_verifier_report_must_be_bound_to_the_audited_result(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = build_run(Path(temporary))
            result = run / "result.json"
            generation = json.loads(result.read_text())["checkpoint"]["generation"]

            def verifier(**change):
                stat = result.stat()
                binding = {"canonical_path": str(result.resolve()), "file_bytes": stat.st_size,
                           "file_mtime_unix_ns": stat.st_mtime_ns, "file_blake3": "0" * 64,
                           "generation": generation, "generation_matches": True, "records_compared": 6,
                           "records_mismatched": 0, "records_not_published": 0, "duplicate_rows": 0}
                report = {"verdict": "PASS", "closure_required": True, "checkpoint": {"generation": generation},
                          "result_binding": binding, "roots_total": 2, "roots_independently_verified": 2}
                for key, value in change.items():
                    if key in binding:
                        binding[key] = value
                    else:
                        report[key] = value
                path = Path(temporary) / "verify.json"
                path.write_text(json.dumps(report))
                return path

            paired = AUDIT.audit_walk(run, require_closure=True, verify_report=verifier())
            self.assertEqual(paired["audit"], "PASS", paired["violations"])
            self.assertTrue(paired["verifier_pairing"]["paired"])
            self.assertTrue(paired["certification"]["independently_verified"])
            cases = [("changed since the verifier bound it", {"file_bytes": 1}),
                     ("a different result.json", {"canonical_path": "/elsewhere/result.json"}),
                     ("checkpoint generation differs", {"generation": generation + 1}),
                     ("rows differ from the verified generation", {"records_mismatched": 1}),
                     ("is not PASS", {"verdict": "INCOMPLETE"}),
                     ("did not require closure", {"closure_required": False}),
                     ("not every root is independently verified", {"roots_independently_verified": 1}),
                     ("not every root is independently verified", {"roots_total": None}),
                     ("--mutate negative control", {"mutation": {"kind": "alias-chain-detour", "applied": True}}),
                     ("bound no result.json", {"result_binding": None})]
            for fragment, change in cases:
                with self.subTest(fragment=fragment):
                    report = AUDIT.audit_walk(run, require_closure=True, verify_report=verifier(**change))
                    self.assertEqual(report["audit"], "FAIL")
                    self.assertFalse(report["certification"]["independently_verified"])
                    # `paired` is the conjunction of the pairing checks, never set unconditionally.
                    self.assertFalse(report["verifier_pairing"]["paired"])
                    self.assertTrue(any(fragment in violation for violation in report["violations"]),
                                    (fragment, report["violations"]))

    def test_ready_alias_whose_representative_streamed_first_is_checked_in_a_second_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = build_run(Path(temporary), "ready", order=[0, 1, 3, 2, 4, 5])
            report = AUDIT.audit_walk(run)
            self.assertEqual(report["audit"], "PASS", report["violations"])
            self.assertEqual(report["alias_containment_checks"], 1)

    def test_exact_lattice_inclusion_matches_point_enumeration(self):
        import random
        rng = random.Random(5)
        def cell(owner):
            lower = [rng.randrange(3) for _ in owner]
            upper = [low + rng.randrange(4) if rng.randrange(4) else None for low in lower]
            d = lambda: rng.randrange(9) - 3 if rng.randrange(4) else None
            least, most = d(), d()
            if least is not None and most is not None and least > most:
                least, most = most, least
            return (owner, "Apply", tuple(lower), tuple(upper), rng.randrange(6) if rng.randrange(4) else None,
                    rng.randrange(9) if rng.randrange(4) else None, least, most)
        window = [()]
        for _ in range(3):
            window = [p + (x,) for p in window for x in range(12)]
        for _ in range(3000):
            owner = "".join(rng.choice("01") for _ in range(3))
            outer, inner = cell(owner), cell(owner)
            inside = [p for p in window if AUDIT.box_member(inner, p)]
            self.assertEqual(AUDIT.box_nonempty(inner), bool(inside), inner)
            points = AUDIT.box_points(inner, 10 ** 6)
            if points is not None and all(u is not None and u < 12 for u in inner[3]):
                self.assertEqual(sorted(points), sorted(inside))
                self.assertEqual(AUDIT.box_contains(outer, inner), all(AUDIT.box_member(outer, p) for p in inside),
                                 (outer, inner))

    def test_later_query_aliased_into_containing_initial_record_is_accepted(self):
        aliases = [(alias_query("phys-c"), 0),
                   (alias_query("phys-d", "01", (0, 4), (0, 9), 0, dict(POWER)), 1),
                   (alias_query("root-a-again", lower=(1, 0), rank=2, power=dict(POWER)), 0)]
        with tempfile.TemporaryDirectory() as temporary:
            report = AUDIT.audit_walk(build_aliased_run(Path(temporary), aliases))
            self.assertEqual(report["violations"], [])
            self.assertEqual(report["audit"], "PASS")
            self.assertEqual((report["initial_queries"], report["distinct_initial_records"],
                              report["aliased_queries"]), (5, 2, 3))

    def test_alias_into_non_containing_record_keeps_changed_query_violations(self):
        cases = [("lower", alias_query("phys-c", lower=(0, 0))),
                 ("upper", alias_query("phys-c", upper=(4, 0))),
                 ("upper", alias_query("phys-c", upper=(3, None))),
                 ("rank", alias_query("phys-c", rank=3)),
                 ("rank", alias_query("phys-c", rank=None)),
                 ("power_bounds", alias_query("phys-c", power=dict(POWER, max_positive_power=4))),
                 ("power_bounds", alias_query("phys-c", power=dict(POWER, max_positive_power=None)))]
        for field, query in cases:
            with self.subTest(field=field, query=query), tempfile.TemporaryDirectory() as temporary:
                report = AUDIT.audit_walk(build_aliased_run(Path(temporary), [(query, 0)]))
                self.assertEqual(report["audit"], "FAIL")
                self.assertIn(f"query phys-c: {field} changed", report["violations"])
                self.assertEqual(report["aliased_queries"], 0)

    def test_alias_across_owners_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            # Equal to record 0 in every coordinate, rank and power bound except the owner.
            query = alias_query("phys-c", "01", lower=(1, 0), rank=2, power=dict(POWER))
            report = AUDIT.audit_walk(build_aliased_run(Path(temporary), [(query, 0)]))
            self.assertEqual(report["violations"], ["query phys-c: owner changed"])
            self.assertEqual(report["aliased_queries"], 0)

    def test_admitting_query_and_distinct_initial_records_stay_exact(self):
        def wider_record(top):
            top["domains"][0]["upper"] = [4, 0]

        def query_count_as_initial(top):
            for field in ("total", "inspected", "published"):
                top[f"initial_entry_domains_{field}"] = 3

        def anchor_after_initial(top):
            top["domains"][5]["initial_overlap"]["anchor_id"] = 3

        root, helper = {"id": "root-a", "domain": 0}, {"id": "helper-b", "domain": 1}
        two_aliases = [(alias_query("phys-c"), 0), (alias_query("phys-e"), 0)]
        cases = [("query root-a: upper changed", [], None, wider_record),
                 ("initial entry obligations not discharged", [(alias_query("phys-c"), 0)], None,
                  query_count_as_initial),
                 # The first query naming a record admitted it, so it must equal it.
                 ("query phys-c: lower changed", [(alias_query("phys-c"), 0)],
                  [{"id": "phys-c", "domain": 0}, root, helper], None),
                 ("inputs do not map every query to an initial record", [], [root, helper, root], None),
                 # An alias targets an initial record, never a later successor,
                 # even when the query count reaches that successor's id.
                 ("inputs do not map every query to an initial record", [(alias_query("phys-c"), 3)], None, None),
                 ("query phys-c: no initial record", [(alias_query("phys-c"), 3)], None, None),
                 ("query phys-d: no initial record", two_aliases + [(alias_query("phys-d"), 3)], None, None),
                 # Partial anchors are bounded by the distinct records, not the queries.
                 ("record 5: partial anchor must be an earlier initial record", two_aliases, None,
                  anchor_after_initial)]
        for fragment, aliases, inputs, mutate in cases:
            with self.subTest(fragment=fragment), tempfile.TemporaryDirectory() as temporary:
                report = AUDIT.audit_walk(build_aliased_run(Path(temporary), aliases, inputs, mutate))
                self.assertEqual(report["audit"], "FAIL")
                self.assertIn(fragment, report["violations"])

    def test_alias_containment_mirrors_the_rust_null_semantics(self):
        record = {"owner": "10", "lower": [1, 0], "upper": [3, None], "rank": 2,
                  "power_bounds": {"max_positive_power": None, "min_power_difference": 2, "max_power_difference": 5}}
        inside = alias_query("q", lower=(1, 4), upper=(3, 7), rank=2,
                             power={"max_positive_power": 9, "min_power_difference": 3, "max_power_difference": 5})
        self.assertTrue(AUDIT.alias_contains(record, inside))
        self.assertTrue(AUDIT.alias_contains(record, dict(inside, upper=[2, None])))
        for change in ({"upper": [None, 7]}, {"lower": [0, 4]}, {"max_numerator_rank": None},
                       {"max_numerator_rank": 3}, {"owner": "01"},
                       {"power_bounds": dict(inside["power_bounds"], min_power_difference=1)},
                       {"power_bounds": dict(inside["power_bounds"], min_power_difference=None)},
                       {"power_bounds": dict(inside["power_bounds"], max_power_difference=6)},
                       {"power_bounds": dict(inside["power_bounds"], max_power_difference=None)},
                       {"power_bounds": {"max_positive_power": 9, "max_power_difference": 5}},
                       {"lower": [1]}, {"upper": [3, True]}):
            with self.subTest(change=change):
                self.assertFalse(AUDIT.alias_contains(record, dict(inside, **change)))
        unbounded = dict(record, rank=None, power_bounds={field: None for field in AUDIT.POWER_FIELDS})
        self.assertTrue(AUDIT.alias_contains(unbounded, dict(inside, max_numerator_rank=None, power_bounds={})))
        self.assertFalse(AUDIT.alias_contains(dict(record, power_bounds={"min_power_difference": 2}), inside))

    def test_alias_containment_requires_walker_fields_and_parser_valid_queries(self):
        record = {"owner": "10", "lower": [1, 0], "upper": [3, None], "rank": None,
                  "power_bounds": {"max_positive_power": None, "min_power_difference": None, "max_power_difference": None}}
        inside = alias_query("q", lower=(1, 4), upper=(3, 7), rank=2,
                             power={"max_positive_power": 9, "min_power_difference": 3, "max_power_difference": 5})
        self.assertTrue(AUDIT.alias_contains(record, inside))
        self.assertTrue(AUDIT.alias_contains(record, dict(inside, lower=[1, 2 ** 64 - 1], upper=[3, None])))
        # Each row is contained by the syntactic comparison alone, but the
        # walker writes no such record or its query parser rejects the query.
        records = [{key: value for key, value in record.items() if key != "rank"}]
        implicit_rank = {key: value for key, value in inside.items() if key != "max_numerator_rank"}
        queries = [implicit_rank] + [dict(inside, **change) for change in (
            {"lower": [1, 8]},
            {"lower": [1, 2 ** 64], "upper": [3, None]},
            {"max_numerator_rank": -1}, {"max_numerator_rank": 2 ** 32},
            {"power_bounds": dict(inside["power_bounds"], min_power_difference=6)},
            {"power_bounds": dict(inside["power_bounds"], max_positive_power=-1)},
            {"power_bounds": dict(inside["power_bounds"], min_power_difference=-2 ** 63 - 1)},
            {"power_bounds": dict(inside["power_bounds"], max_power_difference=2 ** 63)})]
        for changed in records:
            with self.subTest(record=changed):
                self.assertFalse(AUDIT.alias_contains(changed, inside))
        for changed in queries:
            with self.subTest(query=changed):
                self.assertFalse(AUDIT.alias_contains(record, changed))

    def test_alias_containment_rejects_malformed_fields(self):
        record = {"owner": "10", "lower": [1, 0], "upper": [3, None], "rank": None,
                  "power_bounds": {"max_positive_power": None, "min_power_difference": None, "max_power_difference": None}}
        inside = alias_query("q", lower=(1, 4), upper=(3, 7), rank=2,
                             power={"max_positive_power": 9, "min_power_difference": 3, "max_power_difference": 5})
        self.assertTrue(AUDIT.alias_contains(record, inside))
        # Each pair is contained when the malformed field is ignored or coerced.
        pairs = [(dict(record, owner=["1", "0"]), dict(inside, owner=["1", "0"])),
                 (dict(record, lower=[1]), inside), (record, dict(inside, upper=[3])),
                 (dict(record, power_bounds=dict(record["power_bounds"], extra=None)), inside),
                 (record, dict(inside, power_bounds=dict(inside["power_bounds"], extra=None))),
                 (record, dict(inside, power_bounds=None)),
                 (record, dict(inside, lower=[True, 4])), (record, dict(inside, lower=[1.0, 4])),
                 (record, dict(inside, max_numerator_rank=True)),
                 (record, dict(inside, power_bounds=dict(inside["power_bounds"], max_positive_power=True)))]
        for outer, query in pairs:
            with self.subTest(record=outer, query=query):
                self.assertFalse(AUDIT.alias_contains(outer, query))

    def test_initial_records_are_numbered_in_first_appearance_order(self):
        # A document admitting helper-b first gives it record 0, never record 1.
        with tempfile.TemporaryDirectory() as temporary:
            run = build_aliased_run(Path(temporary), [], [{"id": "helper-b", "domain": 1}, {"id": "root-a", "domain": 0}])
            queries = queries_document()
            queries["queries"].reverse()
            (Path(temporary) / "queries.json").write_text(json.dumps(queries, indent=1) + "\n")
            report = AUDIT.audit_walk(run)
            self.assertEqual(report["violations"], ["inputs do not map every query to an initial record"])

    def test_inputs_must_follow_the_query_document_order(self):
        aliases = [(alias_query("phys-c"), 0)]
        inputs = [{"id": "root-a", "domain": 0}, {"id": "phys-c", "domain": 0}, {"id": "helper-b", "domain": 1}]
        with tempfile.TemporaryDirectory() as temporary:
            report = AUDIT.audit_walk(build_aliased_run(Path(temporary), aliases, inputs))
            self.assertEqual(report["violations"], ["inputs are not in query document order"])

    def test_resumed_walk_may_carry_only_reinspected_earlier_session_attempts(self):
        def carried(identity, flag=True):
            entry = {"id": identity, "committed": False, "physical_part": None, "error": "Cancelled",
                     "seconds": 0.03, "stats": apply_stats(0, 0)}
            if flag:
                entry["resume_reinspects_unfinished_part"] = True
            return entry

        def resumed(run):
            request = json.loads((run / "request.json").read_text())
            command = request["command"]
            command[command.index("--checkpoint")] = "--resume"
            (run / "request.json").write_text(json.dumps(request))

        def carrying(entries, returned):
            def mutate(top):
                top["uncommitted_inspections"] = entries
                top["parallel"]["returned_inspections"] = returned
            return mutate

        # (resumed, mutation, violation fragment or None, expected surplus when PASS)
        cases = [(True, carrying([carried(3), carried(0)], 7), None, 0),
                 (False, carrying([carried(3)], 6), "uncommitted_inspections must be empty", None),
                 (True, carrying([carried(3, flag=False)], 6), "is not a carried earlier-session attempt", None),
                 (True, carrying([carried(2)], 6), "carried attempt 2 was not re-inspected", None),
                 (True, carrying([carried(3)], 5), "pool returned_inspections < native records + carried", None),
                 # A crash after a periodic save: unpolled or escrowed results
                 # were counted as returned there and re-inspected here.
                 (True, carrying([], 7), None, 2),
                 (True, carrying([carried(3)], 8), None, 2),
                 (False, carrying([], 7), "pool returned_inspections != native records + carried", None)]
        for index, (resume, mutate, fragment, surplus) in enumerate(cases):
            with self.subTest(case=index), tempfile.TemporaryDirectory() as temporary:
                run = build_run(Path(temporary), "ready", mutate=mutate)
                if resume:
                    resumed(run)
                report = AUDIT.audit_walk(run)
                self.assertEqual(report["resumed"], resume)
                if fragment is None:
                    self.assertEqual(report["audit"], "PASS", report["violations"])
                    self.assertEqual(report["resumed_unpublished_returned_inspections"], surplus)
                else:
                    self.assertEqual(report["audit"], "FAIL")
                    self.assertTrue(any(fragment in violation for violation in report["violations"]),
                                    (fragment, report["violations"]))

    def test_receipt_manifest_command_and_schema_expectations(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = build_run(root, "ordered")
            (run / "supervisor-result.json").write_text(json.dumps({
                "exit_status": 4, "hard_stopped": True, "operator_or_resource_stop": "aggregate_rss_hard_limit"}))
            report = AUDIT.audit_walk(run)
            self.assertIn("resource receipt exit_status != 0", report["violations"])
            self.assertIn("supervisor hard-stopped the native process", report["violations"])
            (run / "supervisor-result.json").unlink()
            guard = root / "guard"
            guard.mkdir()
            (guard / "result.json").write_text(json.dumps({"exit_status": 0, "forced": False, "stop_reason": None,
                                                           "supervisor_error": None, "elapsed_seconds": 2.0}))
            report = AUDIT.audit_walk(run, receipt=guard / "result.json")
            self.assertEqual(report["audit"], "PASS", report["violations"])
            self.assertEqual(report["receipt"]["elapsed_seconds"], 2.0)
            report = AUDIT.audit_walk(run, receipt=guard / "result.json", expect_schema="rustred.owner-domain-walk.json.v9")
            self.assertIn("schema 'rustred.owner-domain-walk.json.v3' != expected 'rustred.owner-domain-walk.json.v9'",
                          report["violations"])
            (root / "checkpoint" / "latest.json").write_text(json.dumps({"schema": 1, "kind": "state", "metadata": {"generation": 1}}))
            report = AUDIT.audit_walk(run, receipt=guard / "result.json")
            self.assertIn("durable checkpoint manifest differs from the result's checkpoint bookkeeping", report["violations"])
            request = json.loads((run / "request.json").read_text())
            request["command"][request["command"].index("--workers") + 1] = "3"
            (run / "request.json").write_text(json.dumps(request))
            report = AUDIT.audit_walk(run, receipt=guard / "result.json")
            self.assertIn("result workers != command --workers", report["violations"])
            (run / "request.json").unlink()
            report = AUDIT.audit_walk(run)
            self.assertTrue(any(violation.startswith("structural:") for violation in report["violations"]))

    def test_cli_writes_audit_json_and_exit_status(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = build_run(Path(temporary), "ordered")
            result = subprocess.run([sys.executable, "-B", AUDIT.__file__, str(run)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads((run / "audit.json").read_text())["audit"], "PASS")
            self.assertEqual(json.loads(result.stdout)["native_inspections"], 5)
            (run / "result.json").write_text((run / "result.json").read_text().replace('"queued_nodes": 0', '"queued_nodes": 2'))
            result = subprocess.run([sys.executable, "-B", AUDIT.__file__, str(run), "--no-output",
                                     "--output", str(Path(temporary) / "elsewhere.json")], capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(json.loads((run / "audit.json").read_text())["audit"], "PASS")
            self.assertFalse((Path(temporary) / "elsewhere.json").exists())

    def test_stream_parser_handles_chunk_boundaries_and_malformed_input(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "result.json"
            document = {"alpha": 1, "domains": [{"id": index, "text": "é" * 40} for index in range(3000)], "omega": [1.5, None]}
            path.write_text(json.dumps(document))
            with patch.object(AUDIT.Stream, "more", autospec=True) as more:
                def small_reads(self):
                    self.buffer = self.buffer[self.cursor:]
                    self.cursor = 0
                    block = self.file.read(7)
                    self.hash.update(block)
                    self.buffer += self.utf8.decode(block, final=not block)
                    self.eof = not block
                more.side_effect = small_reads
                items = list(AUDIT.stream_walk(path))
            records = [item[1] for item in items if item[0] == "domain"]
            self.assertEqual([record["id"] for record in records], list(range(3000)))
            self.assertEqual({item[1]: item[2] for item in items if item[0] == "top"},
                             {"alpha": 1, "domains": "<streamed>", "omega": [1.5, None]})
            self.assertEqual(items[-1][0], "sha256")
            path.write_text('{"alpha": 1, "domains": [{"id": 0}, ], "omega": 2}')
            with self.assertRaises(ValueError):
                list(AUDIT.stream_walk(path))
            path.write_text('{"alpha": 1} trailing')
            with self.assertRaises(ValueError):
                list(AUDIT.stream_walk(path))



class G2UnionCoverTest(unittest.TestCase):
    """The audit's own G2' cover predicate against lattice-point enumeration."""

    def test_anchor_local_eligibility_is_not_transitive_closure(self):
        row = {"record_kind": "native_inspection", "local_inspection_finished": True,
               "frontiers": [], "error": None, "descendant_closed": False,
               "local_classification_discharged": False}
        self.assertTrue(AUDIT.g2_anchor_locally_eligible(row))
        for changed in ({"frontiers": [{"kind": "guard"}]}, {"error": "failure"},
                        {"rescue_abandoned": True}, {"local_inspection_finished": False}):
            self.assertFalse(AUDIT.g2_anchor_locally_eligible(dict(row, **changed)))
        row.update(record_kind="partial_initial_overlap_inspection", local_inspection_finished=False,
                   residual_inspection_finished=True)
        self.assertTrue(AUDIT.g2_anchor_locally_eligible(row))
        row.update(record_kind="g2_residual_anchor_inspection",
                   g2_residual_anchors={"residual_power_bounds": {"max_power_difference": 3}})
        self.assertTrue(AUDIT.g2_anchor_locally_eligible(row))
        row["g2_residual_anchors"]["residual_power_bounds"] = None
        self.assertFalse(AUDIT.g2_anchor_locally_eligible(row))

    def test_rescued_g2_keeps_late_taint_but_rejects_own_frontier_anchor(self):
        def mutate(top):
            anchor, loan = top["domains"][3], top["domains"][5]
            anchor.update(descendant_closed=False)
            loan.pop("initial_overlap")
            loan.update(record_kind="g2_residual_anchor_inspection", lower=[2, 0], upper=[2, 0],
                        descendant_closed=False, local_classification_discharged=False,
                        responsibility_status="discharged_by_residual_and_g2_anchors",
                        g2_residual_anchors={"mode": "union", "merge_stamp": 5, "snapshot_stamp": 5,
                            "coordinates_and_rank_unchanged": True, "residual_power_bounds": None,
                            "residual_pieces": 0, "anchors": [{"id": 3, "stamp": 3, "kind": "native"}]})
            top["partial_initial_inspections"] = 0
            top["delegation"].update(partial_initial_inspections=0, g2_records=1, g2_blocked=0)
            top["descendant_closure"].update(total_closed=4, unresolved_domains=3)

        with tempfile.TemporaryDirectory() as temporary:
            run = build_rescued_run(Path(temporary), mutate)
            request = json.loads((run / "request.json").read_text())
            request["command"] += ["--g2-residual-anchors", "union"]
            (run / "request.json").write_text(json.dumps(request))
            report = AUDIT.audit_walk(run)
            self.assertEqual(report["audit"], "PASS", report["violations"])
            self.assertEqual(report["g2_residual_anchor_checks"]["blocked_responsibility_records"], 0)
            top = json.loads((run / "result.json").read_text())
            top["domains"][3]["frontiers"] = [{"kind": "local_guard"}]
            (run / "result.json").write_text(json.dumps(top))
            report = AUDIT.audit_walk(run)
            self.assertEqual(report["audit"], "FAIL")
            self.assertTrue(any("G2' anchor 3 has no completed frontier-free" in failure
                                for failure in report["violations"]), report["violations"])

    def test_union_cover_matches_enumeration_on_random_boxes(self):
        import random
        rng = random.Random(5)
        owner = "101"

        def box():
            lower = tuple(rng.randrange(3) for _ in owner)
            upper = tuple(low + rng.randrange(4) for low in lower)
            least = rng.choice([None, rng.randrange(-2, 6)])
            most = rng.choice([None, rng.randrange(2, 9)])
            if least is not None and most is not None and least > most:
                least, most = most, least
            return (owner, "Apply", lower, upper, rng.randrange(5), 4 + rng.randrange(7), least, most)

        decided = {True: 0, False: 0}
        for _ in range(400):
            q = box()
            targets = [box() for _ in range(rng.randrange(1, 5))]
            if rng.randrange(3) == 0:
                # A residual D band of q plus q itself split in two bands.
                owner_, phase, lower, upper, rank, positive, least, most = q
                cut = rng.randrange(0, 10)
                targets = [AUDIT.d_band(q, None, cut - 1), AUDIT.d_band(q, cut, None)] + targets[:1]
            points = AUDIT.box_points(q, 10_000)
            self.assertIsNotNone(points)
            brute = all(any(AUDIT.box_member(t, p) for t in targets) for p in points)
            covered = AUDIT.union_covered(q, targets)
            self.assertEqual(covered, brute, (q, targets))
            decided[brute] += 1
        self.assertGreater(decided[True], 50)
        self.assertGreater(decided[False], 50)


def build_rescued_run(directory, mutate=None, amend=True):
    """A rescued walk: helper record 1 kept a frontier, the physics query phys-b
    it absorbed is certified through the amended helper's closed record 6."""
    directory = Path(directory)
    run = build_run(directory)
    result = run / "result.json"
    top = json.loads(result.read_text())
    queries_path = directory / "queries.json"
    queries = json.loads(queries_path.read_text())
    queries["queries"].append({"id": "phys-b", "owner": "01", "lower": [0, 1], "upper": [0, 3],
                               "max_numerator_rank": 1, "power_bounds": dict(POWER)})
    queries["query_roles"]["required"].append("phys-b")
    queries_path.write_text(json.dumps(queries, indent=1) + "\n")
    records = top["domains"]
    records[1].update(frontiers=[{"kind": "local_dispatch_frontier", "disposition": "Unresolved { x }"}],
                      local_classification_discharged=False, descendant_closed=False)
    records.append(native(6, "Apply", "01", [0, 1], [0, 5], 1, apply_stats(1, 0)))
    amendment = {"schema": AUDIT.AMENDMENT_SCHEMA, "sequence": 1, "parent": "b" * 64,
                 "queries": [{"id": "helper-rescue1", "owner": "01", "lower": [0, 1], "upper": [0, 5],
                              "max_numerator_rank": 1, "power_bounds": dict(POWER)}]}
    amendment_path = directory / "amendment-0001.json"
    amendment_path.write_text(json.dumps(amendment) + "\n")
    top.update(status="incomplete", all_scheduled_domains_resolved=False, frontiers=1, scheduled_nodes=7,
               processed_nodes=7, committed_domains=7, completed_nodes=6, native_processed_nodes=6, events=10,
               committed_events=10, contiguous_publication_watermark=7,
               inputs=[{"id": "root-a", "domain": 0}, {"id": "helper-b", "domain": 1}, {"id": "phys-b", "domain": 1},
                       {"id": "helper-rescue1", "domain": 6, "amendment": 1}],
               amendments=[{"sequence": 1, "digest": "a" * 64, "parent": "b" * 64, "queries": 1, "first_input": 3,
                            "first_domain": 6, "quarantined": 1, "resumed_generation": 2}],
               query_certification={"queries_total": 4, "queries_certified": 3})
    top["checkpoint"].update(committed_domains=7, completed_native_inspections=6, committed_events=10,
                             contiguous_publication_watermark=7)
    top["parallel"]["returned_inspections"] = 6
    top["delegation"].update(all_ledger_obligations_discharged=False, logical_publications=7,
                             native_publications=6, native_discharged=5, native_frontier_blocked=1)
    top["descendant_closure"].update(initial_closed=1, total_domains=7, total_closed=6, unresolved_domains=1)
    if mutate is not None:
        mutate(top)
    write_walk(result, run / "events.jsonl", directory / "checkpoint", queries, top)
    # A rescued walk that drained with quarantined frontiers exits 4.
    receipt = json.loads((run / "supervisor-result.json").read_text())
    receipt["exit_status"] = 4
    (run / "supervisor-result.json").write_text(json.dumps(receipt) + "\n")
    request = json.loads((run / "request.json").read_text())
    command = request["command"]
    command[command.index("--max-queries") + 1] = "3"
    command[command.index("--max-query-bytes") + 1] = str(queries_path.stat().st_size)
    command[command.index("--checkpoint")] = "--resume"
    if amend:
        command += ["--amend-queries", str(amendment_path)]
    (run / "request.json").write_text(json.dumps(request, indent=1) + "\n")
    return run


class RescuedWalkAuditTests(unittest.TestCase):
    def test_rescued_walk_certifies_physics_queries_through_amended_records(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = build_rescued_run(Path(temporary))
            report = AUDIT.audit_walk(run, require_closure=True)
            self.assertEqual(report["audit"], "PASS", report["violations"])
            rescue = report["certification"]["rescue"]
            self.assertEqual(rescue["physics_queries"], {"total": 2, "certified": 2,
                                                         "certified_through_amended_records": 1, "uncertified": []})
            self.assertEqual(rescue["helper_records"]["not_closed_ids"], [1])
            self.assertEqual(rescue["amendments"], 1)

    def test_rescued_walk_negative_controls(self):
        cases = {
            "amended record open": lambda top: top["domains"][6].update(descendant_closed=False),
            "frontier record claims closure": lambda top: top["domains"][1].update(descendant_closed=True),
            "chain count": lambda top: top["amendments"].append(dict(top["amendments"][0], sequence=2)),
            "amended input moved": lambda top: top["inputs"][3].update(domain=0),
        }
        for name, mutate in cases.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                run = build_rescued_run(Path(temporary), mutate)
                report = AUDIT.audit_walk(run, require_closure=True)
                self.assertEqual(report["audit"], "FAIL", name)
        with tempfile.TemporaryDirectory() as temporary:
            # The same result without the amendment in the command: frontiers are a violation.
            run = build_rescued_run(Path(temporary), amend=False)
            report = AUDIT.audit_walk(run, require_closure=True)
            self.assertEqual(report["audit"], "FAIL")
            self.assertTrue(any("nonzero frontiers" in v for v in report["violations"]), report["violations"])


if __name__ == "__main__":
    unittest.main()
