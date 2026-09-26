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
    return {"schema": "rustred.owner-domain-queries.json.v2", "queries": [
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
                              "residual_power_bounds": dict(POWER)}),
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

        root, helper = {"id": "root-a", "domain": 0}, {"id": "helper-b", "domain": 1}
        cases = [("query root-a: upper changed", [], None, wider_record),
                 ("initial entry obligations not discharged", [(alias_query("phys-c"), 0)], None,
                  query_count_as_initial),
                 # The first query naming a record admitted it, so it must equal it.
                 ("query phys-c: lower changed", [(alias_query("phys-c"), 0)],
                  [{"id": "phys-c", "domain": 0}, root, helper], None),
                 # An alias targets an initial record, never a later successor.
                 ("inputs do not map every query to an initial record", [(alias_query("phys-c"), 3)], None, None),
                 ("query phys-c: no initial record", [(alias_query("phys-c"), 3)], None, None)]
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

    def test_inputs_must_follow_the_query_document_order(self):
        aliases = [(alias_query("phys-c"), 0)]
        inputs = [{"id": "root-a", "domain": 0}, {"id": "phys-c", "domain": 0}, {"id": "helper-b", "domain": 1}]
        with tempfile.TemporaryDirectory() as temporary:
            report = AUDIT.audit_walk(build_aliased_run(Path(temporary), aliases, inputs))
            self.assertEqual(report["violations"], ["inputs are not in query document order"])

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


if __name__ == "__main__":
    unittest.main()
