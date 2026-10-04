"""Cheap policy regressions. These fixtures do not claim native proof authority."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location("rule_optimizer_evaluate", Path(__file__).with_name("evaluate.py"))
E = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(E)


class EvaluatorTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.owner = self.root / "owner.rrbin"
        self.owner.write_bytes(b"test fixture, not a native owner")
        self.executable = self.root / "rustred"
        self.executable.write_bytes(b"fixture executable is never launched")
        selection = {"owner_count": 1, "total_bundle_bytes": self.owner.stat().st_size,
                     "owners": [{"mask": "10", "path": self.owner.name, "bytes": self.owner.stat().st_size}],
                     "family_fingerprint": "fixture", "initial_frontier_routes": [{"from": "01", "to": "10"}],
                     "route_record_count": 1, "load_limits": {}}
        self.write("selection.json", selection)
        self.write("queries.json", {"queries": [{"id": "required"}, {"id": "helper"}],
                                    "query_roles": {"required": ["required"], "auxiliary": ["helper"]}})
        command = ["/usr/bin/python3", "/repo/examples/python/shared_owner_campaign.py"]
        for flag, value in {"executable": "old", "manifest": "old", "queries": "old", "owner-base": "old",
                            "run-directory": "old", "checkpoint": "old", "max-queries": "2", "max-query-bytes": "200",
                            "workers": "2", "cpus": "0,1", "max-memory-bytes": "1000000000",
                            "host-memory-reserve-bytes": "1000000000"}.items():
            command.extend(("--" + flag, value))
        command.append("--unbounded-work")
        self.write("template.json", {"command": command})
        native = [str(self.executable), "owner-domain-match"]
        for flag in ("manifest", "owner-base", "output", "events", "stop-file", "queries", "checkpoint", "max-queries", "max-query-bytes"):
            native.extend(("--" + flag, "old"))
        native.extend(("--workers", "2", "--follow-successors", "--unbounded-work"))
        self.write("native-template.json", {"command": native})
        self.request = {"schema": E.SCHEMA, "cohort": "fixture", "destination": str(self.root / "planned"),
                        "baseline": {"selection": self.pin("selection.json"), "queries": self.pin("queries.json"),
                                     "owner_base": str(self.root)},
                        "executable": self.pin("rustred"), "walk_template": self.pin("template.json"),
                        "native_template": self.pin("native-template.json"),
                        "criterion": {"primary_metric": "scheduled_nodes", "minimum_relative_gain": 0.2,
                                      "regression_limits": {key: 1.1 for key in ("scheduled_nodes", "native_processed_nodes", "events", "traversal_seconds", "complete_arm_seconds")}},
                        "arm_order": ["baseline", "candidate"], "replacements": [],
                        "budget": {"inclusive_seconds": 1800, "work_mode": "complete", "phase_deadlines": [
                            {"stage": [20, 30], "walk": [400, 430], "cold": [800, 830]},
                            {"stage": [850, 860], "walk": [1300, 1330], "cold": [1740, 1770]}]}}

    def write(self, name, value):
        (self.root / name).write_text(json.dumps(value))

    def pin(self, name):
        path = self.root / name
        return {"path": str(path), "sha256": E.sha256(path)}

    def native_documents(self, evaluation_plan, name):
        expected = evaluation_plan["arms"][name]
        checkpoint = {"pending_domains": 0, "paused": False, "abandoned_obligations": 0,
                      "directory": E.option(expected["expected_native_command"], "--checkpoint"), "generation": 1}
        phase = {"exit_code": 0, "stop_reason": None, "failure": None,
                 "hard_stopped": False, "owned_groups_drained": True}
        # Actual completed CP6 walks can retain an old unresolved closure scan.
        walk = {"status": "incomplete", "all_scheduled_domains_resolved": False,
                "checkpoint": checkpoint, "error": None, "frontiers": 0, "admission_complete": True,
                "admission_failure": None, "failed_nodes": 0, "observer_failed": False,
                "operational_stop": None, "stop_reason": None, "input_frontiers_count": 0,
                "abandoned_obligations": 0, "parallel": {"workers_joined": True},
                "scheduled_nodes": 100, "native_processed_nodes": 80, "events": 400, "traversal_seconds": 2.0}
        command = list(expected["expected_native_command"])
        measured = {"phases": {key: dict(phase) for key in E.PHASES}, "arm_seconds": 4.0,
                    "executable_sha256_before": self.request["executable"]["sha256"],
                    "executable_sha256_after": self.request["executable"]["sha256"]}
        measured["phases"]["walk"]["exit_code"] = 4
        return {"selection": copy.deepcopy(expected["selection"]),
                "input_receipt": {"query_bytes_unchanged": True, "queries_sha256": evaluation_plan["queries"]["sha256"],
                                  **copy.deepcopy(expected["inventory"])},
                "native_request": {"command": command, "resume_requested": False, "amend_queries": [],
                                   "workers": 2, "cpus": [0, 1], "hard_memory_bytes": 1000000000,
                                   "effective_hard_memory_bytes": 1000000000, "host_memory_reserve_bytes": 1000000000},
                "walk": walk, "supervisor": {"checkpoint": checkpoint, "operator_or_resource_stop": None,
                                               "native_stop_reason": None, "hard_stopped": False},
                "cold": {"verdict": "PASS", "violations": [], "violations_suppressed": 0,
                         "closure_required": True, "certification_scope": "all_roots",
                         "reference": {"native_levers": "Off"},
                         "checkpoint": {"owner_digests_match": True, "request_binding_matches": True,
                                        "directory": checkpoint["directory"], "generation": 1},
                         "queries": {"count": 2, "unadmitted": 0, "unresolved_input_frontiers": 0},
                         "certification": {"classes": {"physics": {"total": 1, "independently_verified": 1},
                                                       "auxiliary": {"total": 1, "independently_verified": 1}}},
                         "counts": {"roots": 2, "roots_independently_verified": 2},
                         "reinspection": {"complete": True, "mode": "All", "candidates": 80, "selected": 80,
                                          "tally": {"errors": 0, "frontiers": 0, "uncovered": 0,
                                                    "uncovered_after_reference_error": 0}}}, "measurement": measured}

    def results(self):
        planned = E.plan(self.request)
        return planned, {name: self.native_documents(planned, name) for name in E.ARMS}

    def test_aa_is_fresh_shared_and_keeps_incumbent(self):
        planned, documents = self.results()
        self.assertEqual(planned["arms"]["baseline"]["selection"], planned["arms"]["candidate"]["selection"])
        self.assertNotEqual(planned["arms"]["baseline"]["commands"]["walk"], planned["arms"]["candidate"]["commands"]["walk"])
        self.assertFalse(Path(self.request["destination"]).exists())
        result = E.compare(planned, documents)
        self.assertTrue(result["completed_comparison"])
        self.assertTrue(result["retain_baseline"])
        self.assertFalse(result["cohort_target_observed"])
        self.assertEqual(set(result["candidate_over_baseline"].values()), {1.0})

    def test_plan_summary_omits_large_payloads_without_changing_plan(self):
        planned = E.plan(self.request)
        planned["arms"]["baseline"]["selection"]["family_fingerprint"] = "large payload" * 10000
        original = copy.deepcopy(planned)
        result = E.plan_summary(planned, written=True)
        self.assertEqual(result["plan_file"], str(self.root / "planned" / "plan.json"))
        self.assertEqual(result["arms"]["baseline"], {"owners": 1, "routes": 1, "overlays": 0})
        self.assertEqual(result["query_role_counts"], {"required": 1, "auxiliary": 1})
        self.assertNotIn("large payload", json.dumps(result))
        self.assertLess(len(json.dumps(result)), 4096)
        self.assertEqual(planned, original)
        self.assertIsNone(E.plan_summary(planned)["plan_file"])

    def prefer(self):
        path = self.root / "preferred.rrbin"
        path.write_bytes(b"preferred program fixture")
        self.write("provenance.json", {"note": "fixture only, not source authority"})
        self.request["preferred_programs"] = [{
            **self.pin("preferred.rrbin"), "owner_mask": "10",
            "residual_policy": "defer-to-baseline",
            "source_provenance": [self.pin("provenance.json")]}]

    def test_preference_keeps_baseline_payload_and_is_a_distinct_treatment(self):
        self.prefer()
        planned, documents = self.results()
        base, candidate = (planned["arms"][name] for name in E.ARMS)
        self.assertEqual(base["selection"]["owners"], candidate["selection"]["owners"])
        self.assertNotIn("preferred_owner_programs", base["selection"])
        self.assertEqual(candidate["selection"]["preferred_owner_programs"][0]["residual_policy"],
                         "defer-to-baseline")
        result = E.compare(planned, documents)
        self.assertTrue(result["completed_comparison"])
        self.assertEqual(result["treatment"], "changed_owner_payload")
        self.assertFalse(result["cohort_target_observed"])
        self.assertEqual(E.plan_summary(planned)["arms"]["candidate"]["preferred_programs"], 1)

    def test_preferred_receipt_and_policy_are_part_of_comparison(self):
        self.prefer()
        for mutation in ("missing", "policy", "digest", "path"):
            with self.subTest(mutation=mutation):
                planned, documents = self.results()
                row = documents["candidate"]["input_receipt"]["preferred_owner_programs"][0]
                if mutation == "missing":
                    del documents["candidate"]["input_receipt"]["preferred_owner_programs"]
                elif mutation == "policy":
                    row["residual_policy"] = "accept-new-terminals"
                elif mutation == "digest":
                    row["sha256"] = "changed"
                else:
                    # Staging is allowed to change only payload paths.
                    documents["candidate"]["selection"]["preferred_owner_programs"][0]["path"] = "preferred/relative.rrbin"
                self.assertEqual(E.compare(planned, documents)["completed_comparison"], mutation == "path")

    def test_bad_preferred_declarations_fail_before_execution(self):
        self.prefer()
        original = copy.deepcopy(self.request)
        for mutation in ("policy", "missing-owner", "duplicate", "replacement", "digest", "proof"):
            with self.subTest(mutation=mutation):
                self.request = copy.deepcopy(original)
                row = self.request["preferred_programs"][0]
                if mutation == "policy":
                    row["residual_policy"] = "accept-new-terminals"
                elif mutation == "missing-owner":
                    row["owner_mask"] = "11"
                elif mutation == "duplicate":
                    self.request["preferred_programs"].append(dict(row))
                elif mutation == "replacement":
                    self.request["replacements"] = [dict(row)]
                elif mutation == "digest":
                    row["sha256"] = "changed"
                else:
                    row["source_provenance"] = []
                with self.assertRaises(ValueError):
                    E.plan(self.request)

    def test_preferred_rule_subset_is_preserved_and_bound(self):
        self.prefer()
        for ordinals in (None, [], [0, 110, 464]):
            with self.subTest(ordinals=ordinals):
                self.request["preferred_programs"][0]["rule_ordinals"] = ordinals
                planned, documents = self.results()
                candidate = planned["arms"]["candidate"]
                self.assertEqual(candidate["selection"]["preferred_owner_programs"][0]["rule_ordinals"], ordinals)
                self.assertEqual(candidate["inventory"]["preferred_owner_programs"][0]["rule_ordinals"], ordinals)
                self.assertTrue(E.compare(planned, documents)["completed_comparison"])
                receipt = documents["candidate"]["input_receipt"]["preferred_owner_programs"][0]
                receipt["rule_ordinals"] = [1] if ordinals != [1] else []
                self.assertFalse(E.compare(planned, documents)["completed_comparison"])
                receipt["rule_ordinals"] = ordinals
                documents["candidate"]["selection"]["preferred_owner_programs"][0]["rule_ordinals"] = [1]
                self.assertFalse(E.compare(planned, documents)["completed_comparison"])

    def test_preferred_rule_subset_rejects_noncanonical_metadata(self):
        self.prefer()
        for ordinals in (False, 1, "110", [-1], [True], [1.0], [1 << 64], [1, 1], [2, 1]):
            with self.subTest(ordinals=ordinals):
                self.request["preferred_programs"][0]["rule_ordinals"] = ordinals
                with self.assertRaisesRegex(ValueError, "rule_ordinals"):
                    E.plan(self.request)

    def test_existing_preferred_subset_inventory_is_preserved(self):
        self.prefer()
        self.request["preferred_programs"][0]["rule_ordinals"] = [110]
        preferred_selection = E.plan(self.request)["arms"]["candidate"]["selection"]
        self.write("selection.json", preferred_selection)
        self.request["baseline"]["selection"] = self.pin("selection.json")
        self.request["preferred_programs"] = []
        planned, documents = self.results()
        self.assertEqual(planned["arms"]["baseline"]["inventory"], planned["arms"]["candidate"]["inventory"])
        self.assertTrue(E.compare(planned, documents)["completed_comparison"])
        self.assertEqual(planned["arms"]["baseline"]["inventory"]["preferred_owner_programs"][0]["rule_ordinals"], [110])

    def test_known_regression_not_target(self):
        planned, documents = self.results()
        documents["baseline"]["walk"].update(scheduled_nodes=26025, native_processed_nodes=17957)
        documents["candidate"]["walk"].update(scheduled_nodes=26211, native_processed_nodes=18000)
        documents["baseline"]["measurement"]["arm_seconds"] = 15.508817577268928
        documents["candidate"]["measurement"]["arm_seconds"] = 15.796
        result = E.compare(planned, documents)
        self.assertTrue(result["completed_comparison"])
        self.assertFalse(result["cohort_target_observed"])

    def test_censored_fast_arm_never_wins(self):
        planned, documents = self.results()
        documents["candidate"]["walk"].update(scheduled_nodes=1, native_processed_nodes=1, traversal_seconds=0.01)
        documents["candidate"]["supervisor"]["checkpoint"]["pending_domains"] = 1000
        documents["candidate"]["supervisor"]["operator_or_resource_stop"] = "deadline"
        result = E.compare(planned, documents)
        self.assertFalse(result["completed_comparison"])
        self.assertIsNone(result["candidate_over_baseline"])
        self.assertTrue(result["retain_baseline"])

    def test_missing_completion_fields_fail_closed(self):
        for document, field in (("cold", "reinspection"), ("supervisor", "checkpoint"),
                                ("walk", "frontiers"), ("measurement", "phases")):
            with self.subTest(document=document, field=field):
                planned, documents = self.results()
                del documents["candidate"][document][field]
                self.assertFalse(E.compare(planned, documents)["completed_comparison"])

    def test_actual_checkpoint_only_schema_omits_optional_error(self):
        # Reduced from aa-whole58-v2/baseline/run/result.json (8ef80b52 CLI).
        planned, documents = self.results()
        for name in E.ARMS:
            walk = documents[name]["walk"]
            del walk["error"]
            walk.update(status="incomplete", all_scheduled_domains_resolved=False,
                        admission_complete=True, admission_failure=None, failed_nodes=0,
                        observer_failed=False, operational_stop=None, stop_reason=None,
                        input_frontiers_count=0, queued_nodes=0,
                        record_inventory="authenticated_checkpoint_segments_only")
        self.assertTrue(E.compare(planned, documents)["completed_comparison"])
        del documents["candidate"]["walk"]["failed_nodes"]
        self.assertFalse(E.compare(planned, documents)["completed_comparison"])

    def test_routes_settings_helper_and_binary_mismatch(self):
        for change in ("routes", "helper", "binary", "settings", "resume", "workers"):
            with self.subTest(change=change):
                planned, documents = self.results()
                candidate = documents["candidate"]
                if change == "routes":
                    candidate["selection"]["initial_frontier_routes"] = []
                elif change == "helper":
                    candidate["cold"]["certification"]["classes"]["auxiliary"]["independently_verified"] = 0
                elif change == "binary":
                    candidate["measurement"]["executable_sha256_after"] = "bad"
                elif change == "settings":
                    candidate["native_request"]["command"].extend(("--route-domain-overcover",))
                elif change == "resume":
                    candidate["native_request"]["resume_requested"] = True
                else:
                    candidate["native_request"]["workers"] = 4
                self.assertFalse(E.compare(planned, documents)["completed_comparison"])

    def test_work_gain_with_offsetting_cost_regression_is_rejected(self):
        planned, documents = self.results()
        # A distinct treatment is required before an improvement is possible.
        planned["arms"]["candidate"]["inventory"]["owners"][0]["sha256"] = "changed"
        documents["candidate"]["input_receipt"]["owners"][0]["sha256"] = "changed"
        documents["candidate"]["walk"].update(scheduled_nodes=70, native_processed_nodes=60, events=300)
        documents["candidate"]["measurement"]["arm_seconds"] = 5
        self.assertFalse(E.compare(planned, documents)["cohort_target_observed"])
        documents["candidate"]["measurement"]["arm_seconds"] = 4
        result = E.compare(planned, documents)
        self.assertTrue(result["cohort_target_observed"])
        self.assertFalse(result["promotion_authorized"])

    def test_aa_timing_noise_cannot_be_an_improvement(self):
        planned, documents = self.results()
        planned["criterion"]["primary_metric"] = "complete_arm_seconds"
        documents["candidate"]["measurement"]["arm_seconds"] = 1
        result = E.compare(planned, documents)
        self.assertTrue(result["completed_comparison"])
        self.assertFalse(result["cohort_target_observed"])
        self.assertEqual(result["treatment"], "unchanged_payload_aa_control")

    def test_same_native_settings_deviation_in_both_arms_is_rejected(self):
        planned, documents = self.results()
        for name in E.ARMS:
            documents[name]["native_request"]["command"].append("--route-domain-overcover")
        self.assertFalse(E.compare(planned, documents)["completed_comparison"])

    def test_actual_paths_cold_generation_and_memory_are_bound_to_each_arm(self):
        for change in ("native_path", "cold_path", "cold_generation", "missing_generation", "memory"):
            with self.subTest(change=change):
                planned, documents = self.results()
                candidate = documents["candidate"]
                if change == "native_path":
                    E.replace_option(candidate["native_request"]["command"], "--manifest",
                                     E.option(documents["baseline"]["native_request"]["command"], "--manifest"))
                elif change == "cold_path":
                    candidate["cold"]["checkpoint"] = copy.deepcopy(documents["baseline"]["cold"]["checkpoint"])
                elif change == "cold_generation":
                    candidate["cold"]["checkpoint"]["generation"] = 2
                elif change == "missing_generation":
                    del candidate["cold"]["checkpoint"]["generation"]
                else:
                    candidate["native_request"]["hard_memory_bytes"] *= 2
                self.assertFalse(E.compare(planned, documents)["completed_comparison"])

    def test_cpu_range_uses_existing_supervisor_parser(self):
        template = E.read_json(self.root / "template.json")
        E.replace_option(template["command"], "--cpus", "0-1")
        self.write("template.json", template)
        self.request["walk_template"] = self.pin("template.json")
        planned, documents = self.results()
        self.assertTrue(E.compare(planned, documents)["completed_comparison"])

    def test_explicit_work_cap_and_pilot_deadlines(self):
        self.request["budget"].update(work_mode="bounded", max_domains=2000)
        planned = E.plan(self.request)
        for arm in planned["arms"].values():
            command = arm["commands"]["walk"]
            self.assertNotIn("--unbounded-work", command)
            self.assertEqual(E.option(command, "--max-domains"), "2000")
        self.request["budget"]["inclusive_seconds"] = 1801
        with self.assertRaises(ValueError):
            E.plan(self.request)

    def test_replacement_keeps_routes_roles_suffix_metadata(self):
        donor = self.root / "donor.rrbin"
        donor.write_bytes(b"fixture replacement")
        self.write("source-proof.json", {"notice": "this file is provenance, not Python proof authority"})
        self.request["replacements"] = [{"owner_mask": "10", **self.pin("donor.rrbin"),
                                         "source_provenance": [self.pin("source-proof.json")]}]
        planned = E.plan(self.request)
        self.assertEqual(planned["arms"]["baseline"]["selection"]["initial_frontier_routes"],
                         planned["arms"]["candidate"]["selection"]["initial_frontier_routes"])
        self.assertEqual(planned["query_roles"]["auxiliary"], ["helper"])
        self.assertNotEqual(planned["arms"]["baseline"]["inventory"], planned["arms"]["candidate"]["inventory"])

    def test_bad_digest_duplicate_json_and_invalid_roles(self):
        self.request["executable"]["sha256"] = "bad"
        with self.assertRaises(ValueError):
            E.plan(self.request)
        with self.assertRaises(ValueError):
            json.loads('{"x":1,"x":2}', object_pairs_hook=E.unique_object)

    def overlay_replacement(self):
        for name in ("old-overlay.rrbin", "new-overlay.rrbin", "other-overlay.rrbin", "donor.rrbin"):
            (self.root / name).write_bytes(name.encode())
        self.write("overlay-proof.json", {"note": "fixture provenance, not native authority"})
        selection = E.read_json(self.root / "selection.json")
        # An unaffected row is deliberately first, so order/index are tested.
        selection["domain_rule_overlays"] = [
            {"owner_mask": "01", "path": "other-overlay.rrbin",
             "bytes": (self.root / "other-overlay.rrbin").stat().st_size},
            {"owner_mask": "10", "path": "old-overlay.rrbin", "note": "retain metadata",
             "bytes": (self.root / "old-overlay.rrbin").stat().st_size},
        ]
        self.write("selection.json", selection)
        self.request["baseline"]["selection"] = self.pin("selection.json")
        self.request["replacements"] = [{"owner_mask": "10", **self.pin("donor.rrbin"),
                                         "source_provenance": [self.pin("overlay-proof.json")]}]
        self.request["overlay_replacements"] = [{"owner_mask": "10", "overlay_index": 1,
            "original_sha256": self.pin("old-overlay.rrbin")["sha256"],
            **self.pin("new-overlay.rrbin"), "source_provenance": [self.pin("overlay-proof.json")]}]

    def test_explicit_overlay_reexport_keeps_inventory_order_and_unchanged_rows(self):
        self.overlay_replacement()
        planned, documents = self.results()
        before = planned["arms"]["baseline"]["selection"]["domain_rule_overlays"]
        after = planned["arms"]["candidate"]["selection"]["domain_rule_overlays"]
        self.assertEqual(len(before), len(after))
        self.assertEqual(before[0], after[0])
        self.assertEqual(after[1]["note"], "retain metadata")
        self.assertEqual(after[1]["owner_mask"], before[1]["owner_mask"])
        self.assertEqual(before[1]["path"], str(self.root / "old-overlay.rrbin"))
        self.assertEqual(after[1]["path"], str(self.root / "new-overlay.rrbin"))
        self.assertEqual(planned["overlay_replacements"], self.request["overlay_replacements"])
        self.assertTrue(E.compare(planned, documents)["completed_comparison"])
        # A native cold failure must still reject correctly staged replacements.
        documents["candidate"]["cold"]["verdict"] = "FAIL"
        self.assertFalse(E.compare(planned, documents)["completed_comparison"])

    def test_overlay_reexport_rejects_missing_duplicate_unpaired_and_bad_bindings(self):
        self.overlay_replacement()
        original = copy.deepcopy(self.request)
        for mutation in ("missing", "duplicate", "unpaired", "wrong-index", "bool-index",
                         "owner", "old-digest", "new-digest", "proof", "proof-digest"):
            with self.subTest(mutation=mutation):
                self.request = copy.deepcopy(original)
                row = self.request["overlay_replacements"][0]
                if mutation == "missing":
                    self.request["overlay_replacements"] = []
                elif mutation == "duplicate":
                    self.request["overlay_replacements"].append(dict(row))
                elif mutation == "unpaired":
                    self.request["replacements"] = []
                elif mutation == "wrong-index":
                    row["overlay_index"] = 0
                elif mutation == "bool-index":
                    row["overlay_index"] = True
                elif mutation == "owner":
                    row["owner_mask"] = "01"
                elif mutation == "old-digest":
                    row["original_sha256"] = "wrong"
                elif mutation == "new-digest":
                    row["sha256"] = "wrong"
                elif mutation == "proof":
                    row["source_provenance"] = []
                else:
                    row["source_provenance"][0]["sha256"] = "wrong"
                with self.assertRaises(ValueError):
                    E.plan(self.request)

    def test_multiple_same_owner_overlays_require_complete_explicit_reexports(self):
        self.overlay_replacement()
        selection = E.read_json(self.root / "selection.json")
        selection["domain_rule_overlays"].append(copy.deepcopy(selection["domain_rule_overlays"][1]))
        self.write("selection.json", selection)
        self.request["baseline"]["selection"] = self.pin("selection.json")
        with self.assertRaises(ValueError):
            E.plan(self.request)
        second = copy.deepcopy(self.request["overlay_replacements"][0])
        second["overlay_index"] = 2
        self.request["overlay_replacements"].append(second)
        self.assertEqual(len(E.plan(self.request)["arms"]["candidate"]["inventory"]["domain_rule_overlays"]), 3)

    def test_overlay_reexport_staging_cannot_drop_or_exchange_rows(self):
        self.overlay_replacement()
        for mutation in ("drop", "reorder", "old-payload", "metadata"):
            with self.subTest(mutation=mutation):
                planned, documents = self.results()
                rows = documents["candidate"]["input_receipt"]["domain_rule_overlays"]
                if mutation == "drop":
                    rows.pop()
                elif mutation == "reorder":
                    rows.reverse()
                elif mutation == "old-payload":
                    rows[1]["sha256"] = self.pin("old-overlay.rrbin")["sha256"]
                else:
                    documents["candidate"]["selection"]["domain_rule_overlays"][1]["note"] = "changed"
                self.assertFalse(E.compare(planned, documents)["completed_comparison"])


if __name__ == "__main__":
    unittest.main()
