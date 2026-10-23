"""Scratch-only phase transitions; no production runs or algebra in Python."""
import importlib.util
import hashlib
import io
import json
import os
from pathlib import Path
import re
import signal
import sys
import tempfile
import threading
import unittest
from types import SimpleNamespace
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


PHASES = module("campaign_phases")
PRODUCTION = module("production_saved_owner_campaign")
TELEMETRY = module("campaign_telemetry")
DASHBOARD = module("campaign_dashboard")
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
ROOT = Path(__file__).resolve().parents[2]


class PhaseTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "TMP").mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix="campaign-phases-test-", dir=ROOT / "TMP")
        self.addCleanup(self.temp.cleanup)
        self.campaign = Path(self.temp.name)
        self.checkpoint = self.campaign / "checkpoints/main"
        for name in ("inputs", "checkpoints/main", "runs/first", "amendments"):
            (self.campaign / name).mkdir(parents=True, exist_ok=True)
        self.write("inputs/selection.json", {"owners": []})
        self.write("inputs/queries.json", {"queries": ["original"]})
        self.write("checkpoints/main/latest.json", {"manifest": {
            "format": "RUSTRED-WALK-CP6", "generation": 1, "resumable": True}, "blake3": [0] * 32})
        self.command = ["rustred", "owner-domain-match", "--queries", str(self.campaign / "inputs/queries.json"),
                        "--manifest", str(self.campaign / "inputs/selection.json")]
        self.write("runs/first/request.json", {"command": self.command})
        self.result = {"recursive_worklist_exhausted": True, "admission_complete": True,
                       "admission_failure": None, "failed_nodes": 0, "frontiers": 0,
                       "checkpoint": {"state": "saved", "generation": 1, "resumable": True,
                                      "directory": str(self.checkpoint), "manifest_blake3": "00" * 32}}
        self.write("runs/first/result.json", self.result)
        self.driver = SimpleNamespace(campaign_runs=lambda _: [self.campaign / "runs/first"],
                                      campaign_run_liveness=lambda _: [], write_json=PRODUCTION.write_json,
                                      checkpoint_lock=PRODUCTION.checkpoint_lock)
        self.plan = {"campaign_directory": str(self.campaign), "checkpoint_directory": str(self.checkpoint),
                     "run_directory": str(self.campaign / "runs/next"), "command": ["supervisor"]}
        self.policy = PHASES.configuration(self.campaign)

    def write(self, name, value):
        path = self.campaign / name
        path.parent.mkdir(parents=True, exist_ok=True)
        PRODUCTION.write_json(path, value)

    def binding(self):
        return PHASES.scope_binding(self.campaign, self.checkpoint)

    def ready(self):
        return PHASES.completed_request(self.campaign, self.checkpoint, self.binding(), self.driver)

    def test_default_publishes_and_legacy_opt_in_never_automatically_refines(self):
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")

        self.write("master-reduction/policy.json", {**self.policy, "operation": "refine"})
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        deeper = PHASES.configuration(self.campaign, enabled=True, seed_depth=1)
        self.assertEqual(deeper["seed_depth"], 1)
        self.assertEqual(deeper["operation"], "refine")
        self.assertNotEqual(self.binding()["key"], PHASES.scope_binding(self.campaign, self.checkpoint, 1, "refine")["key"])
        self.write("master-reduction/policy.json", deeper)
        with self.assertRaisesRegex(ValueError, "cannot lower"):
            PHASES.configuration(self.campaign, enabled=True, seed_depth=0)
        with self.assertRaisesRegex(ValueError, "explicit"):
            PHASES.configuration(self.campaign, seed_depth=1)
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")

    def test_relation_column_budget_is_optional_operational_and_removable(self):
        self.assertIsNone(self.policy["max_relation_columns"])
        finite = PHASES.configuration(self.campaign, enabled=True, max_relation_columns=40)
        self.write("master-reduction/policy.json", finite)
        self.assertEqual(PHASES.configuration(self.campaign, enabled=True)["max_relation_columns"], 40)
        self.assertEqual(PHASES.configuration(self.campaign, enabled=True, max_relation_columns=0)["max_relation_columns"], 0)
        for bad in (-1, True, "100"):
            with self.assertRaises(ValueError):
                PHASES.configuration(self.campaign, enabled=True, max_relation_columns=bad)
        with self.assertRaisesRegex(ValueError, "explicit"):
            PHASES.configuration(self.campaign, max_relation_columns=40)

    def test_fresh_search_discards_search_preferences_but_retains_resources(self):
        old = PHASES.configuration(self.campaign, enabled=True, seed_depth=2, containing_sector_depth=2,
                                   saved_rule_assistance=True, circuit_symmetry_assistance=True,
                                   finite_feedback=False, normalization_profile="standard",
                                   collection_artifacts=[self.campaign / "peer"], max_relation_columns=0,
                                   refinement_workers=2, refinement_cpus="0,1", relation_batch_rows=128)
        self.write("master-reduction/policy.json", {**old, "modular_policy": {"batch_rows": 7}})
        for containing in (1, 0):
            with self.subTest(containing=containing):
                with self.assertRaisesRegex(ValueError, "cannot lower"):
                    PHASES.configuration(self.campaign, enabled=True, seed_depth=3,
                                         containing_sector_depth=containing)
                fresh = PHASES.configuration(self.campaign, enabled=True, fresh_search=True, seed_depth=3,
                                             containing_sector_depth=containing)
                self.assertEqual((fresh["seed_depth"], fresh["containing_sector_depth"]), (3, containing))
                self.assertEqual(fresh["refinement_strategy"], "exact")
                self.assertFalse(fresh["saved_rule_assistance"])
                self.assertFalse(fresh["circuit_symmetry_assistance"])
                self.assertTrue(fresh["finite_feedback"])
                self.assertEqual(fresh["collection_artifacts"], [])
                self.assertNotIn("normalization_profile", fresh)
                self.assertNotIn("modular_policy", fresh)
                self.assertEqual((fresh["max_relation_columns"], fresh["refinement_workers"],
                                  fresh["refinement_cpus"], fresh["relation_batch_rows"]), (0, 2, "0,1", 128))
        default = PHASES.configuration(self.campaign, enabled=True, fresh_search=True)
        self.assertEqual((default["seed_depth"], default["containing_sector_depth"]), (0, 0))
        for invalid in (None, 1, "yes"):
            with self.assertRaisesRegex(ValueError, "boolean"):
                PHASES.configuration(self.campaign, enabled=True, fresh_search=invalid)
        with self.assertRaisesRegex(ValueError, "explicit refinement"):
            PHASES.configuration(self.campaign, fresh_search=True)

    def test_production_fresh_search_reaches_configuration_and_is_refine_only(self):
        with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
            with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters",
                                 "--master-fresh-search", "--master-seed-depth", "3",
                                 "--master-containing-sector-depth", "0"])
        self.assertTrue(configure.call_args.kwargs["fresh_search"])
        self.assertEqual(configure.call_args.args[2], 3)
        self.assertEqual(configure.call_args.kwargs["containing_sector_depth"], 0)
        with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
            PRODUCTION.main(["--campaign-directory", str(self.campaign), "--publish-only", "--master-fresh-search"])

    def test_numerator_symmetry_preference_is_exact_only_and_fresh_round_resets_it(self):
        self.assertFalse(self.policy["numerator_symmetry_assistance"])
        enabled = PHASES.configuration(self.campaign, enabled=True, numerator_symmetry_assistance=True)
        self.write("master-reduction/policy.json", enabled)
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True)["numerator_symmetry_assistance"])
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        fresh = PHASES.configuration(self.campaign, enabled=True, fresh_search=True)
        self.assertFalse(fresh["numerator_symmetry_assistance"])
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True, fresh_search=True,
                                            numerator_symmetry_assistance=True)["numerator_symmetry_assistance"])
        with self.assertRaisesRegex(ValueError, "requires exact refinement"):
            PHASES.configuration(self.campaign, enabled=True, refinement_strategy="modular-circuits")
        modular = PHASES.configuration(self.campaign, enabled=True, refinement_strategy="modular-circuits",
                                       numerator_symmetry_assistance=False)
        self.assertFalse(modular["numerator_symmetry_assistance"])
        for value in (True, False):
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, numerator_symmetry_assistance=value)
        for value in (0, "yes"):
            with self.assertRaisesRegex(ValueError, "boolean"):
                PHASES.configuration(self.campaign, enabled=True, numerator_symmetry_assistance=value)
        self.write("master-reduction/policy.json", {**enabled, "numerator_symmetry_assistance": 1})
        with self.assertRaisesRegex(ValueError, "invalid persisted"):
            PHASES.configuration(self.campaign, enabled=True)

    def test_production_numerator_symmetry_flag_reaches_configuration(self):
        for flag, expected in (("--master-numerator-symmetry-assistance", True),
                               ("--no-master-numerator-symmetry-assistance", False)):
            with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters", flag])
            self.assertEqual(configure.call_args.kwargs["numerator_symmetry_assistance"], expected)

    def test_sector_symmetry_preference_is_explicit_exact_and_fresh_round_resets_it(self):
        self.assertFalse(self.policy["sector_symmetry_assistance"])
        enabled = PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=True,
                                       numerator_symmetry_assistance=True)
        self.write("master-reduction/policy.json", enabled)
        resumed = PHASES.configuration(self.campaign, enabled=True)
        self.assertTrue(resumed["sector_symmetry_assistance"])
        self.assertTrue(resumed["numerator_symmetry_assistance"])
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        fresh = PHASES.configuration(self.campaign, enabled=True, fresh_search=True)
        self.assertFalse(fresh["sector_symmetry_assistance"])
        self.assertFalse(fresh["numerator_symmetry_assistance"])
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True, fresh_search=True,
                                            sector_symmetry_assistance=True)["sector_symmetry_assistance"])
        with self.assertRaisesRegex(ValueError, "sector-symmetry assistance requires exact refinement"):
            PHASES.configuration(self.campaign, enabled=True, refinement_strategy="modular-circuits",
                                 numerator_symmetry_assistance=False)
        disabled = PHASES.configuration(self.campaign, enabled=True, refinement_strategy="modular-circuits",
                                        numerator_symmetry_assistance=False, sector_symmetry_assistance=False)
        self.assertFalse(disabled["sector_symmetry_assistance"])
        for value in (True, False):
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, sector_symmetry_assistance=value)
        for value in (0, "yes"):
            with self.assertRaisesRegex(ValueError, "boolean"):
                PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=value)
        self.write("master-reduction/policy.json", {**enabled, "sector_symmetry_assistance": 1})
        with self.assertRaisesRegex(ValueError, "invalid persisted"):
            PHASES.configuration(self.campaign, enabled=True)

    def test_production_sector_symmetry_flag_reaches_configuration_with_legacy_choice(self):
        for flag, expected in (("--master-sector-symmetry-assistance", True),
                               ("--no-master-sector-symmetry-assistance", False)):
            with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters", flag,
                                     "--master-numerator-symmetry-assistance"])
            self.assertIs(configure.call_args.kwargs["sector_symmetry_assistance"], expected)
            self.assertIs(configure.call_args.kwargs["numerator_symmetry_assistance"], True)

    def test_sector_symmetry_policy_defaults_inherits_and_fresh_search_resets(self):
        self.assertEqual(self.policy["sector_symmetry_policy"], "generators")
        explicit = PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=True,
                                        sector_symmetry_policy="polynomial-orbits")
        self.write("master-reduction/policy.json", explicit)
        self.assertEqual(PHASES.configuration(self.campaign, enabled=True)["sector_symmetry_policy"], "polynomial-orbits")
        fresh = PHASES.configuration(self.campaign, enabled=True, fresh_search=True, sector_symmetry_assistance=True)
        self.assertEqual(fresh["sector_symmetry_policy"], "generators")
        for choice in PHASES.SECTOR_SYMMETRY_POLICIES:
            selected = PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=True,
                                            sector_symmetry_policy=choice)
            self.assertEqual(selected["sector_symmetry_policy"], choice)
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, sector_symmetry_policy=choice)
        with self.assertRaisesRegex(ValueError, "requires sector-symmetry assistance"):
            PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=False,
                                 sector_symmetry_policy="generators")
        for bad in (True, "unknown", 3):
            with self.assertRaisesRegex(ValueError, "policy must be"):
                PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=True,
                                     sector_symmetry_policy=bad)
        self.write("master-reduction/policy.json", {**explicit, "sector_symmetry_policy": "unknown"})
        with self.assertRaisesRegex(ValueError, "invalid persisted"):
            PHASES.configuration(self.campaign, enabled=True)

    def test_production_sector_symmetry_policy_reaches_configuration(self):
        for choice in PHASES.SECTOR_SYMMETRY_POLICIES:
            with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters",
                                     "--master-sector-symmetry-assistance", "--master-sector-symmetry-policy", choice])
            self.assertEqual(configure.call_args.kwargs["sector_symmetry_policy"], choice)

    def test_sector_saved_projection_is_optional_inherited_and_fresh_defaults_true(self):
        self.assertTrue(self.policy["sector_symmetry_saved_projection"])
        disabled = PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=True,
                                        sector_symmetry_saved_projection=False)
        self.write("master-reduction/policy.json", disabled)
        self.assertFalse(PHASES.configuration(self.campaign, enabled=True)["sector_symmetry_saved_projection"])
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True, fresh_search=True,
                                            sector_symmetry_assistance=True)["sector_symmetry_saved_projection"])
        for choice in (True, False):
            chosen = PHASES.configuration(self.campaign, enabled=True, sector_symmetry_saved_projection=choice)
            self.assertIs(chosen["sector_symmetry_saved_projection"], choice)
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, sector_symmetry_saved_projection=choice)
            with self.assertRaisesRegex(ValueError, "requires sector-symmetry assistance"):
                PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=False,
                                     sector_symmetry_saved_projection=choice)
            with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                flag = "--master-sector-symmetry-saved-projection" if choice else "--no-master-sector-symmetry-saved-projection"
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters",
                                     "--master-sector-symmetry-assistance", flag])
            self.assertIs(configure.call_args.kwargs["sector_symmetry_saved_projection"], choice)
        for bad in (1, "yes"):
            with self.assertRaisesRegex(ValueError, "must be boolean"):
                PHASES.configuration(self.campaign, enabled=True, sector_symmetry_saved_projection=bad)
        self.write("master-reduction/policy.json", {**disabled, "sector_symmetry_saved_projection": 1})
        with self.assertRaisesRegex(ValueError, "invalid persisted"):
            PHASES.configuration(self.campaign, enabled=True)

    def test_selective_symmetry_feedback_is_explicit_exact_and_fresh_resets_it(self):
        self.assertFalse(self.policy["selective_symmetry_feedback"])
        for bad in (1, "yes", [], {}):
            with self.assertRaises(ValueError):
                PHASES.configuration(self.campaign, enabled=True, selective_symmetry_feedback=bad)
        with self.assertRaisesRegex(ValueError, "explicit"):
            PHASES.configuration(self.campaign, selective_symmetry_feedback=True)
        with self.assertRaisesRegex(ValueError, "requires sector-symmetry"):
            PHASES.configuration(self.campaign, enabled=True, selective_symmetry_feedback=True)
        for conflicting in ("saved_rule_assistance", "circuit_symmetry_assistance"):
            with self.assertRaisesRegex(ValueError, "no dynamic saved-rule"):
                PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=True,
                    selective_symmetry_feedback=True, **{conflicting: True})
        selected = PHASES.configuration(self.campaign, enabled=True,
            sector_symmetry_assistance=True, selective_symmetry_feedback=True)
        self.write("master-reduction/policy.json", selected)
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True)["selective_symmetry_feedback"])
        self.assertFalse(PHASES.configuration(self.campaign, enabled=True,
            fresh_search=True, sector_symmetry_assistance=True)["selective_symmetry_feedback"])
        disabled = PHASES.configuration(self.campaign, enabled=True, selective_symmetry_feedback=False)
        self.assertFalse(disabled["selective_symmetry_feedback"])
        for flag, expected in (("--master-selective-symmetry-feedback", True),
                               ("--no-master-selective-symmetry-feedback", False)):
            with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters",
                        "--master-sector-symmetry-assistance", flag])
            self.assertIs(configure.call_args.kwargs["selective_symmetry_feedback"], expected)
        self.write("master-reduction/policy.json", {**selected, "selective_symmetry_feedback": 1})
        with self.assertRaisesRegex(ValueError, "invalid persisted"):
            PHASES.configuration(self.campaign, enabled=True)

    def test_selective_symmetry_feedback_changes_enabled_phase_identity_only(self):
        self.publish_fixture()
        selected = {}
        for value in (None, False, True):
            policy = PHASES.configuration(self.campaign, enabled=True,
                sector_symmetry_assistance=True, selective_symmetry_feedback=value)
            with patch.object(PHASES, "phase_two", return_value=4) as native:
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            selected[value] = native.call_args.args[4]
            steering = PHASES.read_json(selected[value] / "steering-binding.json")
            if value is True:
                self.assertIs(steering["selective_symmetry_feedback"], True)
                self.assertEqual(steering["selective_symmetry_feedback_recipe"], PHASES.SELECTIVE_SYMMETRY_FEEDBACK_RECIPE)
            else:
                self.assertNotIn("selective_symmetry_feedback", steering)
                self.assertNotIn("selective_symmetry_feedback_recipe", steering)
        self.assertEqual(selected[None], selected[False])
        self.assertNotEqual(selected[False], selected[True])

    def test_selective_feedback_progress_is_bounded_and_not_payload_authority(self):
        result = PHASES.symmetry_progress_metadata({"selective_symmetry_feedback": {
            "enabled": True, "round": 2, "selected_targets": 9, "completed_targets": 6,
            "pending_targets": 3, "selected_queue_exhausted": False, "source_jobs_complete": True,
            "ordinary_work_complete": False, "stage": "x" * 1000,
            "target_payloads": ["huge"] * 1000, "new_relations": True}})
        header = result["selective_symmetry_feedback"]
        self.assertEqual(header["pending_targets"], 3)
        self.assertIs(header["selected_queue_exhausted"], False)
        self.assertNotIn("new_relations", header)
        self.assertNotIn("target_payloads", header)
        self.assertEqual(len(header["stage"]), 300)
        with self.assertRaisesRegex(ValueError, "invalid native selective"):
            PHASES.sector_symmetry_metadata({"selective_symmetry_feedback_enabled": 1})

    def test_local_dimensional_recurrence_is_independent_exact_and_fresh_resets_it(self):
        self.assertFalse(self.policy["local_dimensional_recurrence_assistance"])
        with self.assertRaisesRegex(ValueError, "explicit"):
            PHASES.configuration(self.campaign, local_dimensional_recurrence_assistance=True)
        for bad in (1, "true", [], {}):
            with self.assertRaisesRegex(ValueError, "must be boolean"):
                PHASES.configuration(self.campaign, enabled=True, local_dimensional_recurrence_assistance=bad)
        local = PHASES.configuration(self.campaign, enabled=True, local_dimensional_recurrence_assistance=True)
        self.assertFalse(local["sector_symmetry_assistance"])
        self.write("master-reduction/policy.json", local)
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True)["local_dimensional_recurrence_assistance"])
        self.assertFalse(PHASES.configuration(self.campaign, enabled=True, fresh_search=True)["local_dimensional_recurrence_assistance"])
        with self.assertRaisesRegex(ValueError, "requires exact refinement"):
            PHASES.configuration(self.campaign, enabled=True, refinement_strategy="modular-circuits")
        for flag, value in (("--master-local-dimensional-recurrence-assistance", True),
                             ("--no-master-local-dimensional-recurrence-assistance", False)):
            with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("probe")) as configure, \
                 patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters", flag])
            self.assertIs(configure.call_args.kwargs["local_dimensional_recurrence_assistance"], value)
        self.write("master-reduction/policy.json", {**local, "local_dimensional_recurrence_assistance": 1})
        with self.assertRaisesRegex(ValueError, "invalid persisted"):
            PHASES.configuration(self.campaign, enabled=True)

    def test_local_dimension_identity_distinguishes_only_enabled_recipe(self):
        self.publish_fixture()
        selected = {}
        for value in (None, False, True):
            policy = PHASES.configuration(self.campaign, enabled=True, local_dimensional_recurrence_assistance=value)
            with patch.object(PHASES, "phase_two", return_value=4) as native:
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            selected[value] = native.call_args.args[4]
            steering = PHASES.read_json(selected[value] / "steering-binding.json")
            if value is True:
                self.assertEqual(steering["local_dimensional_recurrence_recipe"], PHASES.LOCAL_DIMENSIONAL_RECURRENCE_RECIPE)
            else:
                self.assertNotIn("local_dimensional_recurrence_assistance", steering)
                self.assertNotIn("local_dimensional_recurrence_recipe", steering)
        self.assertEqual(selected[None], selected[False])
        self.assertNotEqual(selected[False], selected[True])

    def test_local_dimension_receipt_retains_native_descriptor_not_payload_in_telemetry(self):
        descriptor = {"recipe": PHASES.LOCAL_DIMENSIONAL_RECURRENCE_RECIPE,
                      "file": "local-native.rrbin", "bytes": 50, "blake3": "a" * 64}
        report = {"local_dimensional_recurrence_assistance": True, "local_dimensional_recurrence_certificate": descriptor,
            "local_dimensional_recurrence_preparation": {"input_keys": 19, "processed_keys": 17,
                "nominated_directions": 25, "supported_geometries": 3, "weighted_source_rows": 12,
                "equations": 7, "output_terms": 31, "complete": False, "native_witnesses": [1] * 1000}}
        self.assertEqual(PHASES.symmetry_metadata(report)["local_dimensional_recurrence_certificate"], descriptor)
        progress = PHASES.symmetry_progress_metadata(report)["local_dimensional_recurrence_preparation"]
        self.assertEqual(progress["weighted_source_rows"], 12)
        self.assertNotIn("native_witnesses", progress)
        with self.assertRaisesRegex(ValueError, "invalid native local"):
            PHASES.symmetry_metadata({**report, "local_dimensional_recurrence_assistance": 1})
        with self.assertRaisesRegex(ValueError, "invalid native local"):
            PHASES.symmetry_metadata({**report, "local_dimensional_recurrence_certificate": {**descriptor, "recipe": "bad"}})

    def test_local_dimension_partial_finalization_and_pointer_keep_native_provenance(self):
        _, phase = self.stopped_refinement_fixture("paused")
        provenance = {"local_dimensional_recurrence_assistance": True,
            "local_dimensional_recurrence_certificate": {"recipe": PHASES.LOCAL_DIMENSIONAL_RECURRENCE_RECIPE,
                "file": "local-capsule.rrbin", "bytes": 41, "blake3": "c" * 64},
            "inherited_equation_authority": {"local_dimensional_recurrence_assistance": True}}
        self.write(str((phase / "latest.json").relative_to(self.campaign)),
            {**PHASES.read_json(phase / "latest.json"), **provenance})
        original = (phase / "latest.json").read_bytes()
        with patch.object(PHASES, "phase_two", return_value=0) as native:
            PHASES.run(self.plan, {**self.policy, "operation": "finalize-refinement"}, True,
                       self.driver, postprocess_only=True)
        partial = native.call_args.args[1]["partial_refinement"]
        for field, value in provenance.items():
            self.assertEqual(partial[field], value)
        output = native.call_args.args[4]
        self.write(str((output / "artifact.json").relative_to(self.campaign)), {
            **provenance, "inventory": {"complete": True}, "finite_search_complete": False,
            "partial_refinement_source": {"checkpoint": 7}})
        PHASES.publish_pointer(self.campaign, output, self.binding(), "refine", self.driver, partial)
        artifact = PHASES.completed_artifact(self.campaign)
        for field, value in provenance.items():
            self.assertEqual(artifact[field], value)
        self.assertEqual((phase / "latest.json").read_bytes(), original)

    def test_parallel_batch_policy_is_positive_operational_and_retained(self):
        policy=PHASES.configuration(self.campaign,enabled=True,relation_batch_rows=128,relation_block_rows=8)
        self.write("master-reduction/policy.json",policy)
        self.assertEqual(PHASES.configuration(self.campaign,enabled=True)["relation_batch_rows"],128)
        self.assertEqual(PHASES.configuration(self.campaign,enabled=True,relation_block_rows=16)["relation_block_rows"],16)
        for bad in (0,-1,True,"8"):
            with self.assertRaises(ValueError):
                PHASES.configuration(self.campaign,enabled=True,relation_batch_rows=bad)

    def test_modular_policy_is_portable_explicit_and_has_separate_strategy(self):
        path = self.campaign / "modular.json"
        path.write_text('{"batch_rows":7,"reconstruction":{"max_primes":24}}')
        policy = PHASES.configuration(self.campaign, enabled=True,
                                     refinement_strategy="modular-circuits", modular_policy=path)
        self.assertEqual(policy["refinement_strategy"], "modular-circuits")
        self.assertEqual(policy["modular_policy"]["reconstruction"]["max_primes"], 24)
        self.write("master-reduction/policy.json", policy)
        path.unlink()
        self.assertEqual(PHASES.configuration(self.campaign, enabled=True)["modular_policy"], policy["modular_policy"])
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        self.assertEqual(PHASES.configuration(self.campaign, enabled=True, refinement_strategy="exact")["refinement_strategy"], "exact")
        for supplied in ({"refinement_strategy":"modular-circuits"}, {"modular_policy":path}):
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, **supplied)
        with self.assertRaisesRegex(ValueError, "ordinary"):
            PHASES.configuration(self.campaign, enabled=True, saved_rule_assistance=True)

    def test_modular_rejects_explicit_exact_batch_knobs_and_retains_saved_preferences(self):
        exact = PHASES.configuration(self.campaign, enabled=True, relation_batch_rows=128, relation_block_rows=8)
        self.write("master-reduction/policy.json", exact)
        modular = PHASES.configuration(self.campaign, enabled=True, refinement_strategy="modular-circuits")
        self.assertEqual(modular["refinement_strategy"], "modular-circuits")
        self.assertEqual(modular["relation_batch_rows"], 128)
        for supplied in ({"relation_batch_rows":64}, {"relation_block_rows":4}):
            with self.assertRaisesRegex(ValueError, "--modular-policy batch_rows/max_candidates"):
                PHASES.configuration(self.campaign, enabled=True, refinement_strategy="modular-circuits", **supplied)

    def test_production_modular_options_reach_configuration(self):
        with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
            with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters",
                                 "--master-refinement-strategy", "modular-circuits",
                                 "--master-modular-policy", "grants.json"])
            self.assertEqual(configure.call_args.kwargs["refinement_strategy"], "modular-circuits")
            self.assertEqual(configure.call_args.kwargs["modular_policy"], Path("grants.json"))

    def test_publication_verification_defaults_fast_and_rejects_refinement_flag(self):
        self.assertFalse(self.policy["deep_verification"])
        deep = PHASES.configuration(self.campaign, deep_verification=True)
        self.write("master-reduction/policy.json", deep)
        self.assertTrue(PHASES.configuration(self.campaign)["deep_verification"])
        self.assertFalse(PHASES.configuration(self.campaign, deep_verification=False)["deep_verification"])
        for choice in (False, True):
            with self.assertRaisesRegex(ValueError, "not artifact refinement"):
                PHASES.configuration(self.campaign, enabled=True, deep_verification=choice)
        with self.assertRaisesRegex(ValueError, "boolean"):
            PHASES.configuration(self.campaign, deep_verification="yes")

    def test_production_publication_verification_reaches_configuration(self):
        for flag, choice in (("--deep-verification", True), ("--no-deep-verification", False)):
            with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--publish-only", flag])
                self.assertEqual(configure.call_args.kwargs["deep_verification"], choice)
    def test_saved_rule_preference_is_explicit_only_and_legacy_defaults_to_ordinary(self):
        self.assertFalse(self.policy["saved_rule_assistance"])
        legacy = {key: value for key, value in self.policy.items() if key != "saved_rule_assistance"}
        self.write("master-reduction/policy.json", legacy)
        self.assertFalse(PHASES.configuration(self.campaign, enabled=True)["saved_rule_assistance"])
        assisted = PHASES.configuration(self.campaign, enabled=True, saved_rule_assistance=True)
        self.write("master-reduction/policy.json", assisted)
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True)["saved_rule_assistance"])
        ordinary = PHASES.configuration(self.campaign, enabled=True, saved_rule_assistance=False)
        self.assertFalse(ordinary["saved_rule_assistance"])
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        for supplied in (True, False):
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, saved_rule_assistance=supplied)
        self.write("master-reduction/policy.json", {**assisted, "saved_rule_assistance": "true"})
        with self.assertRaisesRegex(ValueError, "invalid persisted"):
            PHASES.configuration(self.campaign, enabled=True)

    def test_production_assistance_flag_reaches_phase_configuration(self):
        for flag, value in (("--master-saved-rule-assistance", True),
                            ("--no-master-saved-rule-assistance", False)):
            with self.subTest(flag=flag), patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters", flag])
                self.assertTrue(configure.call_args.args[1])
                self.assertEqual(configure.call_args.kwargs["saved_rule_assistance"], value)

    def test_circuit_preference_is_refine_only_boolean_and_legacy_compatible(self):
        self.assertFalse(self.policy["circuit_symmetry_assistance"])
        legacy = {key: value for key, value in self.policy.items() if key != "circuit_symmetry_assistance"}
        self.write("master-reduction/policy.json", legacy)
        self.assertFalse(PHASES.configuration(self.campaign, enabled=True)["circuit_symmetry_assistance"])
        assisted = PHASES.configuration(self.campaign, enabled=True, circuit_symmetry_assistance=True)
        self.write("master-reduction/policy.json", assisted)
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True)["circuit_symmetry_assistance"])
        self.assertFalse(PHASES.configuration(self.campaign, enabled=True, circuit_symmetry_assistance=False)["circuit_symmetry_assistance"])
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        for supplied in (True, False):
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, circuit_symmetry_assistance=supplied)
        for invalid in ("true", 1):
            with self.assertRaisesRegex(ValueError, "boolean"):
                PHASES.configuration(self.campaign, enabled=True, circuit_symmetry_assistance=invalid)
            self.write("master-reduction/policy.json", {**assisted, "circuit_symmetry_assistance": invalid})
            with self.assertRaisesRegex(ValueError, "invalid persisted"):
                PHASES.configuration(self.campaign, enabled=True)

    def test_production_circuit_flags_reach_phase_configuration(self):
        for flag, value in (("--master-circuit-symmetry-assistance", True),
                            ("--no-master-circuit-symmetry-assistance", False)):
            with self.subTest(flag=flag), patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters", flag])
                self.assertTrue(configure.call_args.args[1])
                self.assertEqual(configure.call_args.kwargs["circuit_symmetry_assistance"], value)

    def test_finite_feedback_defaults_on_and_persists_explicit_baseline(self):
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True)["finite_feedback"])
        baseline = PHASES.configuration(self.campaign, enabled=True, finite_feedback=False)
        self.write("master-reduction/policy.json", baseline)
        self.assertFalse(PHASES.configuration(self.campaign, enabled=True)["finite_feedback"])
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        self.assertTrue(PHASES.configuration(self.campaign, enabled=True, finite_feedback=True)["finite_feedback"])
        for mode in (True, False):
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, finite_feedback=mode)
        for invalid in ("true", 1):
            with self.assertRaisesRegex(ValueError, "boolean"):
                PHASES.configuration(self.campaign, enabled=True, finite_feedback=invalid)

    def test_production_finite_feedback_flags_reach_configuration(self):
        for flag, value in (("--master-finite-feedback", True), ("--no-master-finite-feedback", False)):
            with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters", flag])
                self.assertEqual(configure.call_args.kwargs["finite_feedback"], value)

    def test_normalization_preference_is_optional_refine_only_and_switches_both_ways(self):
        self.assertNotIn("normalization_profile", self.policy)
        for profile in ("standard", "conservative"):
            policy = PHASES.configuration(self.campaign, enabled=True, normalization_profile=profile)
            self.write("master-reduction/policy.json", policy)
            self.assertEqual(PHASES.configuration(self.campaign, enabled=True)["normalization_profile"], profile)
            self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, normalization_profile=profile)
        for invalid in ("standard-v1", "unlimited", True, []):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, "profile"):
                PHASES.configuration(self.campaign, enabled=True, normalization_profile=invalid)
            self.write("master-reduction/policy.json", {**self.policy, "normalization_profile": invalid})
            with self.assertRaisesRegex(ValueError, "invalid persisted"):
                PHASES.configuration(self.campaign, enabled=True)

    def test_production_normalization_profile_is_refine_only_and_forwarded(self):
        for profile in ("conservative", "standard"):
            with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
                with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters",
                                     "--master-normalization-profile", profile])
                self.assertEqual(configure.call_args.kwargs["normalization_profile"], profile)
        for options in (("--master-normalization-profile", "standard"),
                        ("--publish-only", "--master-normalization-profile", "conservative"),
                        ("--refine-masters", "--master-normalization-profile", "unknown")):
            with self.subTest(options=options), patch.object(PRODUCTION.sys, "stderr", io.StringIO()):
                with self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), *options])
        self.assertFalse((self.campaign / "master-reduction").exists())

    def test_containing_sector_preference_is_explicit_monotone_and_legacy_compatible(self):
        self.assertEqual(self.policy["containing_sector_depth"], 0)
        legacy = {key: value for key, value in self.policy.items() if key != "containing_sector_depth"}
        self.write("master-reduction/policy.json", legacy)
        self.assertEqual(PHASES.configuration(self.campaign, enabled=True)["containing_sector_depth"], 0)
        extra = PHASES.configuration(self.campaign, enabled=True, containing_sector_depth=1)
        self.write("master-reduction/policy.json", extra)
        self.assertEqual(PHASES.configuration(self.campaign, enabled=True)["containing_sector_depth"], 1)
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        with self.assertRaisesRegex(ValueError, "cannot lower"):
            PHASES.configuration(self.campaign, enabled=True, containing_sector_depth=0)
        for supplied in (0, 1):
            with self.assertRaisesRegex(ValueError, "explicit"):
                PHASES.configuration(self.campaign, containing_sector_depth=supplied)
        for invalid in (-1, True, "1"):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, "nonnegative"):
                PHASES.configuration(self.campaign, enabled=True, containing_sector_depth=invalid)
            self.write("master-reduction/policy.json", {**extra, "containing_sector_depth": invalid})
            with self.assertRaisesRegex(ValueError, "invalid persisted"):
                PHASES.configuration(self.campaign)

    def test_production_containing_sector_flag_is_refine_only(self):
        with patch.object(PRODUCTION.PHASES, "configuration", side_effect=ValueError("configuration probe")) as configure:
            with patch.object(PRODUCTION.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                PRODUCTION.main(["--campaign-directory", str(self.campaign), "--refine-masters",
                                 "--master-containing-sector-depth", "2"])
            self.assertTrue(configure.call_args.args[1])
            self.assertEqual(configure.call_args.kwargs["containing_sector_depth"], 2)
        for options in (("--master-containing-sector-depth", "0"),
                        ("--publish-only", "--master-containing-sector-depth", "1"),
                        ("--refine-masters", "--master-containing-sector-depth", "-1")):
            with self.subTest(options=options), patch.object(PRODUCTION.sys, "stderr", io.StringIO()):
                with self.assertRaises(SystemExit):
                    PRODUCTION.main(["--campaign-directory", str(self.campaign), *options])
        self.assertFalse((self.campaign / "master-reduction").exists())

    def test_separate_native_executable_freezes_without_changing_phase_one(self):
        binary = self.campaign / "new-native"
        binary.write_bytes(b"#!/bin/sh\nexit 0\n")
        binary.chmod(0o700)
        policy = PHASES.configuration(self.campaign, True, executable=binary)
        self.assertFalse((self.campaign / "master-reduction").exists())
        PHASES.freeze_master_executable(self.campaign / "master-reduction", policy, PRODUCTION)
        frozen = self.campaign / "master-reduction" / policy["executable"]["path"]
        self.assertEqual(frozen.read_bytes(), binary.read_bytes())
        self.assertFalse((self.campaign / "bin").exists())
        self.write("master-reduction/policy.json", policy)
        binary.write_bytes(b"#!/bin/sh\nexit 1\n")
        self.assertEqual(PHASES.configuration(self.campaign)["executable"], policy["executable"])
        explicit = PHASES.configuration(self.campaign, executable=binary)
        self.assertNotEqual(explicit["executable"]["sha256"], policy["executable"]["sha256"])
        PHASES.freeze_master_executable(self.campaign / "master-reduction", explicit, PRODUCTION)
        self.assertTrue(frozen.is_file())

    def test_drained_receipt_bound_to_exact_checkpoint_not_cached_rootbar(self):
        self.assertIsNotNone(self.ready())
        self.result["recursive_worklist_exhausted"] = False
        self.result["descendant_closure"] = {"initial_closed": 67, "initial_total": 67}
        self.write("runs/first/result.json", self.result)
        self.assertIsNone(self.ready())
        self.result["recursive_worklist_exhausted"] = True
        self.result["checkpoint"]["manifest_blake3"] = "11" * 32
        self.write("runs/first/result.json", self.result)
        self.assertIsNone(self.ready())

    def test_new_rank_or_D_amendment_changes_binding_and_requires_phase_one(self):
        original = self.binding()["key"]
        self.write("amendments/amendment-0001.json", {"rank": 1, "max_D": 9})
        self.assertNotEqual(original, self.binding()["key"])
        self.assertIsNone(self.ready())
        rank = self.binding()["key"]
        self.write("amendments/amendment-0001.json", {"rank": 1, "max_D": 10})
        self.assertNotEqual(rank, self.binding()["key"])
        self.command += ["--amend-queries", str(self.campaign / "amendments/amendment-0001.json")]
        self.write("runs/first/request.json", {"command": self.command})
        self.assertIsNotNone(self.ready())

    def test_resume_goes_directly_back_to_master_reduction_for_same_scope(self):
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as second:
            self.assertEqual(PHASES.run(self.plan, self.policy, True, self.driver), 4)
        first.assert_not_called()
        self.assertEqual(second.call_args.args[3]["key"], self.binding()["key"])
        self.assertFalse((self.campaign / "active-run.json").exists())

    def test_pending_scope_runs_phase_one_and_does_not_start_old_master_scope(self):
        self.write("amendments/amendment-0001.json", {"rank": 1})
        with patch.object(PHASES, "phase_one", return_value=(4, False)) as first, patch.object(PHASES, "phase_two") as second:
            self.assertEqual(PHASES.run(self.plan, self.policy, True, self.driver), 4)
        first.assert_called_once()
        second.assert_not_called()

    def test_postprocess_only_never_starts_solve_when_incomplete(self):
        self.result["recursive_worklist_exhausted"] = False
        self.write("runs/first/result.json", self.result)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two") as second:
            with self.assertRaisesRegex(ValueError, "postprocess-only"):
                PHASES.run(self.plan, self.policy, True, self.driver, postprocess_only=True)
        first.assert_not_called()
        second.assert_not_called()

    def publish_fixture(self, operation="publish", depth=0, saved_rule_assistance=False, containing_sector_depth=0,
                        circuit_symmetry_assistance=False, normalization_profile=None, finite_feedback=True,
                        deep_verification=None):
        path = self.campaign / "master-reduction/scopes/published"
        self.write("master-reduction/scopes/published/artifact.json", {"status": "published_unrefined",
                   "inventory": {"complete": True, **({"independently_verified": deep_verification}
                                                       if deep_verification is not None else {})},
                   **({"deep_verification": deep_verification} if deep_verification is not None else {}),
                   **({"normalization_profile": normalization_profile, "normalization_limits": {"fixture": True}}
                      if normalization_profile is not None else {})})
        PHASES.publish_pointer(self.campaign, path, self.binding(), operation, self.driver,
                               {"seed_depth": depth, "saved_rule_assistance": saved_rule_assistance,
                                "collection_strategy": PHASES.COLLECTION_STRATEGY,
                                "finite_feedback": finite_feedback,
                                "finite_feedback_recipe": PHASES.FINITE_FEEDBACK_RECIPE,
                                "collection_inputs": [],
                                "circuit_symmetry_assistance": circuit_symmetry_assistance,
                                "containing_sector_depth": containing_sector_depth}
                               if operation == "refine" else None)
        return path

    def test_refine_is_explicit_and_reads_published_artifact_without_walking(self):
        source = self.publish_fixture()
        policy = PHASES.configuration(self.campaign, enabled=True)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as second:
            self.assertEqual(PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True), 4)
        first.assert_not_called()
        self.assertEqual(second.call_args.args[1]["source_artifact"], str(source))
        pointer = PHASES.read_json(self.campaign / "artifacts/latest.json")
        self.assertEqual(pointer["operation"], "publish")  # Paused refinement preserves usable output.
        self.assertFalse(Path(pointer["directory"]).is_absolute())

    def stopped_refinement_fixture(self, status="failed"):
        source = self.publish_fixture()
        policy = PHASES.configuration(self.campaign, enabled=True)
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
        phase = native.call_args.args[4]
        self.write(str((phase / "latest.json").relative_to(self.campaign)), {
            "status": status, "operation": "refine", "seed_depth": 0,
            "finite_feedback": True, "finite_feedback_recipe": PHASES.FINITE_FEEDBACK_RECIPE,
            "inventory": {"complete": True}, "completed_source_rows": 7,
            "last_error": "retained failure", "native_state": {"cursor": 7}})
        self.write("master-reduction/active-phase.json", {
            "operation": "refine", "directory": str(phase), "run_directory": str(phase / "runs/stopped"),
            "request": str(self.campaign / "runs/first/request.json"), "scope_binding": self.binding()["key"]})
        return source, phase

    def test_unrefined_publication_is_not_labeled_as_a_partial_refinement(self):
        source = self.publish_fixture()
        native = PHASES.read_json(source / "artifact.json")
        native.update(finite_search_complete=False, refinement_complete=False)
        self.write(str((source / "artifact.json").relative_to(self.campaign)), native)
        PHASES.publish_pointer(self.campaign, source, self.binding(), "publish", self.driver)
        pointer = PHASES.completed_artifact(self.campaign)
        self.assertEqual(pointer["status"], "published_unrefined")
        self.assertNotIn("partial_refinement_source", pointer)

    def test_budget_and_executable_changes_resume_the_original_finite_cursor(self):
        source, phase = self.stopped_refinement_fixture()
        policy = PHASES.configuration(self.campaign, enabled=True, max_relation_columns=0)
        policy["executable"] = {"sha256": "changed-build"}
        with patch.object(PHASES, "freeze_master_executable"), patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
        self.assertEqual(native.call_args.args[4], phase)
        self.assertEqual(native.call_args.args[1]["source_artifact"], str(source))
        self.assertEqual(native.call_args.args[1]["max_relation_columns"], 0)

    def test_fresh_search_requires_latest_stopped_maps_to_be_finalized(self):
        source, phase = self.stopped_refinement_fixture("paused")
        policy_path = self.campaign / "master-reduction/policy.json"
        original_policy = policy_path.read_bytes()
        original_source = (source / "artifact.json").read_bytes()
        fresh = PHASES.configuration(self.campaign, enabled=True, fresh_search=True,
                                     seed_depth=3, containing_sector_depth=1)
        with patch.object(PHASES, "phase_one") as solve, patch.object(PHASES, "phase_two") as native:
            with self.assertRaisesRegex(ValueError, "finalize-refinement"):
                PHASES.run(self.plan, fresh, True, self.driver, postprocess_only=True)
            solve.assert_not_called()
            native.assert_not_called()
        self.assertEqual(policy_path.read_bytes(), original_policy)
        self.assertEqual((source / "artifact.json").read_bytes(), original_source)
        # A retained export is sufficient only while it binds this exact source
        # manifest. More accepted work must be explicitly finalized again.
        retained = {"source_directory": str(phase.relative_to(self.campaign)),
                    "source_manifest_sha256": PHASES.digest(phase / "latest.json")}
        PHASES.publish_pointer(self.campaign, source, self.binding(), "refine", self.driver, retained)
        self.write("master-reduction/completed-phase.json", {
            "directory": str(source), "scope_binding": self.binding()})
        latest = PHASES.read_json(phase / "latest.json")
        latest["completed_source_rows"] += 1
        self.write(str((phase / "latest.json").relative_to(self.campaign)), latest)
        with patch.object(PHASES, "phase_two") as native, self.assertRaisesRegex(ValueError, "finalize-refinement"):
            PHASES.run(self.plan, fresh, True, self.driver, postprocess_only=True)
        native.assert_not_called()

    def test_fresh_search_uses_requested_lower_shape_and_resumes_its_new_round(self):
        source = self.publish_fixture("refine", depth=2, containing_sector_depth=2,
                                      normalization_profile="standard-v1")
        source_bytes = (source / "artifact.json").read_bytes()
        old = PHASES.configuration(self.campaign, enabled=True, seed_depth=2, containing_sector_depth=2)
        self.write("master-reduction/policy.json", old)
        directories = set()
        for containing in (1, 0):
            fresh = PHASES.configuration(self.campaign, enabled=True, fresh_search=True,
                                         seed_depth=3, containing_sector_depth=containing)
            with patch.object(PHASES, "phase_one") as solve, patch.object(PHASES, "phase_two", return_value=4) as native:
                self.assertEqual(PHASES.run(self.plan, fresh, True, self.driver, postprocess_only=True), 4)
            solve.assert_not_called()
            launched, directory = native.call_args.args[1], native.call_args.args[4]
            self.assertNotIn(directory, directories)
            directories.add(directory)
            self.assertEqual(launched["source_artifact"], str(source))
            self.assertEqual((launched["effective_seed_depth"], launched["effective_containing_sector_depth"]),
                             (3, containing))
            self.assertEqual(launched["effective_normalization_profile"], "standard-v1")
            steering = PHASES.read_json(directory / "steering-binding.json")
            self.assertTrue(steering["fresh_search"])
            self.assertEqual(steering["fresh_search_recipe"], PHASES.FRESH_SEARCH_RECIPE)
            saved = PHASES.read_json(self.campaign / "master-reduction/policy.json")
            self.assertNotIn("fresh_search", saved)
            self.assertNotIn("fresh_search_recipe", saved)
        # The second round pauses durably. A regular invocation infers its mode
        # from steering, not a sticky reset request in the saved policy.
        self.write("master-reduction/active-phase.json", {"operation": "refine", "directory": str(directory)})
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True,
                       self.driver, postprocess_only=True)
        self.assertEqual(native.call_args.args[4], directory)
        self.assertTrue(native.call_args.args[1]["fresh_search"])
        self.write(str((directory / "latest.json").relative_to(self.campaign)), {
            "status": "paused", "fresh_search": True, "fresh_search_recipe": PHASES.FRESH_SEARCH_RECIPE,
            "seed_depth": 3, "containing_sector_depth": 0, "completed_source_rows": 4})
        resume = PHASES.configuration(self.campaign, enabled=True)
        self.assertFalse(resume["fresh_search"])
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, resume, True, self.driver, postprocess_only=True)
        resumed = native.call_args.args[1]
        self.assertEqual(native.call_args.args[4], directory)
        self.assertTrue(resumed["fresh_search"])
        self.assertEqual((resumed["effective_seed_depth"], resumed["effective_containing_sector_depth"]), (3, 0))
        with patch.object(PHASES, "phase_two") as native, self.assertRaisesRegex(ValueError, "finalize-refinement"):
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True, fresh_search=True,
                       seed_depth=3, containing_sector_depth=0), True, self.driver, postprocess_only=True)
        native.assert_not_called()
        self.assertEqual((source / "artifact.json").read_bytes(), source_bytes)

    def test_fresh_search_uses_current_finalized_source_not_original_cursor(self):
        source, phase = self.stopped_refinement_fixture("paused")
        original_manifest = (phase / "latest.json").read_bytes()
        export = self.campaign / "master-reduction/scopes/retained-export"
        self.write(str((export / "artifact.json").relative_to(self.campaign)),
                   PHASES.read_json(source / "artifact.json"))
        retained = {"source_directory": str(phase.relative_to(self.campaign)),
                    "source_manifest_sha256": PHASES.digest(phase / "latest.json"),
                    "seed_depth": 2, "containing_sector_depth": 2}
        PHASES.publish_pointer(self.campaign, export, self.binding(), "refine", self.driver, retained)
        self.write("master-reduction/completed-phase.json", {
            "directory": str(export), "scope_binding": self.binding()})
        policy = PHASES.configuration(self.campaign, enabled=True, fresh_search=True,
                                      seed_depth=3, containing_sector_depth=1)
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
        self.assertEqual(native.call_args.args[1]["source_artifact"], str(export))
        self.assertNotEqual(native.call_args.args[4], phase)
        self.assertEqual((phase / "latest.json").read_bytes(), original_manifest)

    def test_numerator_symmetry_changes_only_enabled_phase_identity(self):
        self.publish_fixture()
        selected = {}
        for value in (None, False, True):
            policy = PHASES.configuration(self.campaign, enabled=True, numerator_symmetry_assistance=value)
            with patch.object(PHASES, "phase_two", return_value=4) as native:
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            directory = native.call_args.args[4]
            steering = PHASES.read_json(directory / "steering-binding.json")
            selected[value] = directory
            self.assertEqual(steering.get("numerator_symmetry_assistance", False), value is True)
            if value is not True:
                self.assertNotIn("numerator_symmetry_assistance", steering)
        self.assertEqual(selected[None], selected[False])
        self.assertNotEqual(selected[False], selected[True])
        self.write(str((selected[True] / "latest.json").relative_to(self.campaign)), {"status": "paused"})
        self.write("master-reduction/active-phase.json", {"operation": "refine", "directory": str(selected[True])})
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True,
                       self.driver, postprocess_only=True)
        self.assertEqual(native.call_args.args[4], selected[True])
        self.assertTrue(native.call_args.args[1]["numerator_symmetry_assistance"])

    def test_partial_export_and_publication_preserve_numerator_certificate_not_provider_authority(self):
        _, phase = self.stopped_refinement_fixture("paused")
        provenance = {"numerator_symmetry_assistance": True,
            "numerator_symmetry_certificate": {"recipe": PHASES.NUMERATOR_SYMMETRY_RECIPE,
                "file": "numerator-symmetry.bin", "bytes": 123, "blake3": "a" * 64,
                "inventory_binding": "original-finite-inventory", "authority": "finite-equations"},
            "inherited_equation_authority": {"numerator_symmetry_assistance": True}}
        latest = {**PHASES.read_json(phase / "latest.json"), **provenance}
        self.write(str((phase / "latest.json").relative_to(self.campaign)), latest)
        original = (phase / "latest.json").read_bytes()
        with patch.object(PHASES, "phase_two", return_value=0) as native:
            PHASES.run(self.plan, {**self.policy, "operation": "finalize-refinement"}, True,
                       self.driver, postprocess_only=True)
        partial = native.call_args.args[1]["partial_refinement"]
        for key, value in provenance.items():
            self.assertEqual(partial[key], value)
        export = native.call_args.args[4]
        self.write(str((export / "artifact.json").relative_to(self.campaign)), {
            **provenance, "inventory": {"complete": True}, "finite_search_complete": False,
            "partial_refinement_source": {"checkpoint": 7}})
        PHASES.publish_pointer(self.campaign, export, self.binding(), "refine", self.driver, partial)
        pointer = PHASES.completed_artifact(self.campaign)
        for key, value in provenance.items():
            self.assertEqual(pointer[key], value)
        self.assertFalse(PHASES.configuration(self.campaign, enabled=True, fresh_search=True)["numerator_symmetry_assistance"])
        self.assertEqual((phase / "latest.json").read_bytes(), original)
        # Historical provider use can remain in refinement provenance while a
        # later publication retains those proofs with its current provider off.
        native_report = PHASES.read_json(export / "artifact.json")
        native_report["numerator_symmetry_assistance"] = False
        self.write(str((export / "artifact.json").relative_to(self.campaign)), native_report)
        PHASES.publish_pointer(self.campaign, export, self.binding(), "refine", self.driver, partial)
        self.assertFalse(PHASES.completed_artifact(self.campaign)["numerator_symmetry_assistance"])
        pointer_record = PHASES.read_json(self.campaign / "artifacts/latest.json")
        pointer_record["numerator_symmetry_certificate"] = {**provenance["numerator_symmetry_certificate"], "blake3": "b" * 64}
        self.write("artifacts/latest.json", pointer_record)
        with self.assertRaisesRegex(ValueError, "provenance differs"):
            PHASES.completed_artifact(self.campaign)

    def test_sector_symmetry_changes_only_enabled_identity_and_retains_resume_preference(self):
        self.publish_fixture()
        selected = {}
        for value in (None, False, True):
            policy = PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=value)
            with patch.object(PHASES, "phase_two", return_value=4) as native:
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            directory = native.call_args.args[4]
            steering = PHASES.read_json(directory / "steering-binding.json")
            selected[value] = directory
            self.assertEqual(steering.get("sector_symmetry_assistance", False), value is True)
            if value is not True:
                self.assertNotIn("sector_symmetry_assistance", steering)
                self.assertNotIn("sector_symmetry_recipe", steering)
                self.assertNotIn("sector_symmetry_policy", steering)
            else:
                self.assertEqual(steering["sector_symmetry_recipe"],
                                 "verified-sector-symmetry-sources-v3")
                self.assertEqual(steering["sector_symmetry_policy"], "generators")
        self.assertEqual(selected[None], selected[False])
        self.assertNotEqual(selected[False], selected[True])
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True,
                       sector_symmetry_assistance=True), True, self.driver, postprocess_only=True)
        self.assertEqual(native.call_args.args[4], selected[True])
        self.write(str((selected[True] / "latest.json").relative_to(self.campaign)),
                   {"status": "paused", "sector_symmetry_assistance": True})
        self.write("master-reduction/active-phase.json", {"operation": "refine", "directory": str(selected[True])})
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True,
                       self.driver, postprocess_only=True)
        self.assertEqual(native.call_args.args[4], selected[True])
        self.assertTrue(native.call_args.args[1]["sector_symmetry_assistance"])

    def test_sector_symmetry_policy_has_deterministic_distinct_enabled_phase_identity(self):
        self.publish_fixture()
        selected = {}
        for choice in (None, "generators", "polynomial-orbits", "polynomial-orbits"):
            policy = PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=True,
                                          sector_symmetry_policy=choice)
            with patch.object(PHASES, "phase_two", return_value=4) as native:
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            directory = native.call_args.args[4]
            steering = PHASES.read_json(directory / "steering-binding.json")
            self.assertEqual(steering["sector_symmetry_policy"], choice or "generators")
            if choice in selected:
                self.assertEqual(selected[choice], directory)
            selected[choice] = directory
        self.assertEqual(selected[None], selected["generators"])
        self.assertNotEqual(selected["generators"], selected["polynomial-orbits"])

    def test_sector_saved_projection_changes_only_explicit_false_phase_identity(self):
        self.publish_fixture()
        selected = {}
        for choice in (None, True, False):
            policy = PHASES.configuration(self.campaign, enabled=True, sector_symmetry_assistance=True,
                                          sector_symmetry_saved_projection=choice)
            with patch.object(PHASES, "phase_two", return_value=4) as native:
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            selected[choice] = native.call_args.args[4]
            steering = PHASES.read_json(selected[choice] / "steering-binding.json")
            if choice is False:
                self.assertIs(steering["sector_symmetry_saved_projection"], False)
            else:
                self.assertNotIn("sector_symmetry_saved_projection", steering)
        self.assertEqual(selected[None], selected[True])
        self.assertNotEqual(selected[True], selected[False])

    def test_partial_export_and_publication_preserve_sector_binary_certificates_and_authority(self):
        _, phase = self.stopped_refinement_fixture("paused")
        provenance = {"sector_symmetry_assistance": True, "sector_symmetry_policy": "polynomial-orbits",
            "sector_symmetry_certificate": {"recipe": PHASES.SECTOR_SYMMETRY_RECIPE,
                "policy": "polynomial-orbits",
                "file": "sector-symmetry.bin", "bytes": 123, "blake3": "a" * 64,
                "inventory_binding": "original-finite-inventory", "authority": "finite-equations"},
            "sector_symmetry_reduced_certificate": {"recipe": PHASES.SECTOR_SYMMETRY_REDUCED_RECIPE,
                "file": "sector-symmetry-reduced.bin", "bytes": 112, "blake3": "b" * 64,
                "authority_binding": "saved-rule-binding", "rows": 4, "complete": True,
                "authority": "saved-projection"},
            "sector_symmetry_normalization_certificate": {"recipe": PHASES.SECTOR_SYMMETRY_NORMALIZATION_RECIPE,
                "file": "sector-symmetry-normalization.bin", "bytes": 114, "blake3": "d" * 64,
                "inventory_binding": "original-finite-inventory", "normalization_profile": "ram-controlled-v1"},
            "sector_symmetry_task_certificate": {"recipe": PHASES.SECTOR_SYMMETRY_TASK_RECIPE,
                "file": "sector-symmetry-tasks.bin", "bytes": 118, "blake3": "e" * 64,
                "authority_binding": "composed-sources", "rows": 4},
            "sector_symmetry_projection_checkpoint": {"recipe": PHASES.SECTOR_SYMMETRY_REDUCED_RECIPE,
                "file": "sector-symmetry-projection.bin", "bytes": 117, "blake3": "f" * 64,
                "authority_binding": "saved-rule-binding", "processed_rows": 2, "source_rows": 4,
                "complete": False, "authority": "incomplete-saved-projection"},
            "inherited_equation_authority": {"sector_symmetry_assistance": True,
                                               "sector_symmetry_policy": "polynomial-orbits"}}
        latest = {**PHASES.read_json(phase / "latest.json"), **provenance}
        self.write(str((phase / "latest.json").relative_to(self.campaign)), latest)
        original = (phase / "latest.json").read_bytes()
        with patch.object(PHASES, "phase_two", return_value=0) as native:
            PHASES.run(self.plan, {**self.policy, "operation": "finalize-refinement"}, True,
                       self.driver, postprocess_only=True)
        partial = native.call_args.args[1]["partial_refinement"]
        for key, value in provenance.items():
            self.assertEqual(partial[key], value)
        export = native.call_args.args[4]
        self.write(str((export / "artifact.json").relative_to(self.campaign)), {
            **provenance, "inventory": {"complete": True}, "finite_search_complete": False,
            "partial_refinement_source": {"checkpoint": 7}})
        PHASES.publish_pointer(self.campaign, export, self.binding(), "refine", self.driver, partial)
        for key, value in provenance.items():
            self.assertEqual(PHASES.completed_artifact(self.campaign)[key], value)
        self.assertFalse(PHASES.configuration(self.campaign, enabled=True, fresh_search=True)["sector_symmetry_assistance"])
        self.assertEqual((phase / "latest.json").read_bytes(), original)
        # Retained equations carry their original authority; they do not grant
        # authority to discover new sector maps in an ordinary publication.
        self.write(str((export / "artifact.json").relative_to(self.campaign)), {
            **PHASES.read_json(export / "artifact.json"), "sector_symmetry_assistance": False})
        PHASES.publish_pointer(self.campaign, export, self.binding(), "refine", self.driver, partial)
        self.assertFalse(PHASES.completed_artifact(self.campaign)["sector_symmetry_assistance"])
        pointer = PHASES.read_json(self.campaign / "artifacts/latest.json")
        pointer["sector_symmetry_reduced_certificate"] = {
            **provenance["sector_symmetry_reduced_certificate"], "blake3": "c" * 64}
        self.write("artifacts/latest.json", pointer)
        with self.assertRaisesRegex(ValueError, "provenance differs"):
            PHASES.completed_artifact(self.campaign)

    def test_symmetry_progress_keeps_only_bounded_native_headers(self):
        progress = PHASES.symmetry_progress_metadata({"sector_symmetry_preparation": {
            "discovered_maps": 7, "support_closure_supports": 3, "transported_keys": 9,
            "transported_rows": 11, "complete": False, "maps": ["native-payload"], "input_keys": 939,
            "processed_keys": 104, "closure_keys": 8401, "orbit_images": 12, "orbit_terms": 144,
            "legacy_source_equations": 26, "normalization_input_keys": 939, "normalization_canonical_keys": 728,
            "normalization_unit_aliases": 197, "normalization_projected_numerators": 14,
            "normalization_source_equations": 211, "normalization_output_terms": 981,
            "normalization_safe_zero_certificates": 0,
            "source_jobs_complete": False, "stage": "whole_polynomial_orbits",
            "policy": "polynomial-orbits", "polynomial_orbit_closure_claim": True,
            "skipped": [{"reason": "unsupported", "count": 1}] * 18},
            "sector_symmetry_reduction": {"projected_terms": 17, "incomplete_boundaries": 2,
                                          "source_rows": 11, "processed_rows": 6,
                                          "saved_projection_complete": False, "combined_closure_complete": False,
                                          "all_offspring_reduced": False, "reused_rows": 2,
                                          "normalization_outputs_outside_source_inventory": 0,
                                          "saved_output_keys_outside_symmetry_inventory": 5,
                                          "saved_terminal_keys_outside_source_inventory": 7,
                                          "deferred_symmetry_sources": 7, "unresolved_output_keys": 3,
                                          "combined_closure_scope": "finite original sources; new leaves deferred",
                                          "failure_kinds": {"unsupported_rule": 2, "malformed": "2", "boolean": True},
                                          "failure_receipts": ["native-payload"],
                                          "active_row": {"task": 6, "phase": "Projecting", "elapsed_seconds": 1.5,
                                              "unique_nodes": 55, "active_nodes": 3, "pending_frames": 4,
                                              "cached_entries": 8, "equation": ["native-payload"]},
                                          "rows": ["native-payload"], "complete": False}})
        preparation = progress["sector_symmetry_preparation"]
        self.assertEqual(preparation["discovered_maps"], 7)
        self.assertEqual(preparation["support_closure_supports"], 3)
        self.assertEqual(preparation["transported_keys"], 9)
        self.assertEqual(preparation["transported_rows"], 11)
        self.assertEqual((preparation["input_keys"], preparation["processed_keys"], preparation["closure_keys"]),
                         (939, 104, 8401))
        self.assertEqual((preparation["orbit_images"], preparation["orbit_terms"]), (12, 144))
        self.assertEqual((preparation["normalization_input_keys"], preparation["normalization_canonical_keys"],
                          preparation["normalization_unit_aliases"], preparation["normalization_projected_numerators"]),
                         (939, 728, 197, 14))
        self.assertEqual(preparation["normalization_source_equations"], 211)
        self.assertEqual(preparation["legacy_source_equations"], 26)
        self.assertFalse(preparation["source_jobs_complete"])
        self.assertEqual(preparation["stage"], "whole_polynomial_orbits")
        self.assertEqual(preparation["policy"], "polynomial-orbits")
        self.assertTrue(preparation["polynomial_orbit_closure_claim"])
        self.assertNotIn("maps", preparation)
        self.assertEqual(len(preparation["skipped"]), 16)
        self.assertEqual(preparation["skipped_reasons_omitted"], 2)
        reduction = progress["sector_symmetry_reduction"]
        self.assertEqual(reduction["failure_kinds"], {"unsupported_rule": 2})
        self.assertEqual(reduction["active_row"], {"task": 6, "phase": "Projecting", "elapsed_seconds": 1.5,
            "unique_nodes": 55, "active_nodes": 3, "pending_frames": 4, "cached_entries": 8})
        self.assertNotIn("rows", reduction)
        self.assertNotIn("failure_receipts", reduction)
        self.assertFalse(reduction["combined_closure_complete"])
        self.assertEqual(reduction["saved_output_keys_outside_symmetry_inventory"], 5)
        self.assertEqual(reduction["unresolved_output_keys"], 3)
        normalized = TELEMETRY.normalize_status({"master_reduction": {"operation": "refine", **progress}})["master_reduction"]
        self.assertEqual(normalized["sector_symmetry_preparation"]["input_keys"], 939)
        self.assertEqual(normalized["sector_symmetry_preparation"]["closure_keys"], 8401)
        self.assertEqual(normalized["sector_symmetry_preparation"]["normalization_canonical_keys"], 728)
        self.assertEqual(normalized["sector_symmetry_preparation"]["normalization_unit_aliases"], 197)
        self.assertEqual(normalized["sector_symmetry_preparation"]["normalization_projected_numerators"], 14)
        self.assertFalse(normalized["sector_symmetry_reduction"]["combined_closure_complete"])
        self.assertEqual(normalized["sector_symmetry_reduction"]["failure_kinds"], {"unsupported_rule": 2})

    def test_native_accepted_sector_rows_require_the_complete_reduced_descriptor(self):
        source = {"recipe": "verified-sector-symmetry-sources-v3", "file": "sector.bin", "policy": "generators"}
        report = {"sector_symmetry_assistance": True, "sector_symmetry_certificate": source,
                  "sector_symmetry_policy": "generators",
                  "sector_symmetry_preparation": {"source_jobs_complete": False}}
        self.assertEqual(PHASES.sector_symmetry_metadata(report)["sector_symmetry_certificate"], source)
        checkpoint = {"recipe": PHASES.SECTOR_SYMMETRY_REDUCED_RECIPE, "complete": False,
                      "processed_rows": 1, "source_rows": 3, "file": "projection.bin"}
        report["sector_symmetry_projection_checkpoint"] = checkpoint
        for field in ("completed_assistance_rows", "pending_assistance_rows", "completed_assistance_keys"):
            with self.subTest(native_counter=field), self.assertRaisesRegex(ValueError, "complete reduced certificate"):
                PHASES.sector_symmetry_metadata({**report, field: 1})
        complete = {**checkpoint, "complete": True, "rows": 3, "file": "reduced.bin"}
        accepted = {**report, "completed_assistance_rows": 1, "sector_symmetry_reduced_certificate": complete}
        self.assertEqual(PHASES.sector_symmetry_metadata(accepted)["sector_symmetry_reduced_certificate"], complete)
        with self.assertRaisesRegex(ValueError, "complete reduced certificate"):
            PHASES.sector_symmetry_metadata({**accepted, "sector_symmetry_reduced_certificate": checkpoint})
        with self.assertRaisesRegex(ValueError, "complete reduced certificate"):
            PHASES.sector_symmetry_metadata({**report, "status": "completed_nonminimal",
                                           "sector_symmetry_reduction": {"complete": True}})
        with self.assertRaisesRegex(ValueError, "invalid native sector-symmetry certificate"):
            PHASES.sector_symmetry_metadata({**report, "sector_symmetry_certificate": {
                **source, "recipe": "verified-sector-symmetry-linear-maps-v1"}})
        with self.assertRaisesRegex(ValueError, "invalid native sector-symmetry policy"):
            PHASES.sector_symmetry_metadata({**report, "sector_symmetry_policy": "unknown"})
        with self.assertRaisesRegex(ValueError, "explicit source policy"):
            PHASES.sector_symmetry_metadata({key: value for key, value in report.items()
                                            if key != "sector_symmetry_policy"})
        with self.assertRaisesRegex(ValueError, "certificate policy differs"):
            PHASES.sector_symmetry_metadata({**report, "sector_symmetry_policy": "polynomial-orbits"})

    def test_source_only_sector_receipt_is_distinct_and_preserves_the_same_descriptor_slot(self):
        report = {"sector_symmetry_assistance": True, "sector_symmetry_policy": "generators",
            "sector_symmetry_saved_projection": False, "completed_assistance_rows": 1,
            "sector_symmetry_certificate": {"recipe": PHASES.SECTOR_SYMMETRY_RECIPE,
                "file": "sector.bin", "policy": "generators", "saved_projection_requested": False},
            "sector_symmetry_reduced_certificate": {"recipe": PHASES.SECTOR_SYMMETRY_SOURCE_ROWS_RECIPE,
                "file": "sector-source-rows.bin", "complete": True, "saved_projection_requested": False},
            "inherited_equation_authority": {"sector_symmetry_assistance": True,
                "sector_symmetry_policy": "generators", "sector_symmetry_saved_projection": False}}
        metadata = PHASES.sector_symmetry_metadata(report)
        self.assertIs(metadata["sector_symmetry_saved_projection"], False)
        self.assertEqual(metadata["sector_symmetry_reduced_certificate"]["recipe"], "sector-symmetry-source-rows-v1")
        self.assertNotIn("sector_symmetry_projection_checkpoint", metadata)
        for key in ("sector_symmetry_certificate", "sector_symmetry_reduced_certificate"):
            broken = {**report, key: {**report[key], "saved_projection_requested": True}}
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "saved projection differs"):
                PHASES.sector_symmetry_metadata(broken)
        with self.assertRaisesRegex(ValueError, "differs from saved projection mode"):
            PHASES.sector_symmetry_metadata({**report, "sector_symmetry_reduced_certificate": {
                **report["sector_symmetry_reduced_certificate"], "recipe": PHASES.SECTOR_SYMMETRY_REDUCED_RECIPE}})
        with self.assertRaisesRegex(ValueError, "cannot use a saved projection checkpoint"):
            PHASES.sector_symmetry_metadata({**report, "sector_symmetry_projection_checkpoint": {
                "recipe": PHASES.SECTOR_SYMMETRY_REDUCED_RECIPE, "complete": False}})

    def test_finalize_requires_stopped_current_scope_without_starting_a_solve(self):
        _, phase = self.stopped_refinement_fixture("running")
        policy = {**self.policy, "operation": "finalize-refinement"}
        with patch.object(PHASES, "phase_one") as solve, patch.object(PHASES, "phase_two") as native:
            with self.assertRaisesRegex(ValueError, "stopped paused or failed"):
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            solve.assert_not_called()
            native.assert_not_called()
        self.write("amendments/amendment-0001.json", {"rank": 1})
        with patch.object(PHASES, "phase_one") as solve, patch.object(PHASES, "phase_two") as native:
            with self.assertRaisesRegex(ValueError, "completed current input scope"):
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            solve.assert_not_called()
            native.assert_not_called()
        self.assertTrue((phase / "latest.json").is_file())

    def protocol_plan(self, binary):
        cpu = sorted(os.sched_getaffinity(0))[0]
        return {**self.plan, "steering_policy": {"command_arguments": ["--executable", str(binary)]},
                "requested_workers": 1, "cpus": str(cpu), "checkpoint_interval_seconds": 3600,
                "supervisor_ram_policy": {"max_memory_bytes": 2_000_000_000,
                    "ram_guard_margin_percent": 5, "host_memory_reserve_bytes": 1_000_000,
                    "swap_growth_stop_bytes_per_second": 0, "swap_growth_stop_seconds": 120}}

    def test_explicit_interrupted_recovery_preserves_running_source_and_allows_fresh(self):
        _, source = self.stopped_refinement_fixture("running")
        original = (source / "latest.json").read_bytes()
        plain = PHASES.postprocess_configuration(self.policy, "finalize-refinement")
        with patch.object(PHASES, "phase_two") as native, self.assertRaisesRegex(ValueError, "recover-interrupted"):
            PHASES.run(self.plan, plain, True, self.driver, postprocess_only=True)
        native.assert_not_called()
        binary = self.campaign / "fake-recover"
        binary.write_text(f"#!{sys.executable}\n" + '''import json, pathlib, sys
args = sys.argv
assert args[1] == 'walk-finalize-refinement' and '--recover-interrupted' in args
assert '--seed-depth' not in args and '--artifact' not in args
directory = pathlib.Path(args[args.index('--directory') + 1])
report = {'status':'published_unrefined','operation':'publish','inventory':{'complete':True},
          'finite_search_complete':False,'refinement_complete':False,
          'partial_refinement_source':{'status':'running','recovered_interrupted':True}}
(directory/'artifact.json').write_text(json.dumps(report))
(directory/'latest.json').write_text(json.dumps(report))
''')
        binary.chmod(0o700)
        recovery = PHASES.postprocess_configuration(self.policy, "finalize-refinement", recover_interrupted=True)
        driver = SimpleNamespace(**vars(PRODUCTION))
        with patch.object(PHASES.sys, "stdout", io.StringIO()), patch.object(PHASES.sys, "stderr", io.StringIO()):
            self.assertEqual(PHASES.run(self.protocol_plan(binary), recovery, True, driver, postprocess_only=True), 0)
        pointer = PHASES.completed_artifact(self.campaign)
        self.assertTrue(pointer["refinement"]["recovered_interrupted"])
        self.assertEqual(pointer["refinement"]["original_source_status"], "running")
        self.assertEqual((source / "latest.json").read_bytes(), original)
        saved_policy = PHASES.read_json(self.campaign / "master-reduction/policy.json")
        self.assertNotIn("recover_interrupted", saved_policy)
        PHASES.require_retained_refinement(self.campaign)
        fresh = PHASES.configuration(self.campaign, enabled=True, fresh_search=True,
                                     seed_depth=2, containing_sector_depth=0)
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, fresh, True, self.driver, postprocess_only=True)
        self.assertEqual(native.call_args.args[1]["source_artifact"], str(pointer["resolved_directory"]))
        # Neither a changed checkpoint nor an active process may be hidden by
        # the earlier recovered pointer.
        changed = PHASES.read_json(source / "latest.json")
        changed["completed_source_rows"] += 1
        self.write(str((source / "latest.json").relative_to(self.campaign)), changed)
        with patch.object(PHASES, "phase_two") as native, self.assertRaisesRegex(ValueError, "finalize-refinement"):
            PHASES.run(self.plan, fresh, True, self.driver, postprocess_only=True)
        native.assert_not_called()
        with patch.object(self.driver, "campaign_run_liveness", return_value=["foreign writer"]), \
             patch.object(PHASES, "phase_two") as native, self.assertRaisesRegex(ValueError, "still running"):
            PHASES.run(self.plan, recovery, True, self.driver, postprocess_only=True)
        native.assert_not_called()

    def test_recovery_is_explicit_nonsticky_and_has_a_distinct_output_identity(self):
        _, source = self.stopped_refinement_fixture("paused")
        targets = []
        for recover in (False, True):
            policy = PHASES.postprocess_configuration(self.policy, "finalize-refinement", recover_interrupted=recover)
            with patch.object(PHASES, "phase_two", return_value=4) as native:
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            targets.append(native.call_args.args[4])
            self.assertEqual(native.call_args.args[1]["recover_interrupted"], recover)
        self.assertNotEqual(*targets)
        expected = hashlib.sha256(json.dumps({"partial_refinement": str(source),
            "source_manifest_sha256": PHASES.digest(source / "latest.json")}, sort_keys=True).encode()).hexdigest()
        self.assertEqual(targets[0].name, expected)  # Existing ordinary export hashes remain unchanged.
        for operation in ("publish", "import-refinement"):
            with self.assertRaisesRegex(ValueError, "invalid explicit"):
                PHASES.postprocess_configuration(self.policy, operation, recover_interrupted=True)

    def test_import_recipe_is_bound_archived_and_resumed_without_original_file(self):
        source = self.publish_fixture()
        original = (source / "artifact.json").read_bytes()
        recipe = self.campaign / "proof-recipe.json"
        recipe.write_text('{"schema":"test-native-proof-recipe"}')
        recipe_bytes = recipe.read_bytes()
        policy = PHASES.postprocess_configuration(self.policy, "import-refinement", proof_recipe=recipe)
        with patch.object(PHASES, "phase_one") as solve, patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
        solve.assert_not_called()
        target = native.call_args.args[4]
        self.assertEqual(native.call_args.args[1]["source_artifact"], str(source))
        steering = PHASES.read_json(target / "steering-binding.json")
        self.assertEqual(steering["proof_recipe"]["sha256"], PHASES.digest(recipe))
        self.assertEqual(steering["source_manifest_sha256"], PHASES.digest(source / "artifact.json"))
        self.assertNotIn("proof_recipe", PHASES.read_json(self.campaign / "master-reduction/policy.json"))
        archive = target / "proof-import/recipe.json"
        archive.parent.mkdir()
        archive.write_bytes(recipe_bytes)
        self.write(str((target / "import-state.json").relative_to(self.campaign)), {"status": "paused"})
        self.write("master-reduction/active-phase.json", {"operation": "import-refinement", "directory": str(target)})
        recipe.unlink()
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
        self.assertEqual(native.call_args.args[4], target)
        self.assertEqual(native.call_args.args[1]["proof_recipe"], str(archive))
        self.assertEqual((source / "artifact.json").read_bytes(), original)
        archive.write_text('{}')
        with patch.object(PHASES, "phase_two") as native, self.assertRaisesRegex(ValueError, "archived proof recipe"):
            PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
        native.assert_not_called()
        archive.write_bytes(recipe_bytes)
        changed_recipe = self.campaign / "different-recipe.json"
        changed_recipe.write_text('{"different":true}')
        different = PHASES.postprocess_configuration(self.policy, "import-refinement", proof_recipe=changed_recipe)
        with patch.object(PHASES, "phase_two") as native, self.assertRaisesRegex(ValueError, "cannot change proof recipe"):
            PHASES.run(self.plan, different, True, self.driver, postprocess_only=True)
        native.assert_not_called()
        self.write(str((source / "artifact.json").relative_to(self.campaign)), {"inventory": {"complete": True}, "changed": True})
        with patch.object(PHASES, "phase_two") as native, self.assertRaisesRegex(ValueError, "source publication changed"):
            PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
        native.assert_not_called()

    def test_import_requires_retained_current_refinement_and_completed_scope(self):
        self.stopped_refinement_fixture("paused")
        recipe = self.campaign / "proof.json"
        recipe.write_text('{}')
        policy = PHASES.postprocess_configuration(self.policy, "import-refinement", proof_recipe=recipe)
        with patch.object(PHASES, "phase_one") as solve, patch.object(PHASES, "phase_two") as native:
            with self.assertRaisesRegex(ValueError, "finalize-refinement"):
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            self.result["recursive_worklist_exhausted"] = False
            self.write("runs/first/result.json", self.result)
            with self.assertRaisesRegex(ValueError, "completed current input scope"):
                PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
        solve.assert_not_called()
        native.assert_not_called()

    def test_import_native_protocol_pauses_before_latest_then_publishes_and_feeds_fresh(self):
        source = self.publish_fixture("refine", depth=2, containing_sector_depth=2)
        source_bytes = (source / "artifact.json").read_bytes()
        recipe = self.campaign / "proof-recipe.json"
        recipe.write_text('{"schema":"test-native-proof-recipe"}')
        binary = self.campaign / "fake-import"
        binary.write_text(f"#!{sys.executable}\n" + '''import json, pathlib, sys
args = sys.argv
assert args[1] == 'walk-import-refinement'
assert '--artifact' in args and '--proof-recipe' in args
assert not any(x in args for x in ['--command','--checkpoint','--seed-depth','--fresh-search',
    '--saved-rule-assistance','--deep-verification','--no-deep-verification','--previous-artifact'])
directory = pathlib.Path(args[args.index('--directory') + 1])
recipe = pathlib.Path(args[args.index('--proof-recipe') + 1])
events = pathlib.Path(args[args.index('--events') + 1])
archive = directory/'proof-import/recipe.json'
archive.parent.mkdir(exist_ok=True)
data = recipe.read_bytes()
archive.write_bytes(data)
if '--resume' not in args:
    (directory/'import-state.json').write_text(json.dumps({'status':'paused','proof_import':{'stage':'preparing'}}))
    with events.open('a') as out:
        out.write(json.dumps({'event':'proof_import_progress','status':'paused','stage':'replaying symmetry',
            'proof_import':{'stage':'preparing'}})+'\\n')
    sys.exit(4)
assert recipe == archive
report = {'status':'published_unrefined','operation':'publish','deep_verification':False,
    'inventory':{'complete':True,'independently_verified':False}, 'import_complete':True,
    'proof_import':{'stage':'published','recipe':{'path':'proof-import/recipe.json','blake3':'native-test-hash'},
                    'source_manifest_blake3':'source-native-test-hash'}, 'remaining_terminals':5}
(directory/'artifact.json').write_text(json.dumps(report))
(directory/'latest.json').write_text(json.dumps(report))
with events.open('a') as out:
    out.write(json.dumps({'event':'proof_import_progress','status':'published_unrefined','stage':'publishing',
        'remaining_terminals':5, 'proof_import':report['proof_import']})+'\\n')
''')
        binary.chmod(0o700)
        plan, driver = self.protocol_plan(binary), SimpleNamespace(**vars(PRODUCTION))
        # Both adapter invocations intentionally share this test interpreter;
        # no earlier supervisor process survives between the fake native calls.
        driver.campaign_run_liveness = lambda _: []
        policy = PHASES.postprocess_configuration(self.policy, "import-refinement", proof_recipe=recipe)
        with patch.object(PHASES.sys, "stdout", io.StringIO()), patch.object(PHASES.sys, "stderr", io.StringIO()):
            self.assertEqual(PHASES.run(plan, policy, True, driver, postprocess_only=True), 4)
        active = PHASES.read_json(self.campaign / "master-reduction/active-phase.json")
        target = Path(active["directory"])
        self.assertFalse((target / "latest.json").exists())
        self.assertTrue((target / "import-state.json").is_file())
        self.assertEqual(PHASES.completed_artifact(self.campaign)["resolved_directory"], source)
        recipe.unlink()
        with patch.object(PHASES.sys, "stdout", io.StringIO()), patch.object(PHASES.sys, "stderr", io.StringIO()):
            self.assertEqual(PHASES.run(plan, policy, True, driver, postprocess_only=True), 0)
        pointer = PHASES.completed_artifact(self.campaign)
        self.assertEqual(pointer["resolved_directory"], target)
        self.assertEqual(pointer["operation"], "publish")
        self.assertTrue(pointer["import_complete"])
        self.assertNotIn("refinement", pointer)  # Old (2,2) search metadata is not the new round.
        self.assertEqual((source / "artifact.json").read_bytes(), source_bytes)
        status = PRODUCTION.SUPERVISOR.MONITOR.read_status(self.campaign)
        self.assertEqual(status["master_reduction"]["operation"], "import-refinement")
        self.assertTrue(status["master_reduction"]["import_complete"])
        with patch.object(PHASES, "phase_two") as native:
            self.assertEqual(PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True), 0)
        native.assert_not_called()
        fresh = PHASES.configuration(self.campaign, enabled=True, fresh_search=True,
                                     seed_depth=2, containing_sector_depth=0)
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, fresh, True, self.driver, postprocess_only=True)
        self.assertEqual(native.call_args.args[1]["source_artifact"], str(target))
        self.assertEqual(native.call_args.args[1]["effective_containing_sector_depth"], 0)

    def test_import_success_does_not_promote_changed_recipe_or_source(self):
        source = self.publish_fixture()
        source_bytes = (source / "artifact.json").read_bytes()
        pointer_bytes = (self.campaign / "artifacts/latest.json").read_bytes()
        recipe = self.campaign / "proof.json"
        recipe.write_text('{"original":true}')
        binary = self.campaign / "fake-mutated-import"
        driver = SimpleNamespace(**vars(PRODUCTION))
        for mutation, expected in (("recipe", "archived proof recipe differs"),
                                    ("source", "source changed during replay")):
            with self.subTest(mutation=mutation):
                (source / "artifact.json").write_bytes(source_bytes)
                binary.write_text(f"#!{sys.executable}\nmutation={mutation!r}\n" + '''import json, pathlib, sys
args = sys.argv
directory = pathlib.Path(args[args.index('--directory') + 1])
source = pathlib.Path(args[args.index('--artifact') + 1])
recipe = pathlib.Path(args[args.index('--proof-recipe') + 1])
archive = directory/'proof-import/recipe.json'
archive.parent.mkdir()
archive.write_bytes(b'{}' if mutation == 'recipe' else recipe.read_bytes())
if mutation == 'source':
    (source/'artifact.json').write_text(json.dumps({'inventory':{'complete':True},'changed':True}))
report = {'status':'published_unrefined','operation':'publish','deep_verification':False,
          'inventory':{'complete':True,'independently_verified':False},
          'import_complete':True,'proof_import':{'stage':'published'}}
(directory/'artifact.json').write_text(json.dumps(report))
''')
                binary.chmod(0o700)
                directory = self.campaign / "master-reduction/scopes" / ("mutated-" + mutation)
                policy = {**self.policy, "operation": "import-refinement", "source_artifact": str(source),
                          "proof_recipe": str(recipe), "import_recipe_sha256": PHASES.digest(recipe),
                          "import_source_manifest_sha256": PHASES.digest(source / "artifact.json")}
                with patch.object(PHASES.sys, "stdout", io.StringIO()), patch.object(PHASES.sys, "stderr", io.StringIO()), \
                     self.assertRaisesRegex(ValueError, expected):
                    PHASES.phase_two(self.protocol_plan(binary), policy, self.campaign / "runs/first/request.json",
                                     self.binding(), directory, driver)
                self.assertEqual((self.campaign / "artifacts/latest.json").read_bytes(), pointer_bytes)

    def test_partial_export_retains_cursor_updates_both_pointers_and_resumes_same_refinement(self):
        source, phase = self.stopped_refinement_fixture()
        source_manifest = (phase / "latest.json").read_bytes()
        active_path = self.campaign / "master-reduction/active-phase.json"
        active = active_path.read_bytes()
        with self.assertRaisesRegex(ValueError, "finalize-refinement"):
            PHASES.require_retained_refinement(self.campaign)
        binary = self.campaign / "fake-export"
        binary.write_text(f"#!{sys.executable}\n" + '''import json, pathlib, sys
args = sys.argv
assert args[1] == 'walk-finalize-refinement'
assert '--artifact' not in args and '--seed-depth' not in args and '--command' not in args
directory = pathlib.Path(args[args.index('--directory') + 1])
source = pathlib.Path(args[args.index('--refinement') + 1])
report = {'status':'published_unrefined', 'operation':'publish',
          'inventory':{'complete':True,'independently_verified':False},
          'finite_search_complete':False, 'refinement_complete':False,
          'partial_refinement_source':{'checkpoint':7, 'last_error':'retained failure'}}
(directory/'artifact.json').write_text(json.dumps(report))
(directory/'latest.json').write_text(json.dumps(report))
''')
        binary.chmod(0o700)
        cpu = sorted(os.sched_getaffinity(0))[0]
        plan = {**self.plan, "steering_policy": {"command_arguments": ["--executable", str(binary)]},
                "requested_workers": 1, "cpus": str(cpu), "checkpoint_interval_seconds": 3600,
                "supervisor_ram_policy": {"max_memory_bytes": 2_000_000_000,
                    "ram_guard_margin_percent": 5, "host_memory_reserve_bytes": 1_000_000,
                    "swap_growth_stop_bytes_per_second": 0, "swap_growth_stop_seconds": 120}}
        driver = SimpleNamespace(**vars(PRODUCTION))
        policy = {**self.policy, "operation": "finalize-refinement"}
        with patch.object(PHASES.sys, "stdout", io.StringIO()), patch.object(PHASES.sys, "stderr", io.StringIO()):
            self.assertEqual(PHASES.run(plan, policy, True, driver, postprocess_only=True), 0)
        pointer = PHASES.completed_artifact(self.campaign)
        self.assertEqual(pointer["status"], "published_partial_refinement")
        self.assertFalse(pointer["finite_search_complete"])
        self.assertFalse(pointer["refinement_complete"])
        self.assertFalse(pointer["master_minimality_claim"])
        self.assertEqual(active_path.read_bytes(), active)
        self.assertEqual((phase / "latest.json").read_bytes(), source_manifest)
        completed = self.campaign / "master-reduction/completed-phase.json"
        self.assertEqual(PHASES.read_json(completed)["directory"], str(pointer["resolved_directory"]))
        PHASES.require_retained_refinement(self.campaign)
        # An interrupted pointer update is repairable without another export.
        completed.unlink()
        with self.assertRaisesRegex(ValueError, "finalize-refinement"):
            PHASES.require_retained_refinement(self.campaign)
        with patch.object(PHASES, "phase_two") as native:
            self.assertEqual(PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True), 0)
            native.assert_not_called()
        PHASES.require_retained_refinement(self.campaign)
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True,
                       self.driver, postprocess_only=True)
            self.assertEqual(native.call_args.args[4], phase)
            self.assertEqual(native.call_args.args[1]["source_artifact"], str(source))
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True, seed_depth=1), True,
                       self.driver, postprocess_only=True)
            self.assertNotEqual(native.call_args.args[4], phase)
            self.assertEqual(native.call_args.args[1]["source_artifact"], str(pointer["resolved_directory"]))
        # New rows after finalization need another explicit export before extension.
        latest = PHASES.read_json(phase / "latest.json")
        latest["completed_source_rows"] += 1
        self.write(str((phase / "latest.json").relative_to(self.campaign)), latest)
        with self.assertRaisesRegex(ValueError, "finalize-refinement"):
            PHASES.require_retained_refinement(self.campaign)

    def test_default_resume_preserves_same_scope_refined_artifact_without_rerun(self):
        self.publish_fixture("refine", 1)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two") as second:
            self.assertEqual(PHASES.run(self.plan, self.policy, True, self.driver), 0)
        first.assert_not_called()
        second.assert_not_called()
        self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["operation"], "refine")

    def test_deep_publication_upgrades_fast_artifact_and_preserves_refinement(self):
        source = self.publish_fixture("refine", depth=1, deep_verification=False)
        deep = PHASES.configuration(self.campaign, deep_verification=True)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as native:
            self.assertEqual(PHASES.run(self.plan, deep, True, self.driver, postprocess_only=True), 4)
            policy = native.call_args.args[1]
            self.assertEqual(policy["previous_artifact"], str(source))
            self.assertEqual(policy["preserved_refinement"]["seed_depth"], 1)
            binding = PHASES.read_json(native.call_args.args[4] / "steering-binding.json")
            self.assertTrue(binding["deep_verification"])
        first.assert_not_called()

    def test_verified_legacy_or_new_artifact_satisfies_fast_and_deep_requests(self):
        for prior in (None, True):
            self.publish_fixture(deep_verification=prior)
            for choice in (False, True):
                policy = PHASES.configuration(self.campaign, deep_verification=choice)
                with patch.object(PHASES, "phase_two") as native:
                    self.assertEqual(PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True), 0)
                native.assert_not_called()

    def test_publication_modes_have_separate_resume_identities(self):
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, self.policy, True, self.driver)
            fast = native.call_args.args[4]
            deep = PHASES.configuration(self.campaign, deep_verification=True)
            PHASES.run(self.plan, deep, True, self.driver)
            self.assertNotEqual(native.call_args.args[4], fast)
            self.assertTrue(native.call_args.args[1]["deep_verification"])
            PHASES.run(self.plan, PHASES.configuration(self.campaign), True, self.driver)
            self.assertTrue(native.call_args.args[1]["deep_verification"])

    def test_assistance_mode_has_a_distinct_collection_checkpoint_identity(self):
        source = self.publish_fixture()
        source_bytes = (source / "artifact.json").read_bytes()
        ordinary = PHASES.configuration(self.campaign, enabled=True)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as second:
            PHASES.run(self.plan, ordinary, True, self.driver, postprocess_only=True)
            old_directory = second.call_args.args[4]
            legacy_identity = {"scope": self.binding()["key"], "operation": "refine", "seed_depth": 0,
                               "collection_strategy": PHASES.COLLECTION_STRATEGY, "collection_inputs": [],
                               "finite_feedback": True, "finite_feedback_recipe": PHASES.FINITE_FEEDBACK_RECIPE,
                               "source": str(source.relative_to(self.campaign)), "executable": None}
            expected = hashlib.sha256(json.dumps(legacy_identity, sort_keys=True).encode()).hexdigest()
            self.assertEqual(old_directory.name, expected)
            (old_directory / "latest.json").write_text("{}")
            assisted = PHASES.configuration(self.campaign, enabled=True, saved_rule_assistance=True)
            PHASES.run(self.plan, assisted, True, self.driver, postprocess_only=True)
            assisted_directory = second.call_args.args[4]
            self.assertNotEqual(assisted_directory, old_directory)
            self.assertFalse((assisted_directory / "latest.json").exists())
            self.assertEqual(second.call_args.args[1]["source_artifact"], str(source))
            self.assertTrue(PHASES.read_json(assisted_directory / "steering-binding.json")["saved_rule_assistance"])
            resumed = PHASES.configuration(self.campaign, enabled=True)
            PHASES.run(self.plan, resumed, True, self.driver, postprocess_only=True)
            self.assertEqual(second.call_args.args[4], assisted_directory)
            ordinary = PHASES.configuration(self.campaign, enabled=True, saved_rule_assistance=False)
            PHASES.run(self.plan, ordinary, True, self.driver, postprocess_only=True)
            self.assertEqual(second.call_args.args[4], old_directory)
        first.assert_not_called()
        self.assertEqual((source / "artifact.json").read_bytes(), source_bytes)

    def test_modular_recipe_is_distinct_from_exact_and_grants_are_operational(self):
        self.publish_fixture()
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True, self.driver, postprocess_only=True)
            exact = native.call_args.args[4]
            modular = PHASES.configuration(self.campaign, enabled=True, refinement_strategy="modular-circuits")
            PHASES.run(self.plan, modular, True, self.driver, postprocess_only=True)
            target = native.call_args.args[4]
            self.assertNotEqual(target, exact)
            identity = PHASES.read_json(target / "steering-binding.json")
            self.assertEqual(identity["modular_search_recipe"], PHASES.MODULAR_SEARCH_RECIPE)
            path = self.campaign / "modular-grants.json"
            path.write_text('{"reconstruction":{"max_primes":32}}')
            larger = PHASES.configuration(self.campaign, enabled=True, modular_policy=path)
            PHASES.run(self.plan, larger, True, self.driver, postprocess_only=True)
            self.assertEqual(native.call_args.args[4], target)

    def test_completed_modular_pointer_keeps_exact_search_incomplete_and_allows_retry(self):
        source = self.publish_fixture("refine")
        report = PHASES.read_json(source / "artifact.json")
        report.update(status="completed_nonminimal", refinement_strategy="modular-circuits",
                      finite_search_complete=False, modular_search_complete=True, refinement_complete=True)
        PRODUCTION.write_json(source / "artifact.json", report)
        refinement = PHASES.completed_artifact(self.campaign)["refinement"]
        refinement.update(refinement_strategy="modular-circuits", modular_search_complete=True)
        PHASES.publish_pointer(self.campaign, source, self.binding(), "refine", self.driver, refinement)
        pointer = PHASES.completed_artifact(self.campaign)
        self.assertFalse(pointer["finite_search_complete"])
        self.assertTrue(pointer["refinement_complete"])
        policy = PHASES.configuration(self.campaign, enabled=True, refinement_strategy="modular-circuits")
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            self.assertEqual(PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True), 0)
            native.assert_not_called()
            grants = self.campaign / "larger-grants.json"
            grants.write_text('{"reconstruction":{"max_primes":32}}')
            retry = PHASES.configuration(self.campaign, enabled=True, modular_policy=grants)
            self.assertEqual(PHASES.run(self.plan, retry, True, self.driver, postprocess_only=True), 4)
            self.assertEqual(native.call_args.args[1]["source_artifact"], str(source))
            exact = PHASES.configuration(self.campaign, enabled=True, refinement_strategy="exact")
            self.assertEqual(PHASES.run(self.plan, exact, True, self.driver, postprocess_only=True), 4)

    def test_completed_refinement_only_satisfies_the_same_assistance_mode(self):
        for previous_mode in (False, True):
            with self.subTest(previous_mode=previous_mode):
                source = self.publish_fixture("refine", 0, previous_mode)
                same = PHASES.configuration(self.campaign, enabled=True, saved_rule_assistance=previous_mode)
                changed = PHASES.configuration(self.campaign, enabled=True, saved_rule_assistance=not previous_mode)
                with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as second:
                    self.assertEqual(PHASES.run(self.plan, same, True, self.driver, postprocess_only=True), 0)
                    second.assert_not_called()
                    self.assertEqual(PHASES.run(self.plan, changed, True, self.driver, postprocess_only=True), 4)
                    self.assertEqual(second.call_args.args[1]["source_artifact"], str(source))
                    self.assertEqual(second.call_args.args[1]["saved_rule_assistance"], not previous_mode)
                first.assert_not_called()

    def test_completed_old_refinement_does_not_skip_new_collection_stage(self):
        self.publish_fixture("refine", 0)
        pointer = PHASES.read_json(self.campaign / "artifacts/latest.json")
        pointer["refinement"].pop("collection_strategy")
        self.write("artifacts/latest.json", pointer)
        policy = PHASES.configuration(self.campaign, enabled=True)
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            self.assertEqual(PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True), 4)
        native.assert_called_once()

    def test_feedback_recipe_and_mode_prevent_stale_refinement_reuse(self):
        for previous_mode in (False, True):
            self.publish_fixture("refine", finite_feedback=previous_mode)
            same = PHASES.configuration(self.campaign, enabled=True, finite_feedback=previous_mode)
            changed = PHASES.configuration(self.campaign, enabled=True, finite_feedback=not previous_mode)
            with patch.object(PHASES, "phase_two", return_value=4) as native:
                self.assertEqual(PHASES.run(self.plan, same, True, self.driver, postprocess_only=True), 0)
                native.assert_not_called()
                self.assertEqual(PHASES.run(self.plan, changed, True, self.driver, postprocess_only=True), 4)
                self.assertEqual(native.call_args.args[1]["finite_feedback"], not previous_mode)
        pointer = PHASES.read_json(self.campaign / "artifacts/latest.json")
        pointer["refinement"]["finite_feedback_recipe"] = "obsolete"
        self.write("artifacts/latest.json", pointer)
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            self.assertEqual(PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True, finite_feedback=True),
                                       True, self.driver, postprocess_only=True), 4)
        native.assert_called_once()

    def test_feedback_mode_has_distinct_checkpoint_and_repeat_resumes_it(self):
        self.publish_fixture()
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True, self.driver)
            enabled = native.call_args.args[4]
            baseline = PHASES.configuration(self.campaign, enabled=True, finite_feedback=False)
            PHASES.run(self.plan, baseline, True, self.driver)
            disabled = native.call_args.args[4]
            self.assertNotEqual(enabled, disabled)
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True, self.driver)
            self.assertEqual(native.call_args.args[4], disabled)
            self.assertFalse(PHASES.read_json(disabled / "steering-binding.json")["finite_feedback"])

    def test_collection_peer_inputs_require_refine_and_are_content_bound(self):
        self.publish_fixture()
        peer = self.campaign / "peer-artifact"
        self.write("peer-artifact/artifact.json", {"native_state": {"blake3": "first"}})
        with self.assertRaisesRegex(ValueError, "explicit"):
            PHASES.configuration(self.campaign, collection_artifacts=[peer])
        policy = PHASES.configuration(self.campaign, enabled=True, collection_artifacts=[peer])
        with patch.object(PHASES, "phase_two", return_value=4) as native:
            PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            first_directory = native.call_args.args[4]
            self.assertEqual(native.call_args.args[1]["collection_artifacts"], [str(peer)])
            self.write("peer-artifact/artifact.json", {"native_state": {"blake3": "changed"}})
            PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
            self.assertNotEqual(native.call_args.args[4], first_directory)
    def test_standard_profile_has_distinct_phase_and_omitted_repeat_uses_saved_preference(self):
        source = self.publish_fixture()
        original = (source / "artifact.json").read_bytes()
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as second:
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True, self.driver, postprocess_only=True)
            conservative = second.call_args.args[4]
            self.assertEqual(second.call_args.args[1]["effective_normalization_profile"], "conservative-v1")
            self.assertNotIn("normalization_profile", PHASES.read_json(conservative / "steering-binding.json"))
            standard = PHASES.configuration(self.campaign, enabled=True, normalization_profile="standard")
            PHASES.run(self.plan, standard, True, self.driver, postprocess_only=True)
            standard_directory = second.call_args.args[4]
            self.assertNotEqual(standard_directory, conservative)
            self.assertEqual(PHASES.read_json(standard_directory / "steering-binding.json")["normalization_profile"], "standard-v1")
            (standard_directory / "latest.json").write_text('{"normalization_profile":"standard-v1"}')
            PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True, self.driver, postprocess_only=True)
            self.assertEqual(second.call_args.args[4], standard_directory)
            self.assertEqual(second.call_args.args[1]["effective_normalization_profile"], "standard-v1")
        first.assert_not_called()
        self.assertEqual((source / "artifact.json").read_bytes(), original)

    def test_source_profile_inheritance_and_explicit_switch_preserve_effective_depths(self):
        for source_profile, requested in (("standard-v1", "conservative"), ("conservative-v1", "standard")):
            with self.subTest(source_profile=source_profile):
                source = self.publish_fixture("refine", depth=2, containing_sector_depth=2,
                                              normalization_profile=source_profile)
                original = (source / "artifact.json").read_bytes()
                # Old pointer metadata can omit the profile; native metadata remains authoritative.
                pointer = PHASES.read_json(self.campaign / "artifacts/latest.json")
                pointer.pop("normalization_profile")
                pointer.pop("normalization_limits")
                self.write("artifacts/latest.json", pointer)
                with patch.object(PHASES, "phase_two", return_value=4) as second:
                    self.assertEqual(PHASES.run(self.plan, {**self.policy, "operation": "refine"}, True,
                                               self.driver, postprocess_only=True), 0)
                    second.assert_not_called()
                    changed = PHASES.configuration(self.campaign, enabled=True, normalization_profile=requested)
                    self.assertEqual(PHASES.run(self.plan, changed, True, self.driver, postprocess_only=True), 4)
                    launched = second.call_args.args[1]
                    self.assertEqual(launched["effective_normalization_profile"], requested + "-v1")
                    self.assertEqual(launched["effective_seed_depth"], 2)
                    self.assertEqual(launched["effective_containing_sector_depth"], 2)
                    remembered = PHASES.configuration(self.campaign, enabled=True)
                    PHASES.run(self.plan, remembered, True, self.driver, postprocess_only=True)
                    self.assertEqual(second.call_args.args[1]["effective_normalization_profile"], requested + "-v1")
                self.assertEqual((source / "artifact.json").read_bytes(), original)

    def test_profile_and_both_provider_flags_have_independent_phase_bindings(self):
        self.publish_fixture()
        directories = set()
        for profile in ("conservative", "standard"):
            for saved, circuit in ((False, False), (True, False), (False, True), (True, True)):
                policy = PHASES.configuration(self.campaign, enabled=True, normalization_profile=profile,
                                              saved_rule_assistance=saved, circuit_symmetry_assistance=circuit)
                with patch.object(PHASES, "phase_two", return_value=4) as second:
                    PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True)
                    directories.add(second.call_args.args[4])
        self.assertEqual(len(directories), 8)

    def test_pointer_profile_and_limits_must_agree_with_native_artifact(self):
        self.publish_fixture(normalization_profile="standard-v1")
        pointer = PHASES.read_json(self.campaign / "artifacts/latest.json")
        self.assertEqual(pointer["normalization_profile"], "standard-v1")
        self.assertEqual(pointer["normalization_limits"], {"fixture": True})
        for mismatch in ({"normalization_profile": "conservative-v1"}, {"normalization_limits": {}}):
            self.write("artifacts/latest.json", {**pointer, **mismatch})
            with self.assertRaisesRegex(ValueError, "differ.* from native"):
                PHASES.completed_artifact(self.campaign)

    def test_containing_sector_search_isolated_checkpoint_and_inherited_effective_depth(self):
        source = self.publish_fixture()
        ordinary = PHASES.configuration(self.campaign, enabled=True)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as second:
            PHASES.run(self.plan, ordinary, True, self.driver, postprocess_only=True)
            ordinary_directory = second.call_args.args[4]
            extra = PHASES.configuration(self.campaign, enabled=True, containing_sector_depth=1)
            PHASES.run(self.plan, extra, True, self.driver, postprocess_only=True)
            extra_directory = second.call_args.args[4]
            self.assertNotEqual(ordinary_directory, extra_directory)
            self.assertEqual(PHASES.read_json(extra_directory / "steering-binding.json")["containing_sector_depth"], 1)
            resumed = PHASES.configuration(self.campaign, enabled=True)
            PHASES.run(self.plan, resumed, True, self.driver, postprocess_only=True)
            self.assertEqual(second.call_args.args[4], extra_directory)
            self.publish_fixture("refine", depth=2, containing_sector_depth=2)
            second.reset_mock()
            self.assertEqual(PHASES.run(self.plan, extra, True, self.driver, postprocess_only=True), 0)
            second.assert_not_called()
            changed = {**extra, "saved_rule_assistance": True}
            PHASES.run(self.plan, changed, True, self.driver, postprocess_only=True)
            launched = second.call_args.args[1]
            self.assertEqual(launched["source_artifact"], str(source))
            self.assertEqual(launched["containing_sector_depth"], 1)  # Requested depth binds resume.
            self.assertEqual(launched["effective_containing_sector_depth"], 2)
            self.assertEqual(launched["effective_seed_depth"], 2)
        first.assert_not_called()

    def test_all_assistance_modes_have_distinct_checkpoints_and_exact_reuse_policy(self):
        source = self.publish_fixture()
        source_bytes = (source / "artifact.json").read_bytes()
        directories = set()
        for saved, circuit in ((False, False), (True, False), (False, True), (True, True)):
            policy = PHASES.configuration(self.campaign, enabled=True, saved_rule_assistance=saved,
                                          circuit_symmetry_assistance=circuit)
            with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as second:
                self.assertEqual(PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True), 4)
                directory = second.call_args.args[4]
                self.assertNotIn(directory, directories)
                directories.add(directory)
                (directory / "latest.json").write_text("{}")
                restored = PHASES.configuration(self.campaign, enabled=True)
                PHASES.run(self.plan, restored, True, self.driver, postprocess_only=True)
                self.assertEqual(second.call_args.args[4], directory)
                first.assert_not_called()
        self.assertEqual((source / "artifact.json").read_bytes(), source_bytes)
        for saved, circuit in ((False, False), (True, False), (False, True), (True, True)):
            self.publish_fixture("refine", saved_rule_assistance=saved, circuit_symmetry_assistance=circuit)
            same = PHASES.configuration(self.campaign, enabled=True, saved_rule_assistance=saved,
                                        circuit_symmetry_assistance=circuit)
            changed = {**same, "circuit_symmetry_assistance": not circuit}
            with patch.object(PHASES, "phase_two", return_value=4) as second:
                self.assertEqual(PHASES.run(self.plan, same, True, self.driver, postprocess_only=True), 0)
                second.assert_not_called()
                self.assertEqual(PHASES.run(self.plan, changed, True, self.driver, postprocess_only=True), 4)
                self.assertEqual(second.call_args.args[1]["circuit_symmetry_assistance"], not circuit)

    def test_completed_ordinary_search_does_not_satisfy_containing_sector_request(self):
        self.publish_fixture("refine", depth=2)
        extra = PHASES.configuration(self.campaign, enabled=True, containing_sector_depth=1)
        with patch.object(PHASES, "phase_two", return_value=4) as second:
            self.assertEqual(PHASES.run(self.plan, extra, True, self.driver, postprocess_only=True), 4)
        self.assertEqual(second.call_args.args[1]["containing_sector_depth"], 1)

    def test_extension_prevents_refining_old_scope(self):
        self.publish_fixture()
        self.write("amendments/amendment-0001.json", {"rank": 1})
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two") as second:
            with self.assertRaisesRegex(ValueError, "current scope"):
                PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True,
                           self.driver, postprocess_only=True)
        first.assert_not_called()
        second.assert_not_called()

    def test_interruption_at_phase_one_boundary_does_not_begin_phase_two(self):
        for native_status in (0, 4):
            with patch.object(PHASES, "phase_one", return_value=(native_status, True)), patch.object(PHASES, "phase_two") as second:
                self.assertEqual(PHASES.run(self.plan, self.policy, False, self.driver), 4)
            second.assert_not_called()

    def test_dispatcher_lock_rejects_parallel_launch(self):
        target = self.campaign / "master-reduction"
        with PHASES.dispatcher_lock(target):
            with self.assertRaisesRegex(ValueError, "running phase dispatcher"):
                with PHASES.dispatcher_lock(target):
                    self.fail("second dispatcher admitted")

    def test_initial_dispatcher_creation_is_serialized_with_scope_extension(self):
        (self.checkpoint / "checkpoint.lock").touch()
        with PRODUCTION.checkpoint_lock(self.checkpoint):
            with self.assertRaisesRegex(ValueError, "in use"):
                PHASES.run(self.plan, self.policy, True, self.driver)
        self.assertFalse((self.campaign / "master-reduction/dispatcher.lock").exists())

    def test_phase_one_ctrl_c_forwards_to_only_its_owned_supervisor(self):
        child = unittest.mock.Mock(pid=456)
        child.poll.return_value = None
        installed = {}
        def install(sig, handler):
            previous = installed.get(sig)
            installed[sig] = handler
            return previous
        def wait():
            installed[signal.SIGINT](signal.SIGINT, None)
            return 4
        child.wait.side_effect = wait
        with patch.object(PHASES.subprocess, "Popen", return_value=child), patch.object(PHASES.signal, "signal", side_effect=install), patch.object(PHASES.os, "killpg") as send:
            self.assertEqual(PHASES.phase_one(["owned-supervisor"]), (4, True))
        send.assert_called_once_with(456, signal.SIGINT)

    def test_interrupt_during_spawn_is_forwarded_after_child_exists(self):
        child = unittest.mock.Mock(pid=456)
        child.poll.return_value = None
        child.wait.return_value = 4
        installed = {}
        def install(sig, handler):
            previous = installed.get(sig)
            installed[sig] = handler
            return previous
        def spawn(*args, **kwargs):
            installed[signal.SIGINT](signal.SIGINT, None)
            return child
        with patch.object(PHASES.subprocess, "Popen", side_effect=spawn), patch.object(PHASES.signal, "signal", side_effect=install), patch.object(PHASES.os, "killpg") as send:
            self.assertEqual(PHASES.phase_one(["owned-supervisor"]), (4, True))
        send.assert_called_once_with(456, signal.SIGINT)

    def test_real_scratch_child_saves_on_sigint_and_resume_returns_to_phase_two(self):
        self._scratch_native_pause_resume("publish")

    def test_finite_postprocess_guard_keeps_process_limit_with_lower_host_availability(self):
        # Stop at guard construction: exercise real phase admission without
        # launching a child or claiming publication/refinement correctness.
        driver = SimpleNamespace(**vars(PRODUCTION))
        plan = {**self.plan, "engine": "rustred.finite-demand.v1",
                "finite_policy": str(self.campaign / "finite-policy.json"),
                "steering_policy": {"command_arguments": ["--executable", "unused-native"]},
                "requested_workers": 1, "cpus": str(min(os.sched_getaffinity(0))),
                "checkpoint_interval_seconds": 300,
                "supervisor_ram_policy": {"memory_policy": "process-limit",
                    "max_memory_bytes": 600_000_000_000, "ram_guard_margin_percent": 5,
                    "host_memory_reserve_bytes": 150_000_000_000,
                    "swap_growth_stop_bytes_per_second": 0}}
        host = {"available_bytes": 100_000_000_000, "host_available_bytes": 100_000_000_000}
        for operation in ("publish", "refine", "finalize-refinement"):
            with self.subTest(operation=operation):
                policy = {**self.policy, "operation": operation,
                          "source_artifact": str(self.campaign / "source-artifact"),
                          "partial_source": str(self.campaign / "paused-refinement")}
                directory = self.campaign / "master-reduction/scopes" / operation
                with patch.object(driver.SUPERVISOR, "host_memory", return_value=host), \
                     patch.object(driver.SUPERVISOR, "RamGuard", side_effect=RuntimeError("guard probe")) as guard, \
                     patch.object(driver.SUPERVISOR, "owned_process") as child:
                    with self.assertRaisesRegex(RuntimeError, "guard probe"):
                        PHASES.phase_two(plan, policy, self.campaign / "runs/first/request.json",
                                         self.binding(), directory, driver)
                    guard.assert_called_once_with(600_000_000_000, 570_000_000_000, 0, 0,
                                                   driver.SUPERVISOR.DEFAULT_SWAP_GROWTH_STOP_SECONDS)
                    child.assert_not_called()

    def test_deep_publication_saves_and_resumes_its_own_mode(self):
        self._scratch_native_pause_resume("publish", deep_verification=True)

    def test_deep_upgrade_keeps_refined_pointer_after_native_publication(self):
        self._scratch_native_pause_resume("publish", deep_verification=True, preserve_refined=True)

    def test_explicit_refinement_saves_and_resumes_without_replacing_publication_on_pause(self):
        self._scratch_native_pause_resume("refine")

    def test_refiner_only_128_worker_override_reaches_native_and_keeps_closure_policy(self):
        cpus=sorted(os.sched_getaffinity(0))[:128]
        if len(cpus)<128: self.skipTest("requires 128 permitted CPU IDs for the protocol control")
        self._scratch_native_pause_resume("refine", refinement_workers=128,
                                         refinement_cpus=",".join(map(str,cpus)))

    def test_assisted_refinement_forwards_mode_and_resumes_its_checkpoint(self):
        self._scratch_native_pause_resume("refine", saved_rule_assistance=True)

    def test_explicit_unlimited_column_budget_reaches_native_resume(self):
        self._scratch_native_pause_resume("refine", max_relation_columns=0)

    def test_publication_does_not_enable_saved_refinement_preference(self):
        self._scratch_native_pause_resume("publish", saved_rule_assistance=True, containing_sector_depth=1,
                                          circuit_symmetry_assistance=True, normalization_profile="standard")

    def test_standard_profile_refinement_is_pinned_across_pause_and_resume(self):
        self._scratch_native_pause_resume("refine", normalization_profile="standard")

    def test_circuit_refinement_forwards_mode_and_resumes(self):
        self._scratch_native_pause_resume("refine", circuit_symmetry_assistance=True)

    def test_combined_refinement_forwards_both_modes_and_resumes(self):
        self._scratch_native_pause_resume("refine", saved_rule_assistance=True,
                                          circuit_symmetry_assistance=True)

    def test_containing_sector_refinement_forwards_depth_and_resumes_its_checkpoint(self):
        self._scratch_native_pause_resume("refine", containing_sector_depth=1, inherited_containing_depth=2)

    def test_fresh_refinement_forwards_mode_and_resumes_without_old_shape_in_ui(self):
        self._scratch_native_pause_resume("refine", fresh_search=True, seed_depth=3,
                                         containing_sector_depth=0, inherited_containing_depth=2)

    def test_numerator_symmetry_refinement_forwards_mode_and_resumes_its_checkpoint(self):
        self._scratch_native_pause_resume("refine", numerator_symmetry_assistance=True)

    def test_fresh_search_can_explicitly_enable_numerator_symmetry(self):
        self._scratch_native_pause_resume("refine", fresh_search=True, seed_depth=3,
                                         numerator_symmetry_assistance=True)

    def test_sector_symmetry_refinement_forwards_mode_and_resumes_its_checkpoint(self):
        self._scratch_native_pause_resume("refine", sector_symmetry_assistance=True)

    def test_fresh_search_accepts_strong_and_legacy_symmetry_together(self):
        self._scratch_native_pause_resume("refine", fresh_search=True, seed_depth=3,
                                         sector_symmetry_assistance=True, numerator_symmetry_assistance=True)

    def test_polynomial_orbit_policy_forwards_and_rejects_changed_resume(self):
        self._scratch_native_pause_resume("refine", sector_symmetry_assistance=True,
                                         sector_symmetry_policy="polynomial-orbits")

    def test_source_only_sector_refinement_forwards_and_rejects_changed_resume(self):
        self._scratch_native_pause_resume("refine", sector_symmetry_assistance=True,
                                         sector_symmetry_saved_projection=False)

    def test_selective_feedback_native_round_receipts_pause_resume_and_publish(self):
        self._scratch_native_pause_resume("refine", sector_symmetry_assistance=True,
                                         selective_symmetry_feedback=True)

    def test_local_dimension_native_cohort_pauses_resumes_and_exports_standalone(self):
        self._scratch_native_pause_resume("refine", local_dimensional_recurrence_assistance=True)

    def test_local_dimension_native_cohort_coexists_with_selective_symmetry(self):
        self._scratch_native_pause_resume("refine", sector_symmetry_assistance=True,
            selective_symmetry_feedback=True, local_dimensional_recurrence_assistance=True)

    def _scratch_native_pause_resume(self, operation, saved_rule_assistance=False,
                                    containing_sector_depth=0, inherited_containing_depth=0,
                                    circuit_symmetry_assistance=False, normalization_profile=None,
                                    deep_verification=False, preserve_refined=False, max_relation_columns=None,
                                    refinement_workers=None, refinement_cpus=None, fresh_search=False, seed_depth=0,
                                    numerator_symmetry_assistance=False, sector_symmetry_assistance=False,
                                    sector_symmetry_policy="generators", sector_symmetry_saved_projection=True,
                                    selective_symmetry_feedback=False, local_dimensional_recurrence_assistance=False):
        # Fake native protocol only; this proves process/checkpoint orchestration,
        # not mathematical correctness or a successful native Rust solve.
        binary = self.campaign / "fake-native"
        binary.write_text(f"#!{sys.executable}\nexpected_assistance = {saved_rule_assistance and operation == 'refine'}\n"
                          f"expected_threads = {refinement_workers or 1}\n"
                          f"expected_circuit = {circuit_symmetry_assistance and operation == 'refine'}\n"
                          f"expected_profile = {repr(normalization_profile if operation == 'refine' else None)}\n"
                          f"expected_deep = {deep_verification}\n"
                          f"expected_previous = {preserve_refined}\n"
                          f"expected_column_budget = {max_relation_columns!r}\n"
                          f"expected_fresh = {fresh_search}\n"
                          f"expected_seed_depth = {seed_depth}\n"
                          f"expected_numerator = {numerator_symmetry_assistance and operation == 'refine'}\n"
                          f"expected_sector = {sector_symmetry_assistance and operation == 'refine'}\n"
                          f"expected_sector_policy = {sector_symmetry_policy!r}\n"
                          f"expected_sector_projection = {sector_symmetry_saved_projection}\n"
                          f"expected_selective = {selective_symmetry_feedback and operation == 'refine'}\n"
                          f"expected_local_dim = {local_dimensional_recurrence_assistance and operation == 'refine'}\n"
                          f"expected_containing_depth = {containing_sector_depth if operation == 'refine' else 0}\n" + '''import json, pathlib, sys, time
args = sys.argv
assert int(args[args.index('--threads') + 1]) == expected_threads
refine = args[1] == 'walk-master-reduce'
assert ('--seed-depth' in args) == refine
assert ('--fresh-search' in args) == expected_fresh
if refine:
    assert int(args[args.index('--seed-depth') + 1]) == expected_seed_depth
assert ('--saved-rule-assistance' in args) == expected_assistance
assert ('--circuit-symmetry-assistance' in args) == expected_circuit
assert ('--numerator-symmetry-assistance' in args) == expected_numerator
assert args.count('--sector-symmetry-assistance') == int(expected_sector)
assert args.count('--sector-symmetry-policy') == int(expected_sector)
if expected_sector:
    assert args[args.index('--sector-symmetry-policy') + 1] == expected_sector_policy
assert args.count('--sector-symmetry-saved-projection') == int(expected_sector and expected_sector_projection)
assert args.count('--no-sector-symmetry-saved-projection') == int(expected_sector and not expected_sector_projection)
assert args.count('--selective-symmetry-feedback') == int(expected_selective)
assert args.count('--local-dimensional-recurrence-assistance') == int(expected_local_dim)
assert ('--finite-feedback' in args) == refine
assert ('--max-relation-columns' in args) == (refine and expected_column_budget is not None)
if expected_column_budget is not None:
    assert int(args[args.index('--max-relation-columns') + 1]) == expected_column_budget
assert ('--deep-verification' in args) == (not refine and expected_deep)
assert ('--no-deep-verification' in args) == (not refine and not expected_deep)
assert ('--previous-artifact' in args) == (expected_previous and '--resume' not in args)
assert ('--normalization-profile' in args) == (expected_profile is not None)
if expected_profile is not None:
    assert args[args.index('--normalization-profile') + 1] == expected_profile
profile = (expected_profile or 'conservative') + '-v1'
metadata = {'normalization_profile': profile, 'normalization_limits': {'fixture': True}}
metadata.update(finite_feedback=refine, finite_feedback_recipe='finite-row-feedback-v1')
if expected_local_dim:
    metadata.update(local_dimensional_recurrence_assistance=True,
        local_dimensional_recurrence_certificate={'recipe':'verified-local-dimensional-recurrence-v1',
            'file':'local-dimensional.rrbin','bytes':22,'blake3':'e'*64},
        local_dimensional_recurrence_preparation={'input_keys':4,'processed_keys':4,
            'nominated_directions':6,'supported_geometries':2,'weighted_source_rows':7,
            'equations':3,'output_terms':10,'complete':True,'native_witnesses':[100]*1000})
if expected_selective:
    metadata.update(selective_symmetry_feedback_enabled=True, selective_symmetry_feedback={
        'recipe':'selective-symmetry-ibp-feedback-v1','stage':'executing','completed_rounds':1,
        'round':2,'selected_targets':5,'completed_targets':3,'pending_targets':2,
        'selected_queue_exhausted':False,'source_jobs_complete':True,'ordinary_work_complete':False,
        'rounds':[{'round':1,'stage':'complete','parent_native_state':
            {'file':'round-1-parent.rrbin','bytes':12,'blake3':'a'*64},
            'selection':{'file':'round-1-selection.rrbin','bytes':13,'blake3':'b'*64}}]})
if expected_numerator:
    metadata.update(numerator_symmetry_assistance=True,
        numerator_symmetry_preparation={'input_keys':4,'verified_generators':1,'proposed_generators':2,'equations':3})
if expected_sector:
    metadata.update(sector_symmetry_assistance=True, sector_symmetry_policy=expected_sector_policy,
        sector_symmetry_saved_projection=expected_sector_projection,
        sector_symmetry_preparation={'input_keys':4,'verified_generators':2,'proposed_generators':3,
            'processed_keys':4,'closure_keys':9,'source_jobs_complete':True,
            'analyzed_supports':2,'equation_targets':3,'equations':5,'output_terms':12,'complete':True},
        sector_symmetry_reduction={'processed_rows':5,'source_rows':5,
            'saved_reduced_rows':4 if expected_sector_projection else 0,
            'projected_terms':9 if expected_sector_projection else None,
            'source_terms':None if expected_sector_projection else 9,
            'incomplete_boundaries':1 if expected_sector_projection else 0,'complete':True,
            'saved_projection_complete':False if expected_sector_projection else None,'combined_closure_complete':False,
            'saved_projection_requested':expected_sector_projection,'exact_source_rows_ready':True,
            'deferred_symmetry_sources':1,'unresolved_output_keys':1},
        completed_assistance_rows=5,
        sector_symmetry_certificate={'recipe':'verified-sector-symmetry-sources-v3','file':'sector.bin',
            'policy':expected_sector_policy,'saved_projection_requested':expected_sector_projection},
        sector_symmetry_reduced_certificate={'recipe':('sector-symmetry-saved-projection-v1'
            if expected_sector_projection else 'sector-symmetry-source-rows-v1'),
            'file':'sector-reduced.bin','complete':True,'rows':5,
            'saved_projection_requested':expected_sector_projection})
if expected_fresh:
    metadata.update(fresh_search=True, fresh_search_recipe='relations-only-v1',
                    seed_depth=expected_seed_depth, containing_sector_depth=expected_containing_depth)
metadata.update(deep_verification=expected_deep, inventory={'complete': True, 'independently_verified': expected_deep,
    'installed': {'rules': 9982}, 'encountered': {'rules': 6698}})
assert ('--containing-sector-depth' in args) == bool(expected_containing_depth or expected_fresh)
if expected_containing_depth or expected_fresh:
    assert int(args[args.index('--containing-sector-depth') + 1]) == expected_containing_depth
assert ('--artifact' in args) == refine
assert ('--command' in args) != refine
directory = pathlib.Path(args[args.index('--directory') + 1])
events = pathlib.Path(args[args.index('--events') + 1])
stop = pathlib.Path(args[args.index('--stop-file') + 1])
assert all(__import__('os').environ[x] == '1' for x in ['RAYON_NUM_THREADS','OMP_NUM_THREADS'])
def event(status):
    with events.open('a') as out:
        out.write(json.dumps({'event':'master_reduction_progress','phase':'Master reduction',
            'stage':'elimination','status':status,'remaining_terminals':3,'artifact':str(directory),
            **{k:v for k,v in metadata.items() if k != 'inventory'},
            'checkpoint':{'state':'saved','generation':1,'directory':str(directory),'bytes':20}})+'\\n')
if '--resume' not in args:
    while not stop.exists(): time.sleep(.01)
    (directory/'latest.json').write_text(json.dumps(metadata))
    event('paused')
    sys.exit(4)
(directory/'artifact.json').write_text(json.dumps(metadata))
event('completed_nonminimal' if refine else 'published_unrefined')
''')
        binary.chmod(0o700)
        cpus = sorted(os.sched_getaffinity(0))[:1]
        policy = {"command_arguments": ["--executable", str(binary)]}
        plan = {**self.plan, "steering_policy": policy, "requested_workers": 1, "cpus": str(cpus[0]),
                "checkpoint_interval_seconds": 3600,
                "supervisor_ram_policy": {"max_memory_bytes": 2_000_000_000,
                    "ram_guard_margin_percent": 5, "host_memory_reserve_bytes": 1_000_000,
                    "swap_growth_stop_bytes_per_second": 0, "swap_growth_stop_seconds": 120}}
        driver = SimpleNamespace(**vars(PRODUCTION))
        native_policy = {**self.policy, "operation": operation, "saved_rule_assistance": saved_rule_assistance,
                         "fresh_search": fresh_search, "seed_depth": seed_depth,
                         "numerator_symmetry_assistance": numerator_symmetry_assistance,
                         "sector_symmetry_assistance": sector_symmetry_assistance,
                         "sector_symmetry_policy": sector_symmetry_policy,
                         "sector_symmetry_saved_projection": sector_symmetry_saved_projection,
                         "selective_symmetry_feedback": selective_symmetry_feedback,
                         "local_dimensional_recurrence_assistance": local_dimensional_recurrence_assistance,
                         "max_relation_columns": max_relation_columns,
                         "deep_verification": deep_verification,
                         "circuit_symmetry_assistance": circuit_symmetry_assistance,
                         "containing_sector_depth": containing_sector_depth,
                         "effective_containing_sector_depth": (containing_sector_depth if fresh_search else
                                                                max(containing_sector_depth, inherited_containing_depth))}
        if refinement_workers is not None:
            native_policy.update(refinement_workers=refinement_workers,refinement_cpus=refinement_cpus)
        if normalization_profile is not None:
            native_policy["normalization_profile"] = normalization_profile
            native_policy["effective_normalization_profile"] = normalization_profile + "-v1"
        if operation == "refine":
            native_policy["source_artifact"] = str(self.publish_fixture())
        preserved_source = None
        if preserve_refined:
            preserved_source = self.publish_fixture("refine", depth=2, deep_verification=False)
            original_source = (preserved_source / "artifact.json").read_bytes()
            native_policy["previous_artifact"] = str(preserved_source)
            native_policy["preserved_refinement"] = PHASES.completed_artifact(self.campaign)["refinement"]
        binding = self.binding()
        directory = self.campaign / "master-reduction/scopes" / binding["key"]
        directory.mkdir(parents=True)
        timer = threading.Timer(.4, lambda: os.kill(os.getpid(), signal.SIGINT))
        with patch.object(PHASES.sys, "stderr", io.StringIO()), patch.object(PHASES.sys, "stdout", io.StringIO()) as output:
            timer.start()
            try:
                self.assertEqual(PHASES.phase_two(plan, native_policy, self.campaign / "runs/first/request.json", binding, directory, driver), 4)
            finally:
                timer.cancel()
                timer.join()
            self.assertTrue((directory / "latest.json").exists())
            if operation == "publish":
                with self.assertRaisesRegex(ValueError, "cannot change deep verification on resume"):
                    PHASES.phase_two(plan, {**native_policy, "deep_verification": not deep_verification},
                                     self.campaign / "runs/first/request.json", binding, directory, driver)
            if operation == "refine":
                self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["status"], "published_unrefined")
                with self.assertRaisesRegex(ValueError, "cannot change finite feedback on resume"):
                    PHASES.phase_two(plan, {**native_policy, "finite_feedback": False},
                                     self.campaign / "runs/first/request.json", binding, directory, driver)
                if fresh_search:
                    with self.assertRaisesRegex(ValueError, "cannot change fresh-search mode on resume"):
                        PHASES.phase_two(plan, {**native_policy, "fresh_search": False},
                                         self.campaign / "runs/first/request.json", binding, directory, driver)
                if numerator_symmetry_assistance:
                    with self.assertRaisesRegex(ValueError, "cannot change numerator-symmetry assistance on resume"):
                        PHASES.phase_two(plan, {**native_policy, "numerator_symmetry_assistance": False},
                                         self.campaign / "runs/first/request.json", binding, directory, driver)
                if sector_symmetry_assistance:
                    with self.assertRaisesRegex(ValueError, "cannot change sector-symmetry assistance on resume"):
                        PHASES.phase_two(plan, {**native_policy, "sector_symmetry_assistance": False},
                                         self.campaign / "runs/first/request.json", binding, directory, driver)
                    with self.assertRaisesRegex(ValueError, "cannot change sector-symmetry policy on resume"):
                        PHASES.phase_two(plan, {**native_policy, "sector_symmetry_policy": (
                            "polynomial-orbits" if sector_symmetry_policy == "generators" else "generators")},
                            self.campaign / "runs/first/request.json", binding, directory, driver)
                    with self.assertRaisesRegex(ValueError, "cannot change sector-symmetry saved projection on resume"):
                        PHASES.phase_two(plan, {**native_policy, "sector_symmetry_saved_projection": not sector_symmetry_saved_projection},
                            self.campaign / "runs/first/request.json", binding, directory, driver)
                if selective_symmetry_feedback:
                    with self.assertRaisesRegex(ValueError, "cannot change selective-symmetry feedback on resume"):
                        PHASES.phase_two(plan, {**native_policy, "selective_symmetry_feedback": False},
                            self.campaign / "runs/first/request.json", binding, directory, driver)
                if local_dimensional_recurrence_assistance:
                    with self.assertRaisesRegex(ValueError, "cannot change local dimensional-recurrence assistance on resume"):
                        PHASES.phase_two(plan, {**native_policy, "local_dimensional_recurrence_assistance": False},
                            self.campaign / "runs/first/request.json", binding, directory, driver)
                if normalization_profile == "standard":
                    paused_pointer = (self.campaign / "artifacts/latest.json").read_bytes()
                    with self.assertRaisesRegex(ValueError, "cannot change normalization profile on resume"):
                        PHASES.phase_two(plan, {**native_policy, "effective_normalization_profile": "conservative-v1"},
                                         self.campaign / "runs/first/request.json", binding, directory, driver)
                    self.assertEqual((self.campaign / "artifacts/latest.json").read_bytes(), paused_pointer)
                    # Effective policy remains pinned when the requested option is omitted on resume.
                    native_policy.pop("normalization_profile")
            self.assertEqual(PHASES.phase_two(plan, native_policy, self.campaign / "runs/first/request.json", binding, directory, driver), 0)
            self.assertIn("Published executable rule records: 9,982", output.getvalue())
            self.assertIn("Encountered cover rule records: 6,698", output.getvalue())
        self.assertTrue((directory / "artifact.json").exists())
        pointer = PHASES.read_json(self.campaign / "master-reduction/active-phase.json")
        observer = PRODUCTION.SUPERVISOR.MONITOR
        self.assertEqual(observer.active_status_directory(self.campaign), Path(pointer["run_directory"]))
        status = observer.read_status(self.campaign)
        self.assertEqual(plan["requested_workers"],1)
        self.assertEqual(plan["cpus"],str(cpus[0]))
        self.assertEqual(status["workers"],refinement_workers or 1)
        expected = "published_unrefined" if operation == "publish" else "completed_nonminimal"
        self.assertEqual(status["state"], expected)
        self.assertEqual(status["master_reduction"]["remaining_terminals"], 3)
        self.assertEqual(status["master_reduction"]["published_rule_records"], 9982)
        self.assertEqual(status["master_reduction"]["observed_cover_rules"], 6698)
        self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["status"],
                         "completed_nonminimal" if preserve_refined else expected)
        if preserve_refined:
            published = PHASES.completed_artifact(self.campaign)
            self.assertEqual(published["operation"], "refine")
            self.assertEqual(published["resolved_directory"], directory)
            self.assertEqual(published["refinement"]["seed_depth"], 2)
            self.assertTrue(published["independently_verified"])
            self.assertEqual((preserved_source / "artifact.json").read_bytes(), original_source)
        if operation == "refine":
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["normalization_profile"],
                             (normalization_profile or "conservative") + "-v1")
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"]["circuit_symmetry_assistance"],
                             circuit_symmetry_assistance)
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"]["saved_rule_assistance"],
                             saved_rule_assistance)
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"].get("numerator_symmetry_assistance", False),
                             numerator_symmetry_assistance)
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"].get("sector_symmetry_assistance", False),
                             sector_symmetry_assistance)
            if sector_symmetry_assistance:
                self.assertIs(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"]["sector_symmetry_saved_projection"],
                              sector_symmetry_saved_projection)
                self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"]["sector_symmetry_policy"],
                                 sector_symmetry_policy)
                self.assertEqual(status["master_reduction"]["sector_symmetry_preparation"]["equations"], 5)
                terms_field = "projected_terms" if sector_symmetry_saved_projection else "source_terms"
                self.assertEqual(status["master_reduction"]["sector_symmetry_reduction"][terms_field], 9)
            if selective_symmetry_feedback:
                published = PHASES.completed_artifact(self.campaign)
                self.assertTrue(published["selective_symmetry_feedback_enabled"])
                self.assertEqual(published["selective_symmetry_feedback"]["rounds"][0]["parent_native_state"]["file"],
                                 "round-1-parent.rrbin")
                feedback = status["master_reduction"]["selective_symmetry_feedback"]
                self.assertEqual(feedback["pending_targets"], 2)
                self.assertNotIn("rounds", feedback)
            if local_dimensional_recurrence_assistance:
                published = PHASES.completed_artifact(self.campaign)
                self.assertTrue(published["local_dimensional_recurrence_assistance"])
                self.assertEqual(published["local_dimensional_recurrence_certificate"]["file"], "local-dimensional.rrbin")
                preparation = status["master_reduction"]["local_dimensional_recurrence_preparation"]
                self.assertEqual(preparation["weighted_source_rows"], 7)
                self.assertNotIn("native_witnesses", preparation)
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"]["containing_sector_depth"],
                             containing_sector_depth if fresh_search else max(containing_sector_depth, inherited_containing_depth))
            if fresh_search:
                self.assertEqual(status["master_reduction"]["seed_depth"], seed_depth)
                self.assertEqual(status["master_reduction"]["containing_sector_depth"], containing_sector_depth)
                self.assertTrue(status["master_reduction"]["fresh_search"])
                pointer = PHASES.read_json(self.campaign / "artifacts/latest.json")
                self.assertTrue(pointer["refinement"]["fresh_search"])


class MasterDashboardTests(unittest.TestCase):
    def test_publication_rule_inventory_survives_new_and_legacy_reports(self):
        for counts in (
                {"published_rule_records": 9982, "observed_cover_rules": 6698},
                {"inventory": {"installed": {"rules": 9982}, "encountered": {"rules": 6698}}}):
            # JSON roundtrip reflects the actual serialized report/event shape.
            native = json.loads(json.dumps({"operation": "publish", "stage": "published",
                                           "artifact": "/artifact", **counts}))
            frame = TELEMETRY.normalize_status({"state": "published_unrefined", "master_reduction": native})
            self.assertEqual(frame["master_reduction"]["published_rule_records"], 9982)
            self.assertEqual(frame["master_reduction"]["observed_cover_rules"], 6698)
            shown = DASHBOARD.render_table(frame, 150, 32, False)
            self.assertTrue(any("Published rules" in line and "9,982" in line for line in shown))
            self.assertTrue(any("Observed cover" in line and "6,698" in line for line in shown))
            self.assertIn("installed executable rule records", "\n".join(shown))
            self.assertIn("Published executable rule records 9,982", "\n".join(DASHBOARD.plain_summary(frame)))
        for value in (None, -1, True, "9982", 1.5):
            frame = TELEMETRY.normalize_status({"master_reduction": {
                "operation": "publish", "published_rule_records": value}})
            self.assertIsNone(frame["master_reduction"]["published_rule_records"])
            self.assertIsNone(frame["master_reduction"]["observed_cover_rules"])
            shown = DASHBOARD.render_table(frame, 150, 32, False)
            self.assertTrue(any("Available rules" in line and "unknown" in line for line in shown))
        frame = TELEMETRY.normalize_status({"master_reduction": {
            "operation": "publish", "published_rule_records": 0, "observed_cover_rules": 0}})
        self.assertEqual(frame["master_reduction"]["published_rule_records"], 0)

    def test_feedback_native_counts_survive_telemetry_and_table(self):
        frame = TELEMETRY.normalize_status({"state": "running", "master_reduction": {
            "stage": "terminal_collection", "collection": {
                "finite_feedback_stage": "completed", "finite_feedback_rows": 17,
                "finite_feedback_columns": 12, "finite_feedback_auxiliary_columns": 8,
                "finite_feedback_aliases": 3, "finite_feedback_equations": 2}}})
        collection = frame["master_reduction"]["collection"]
        self.assertEqual(collection["finite_feedback_equations"], 2)
        self.assertEqual(collection["finite_feedback_auxiliary_columns"], 8)
        shown = "\n".join(DASHBOARD.render_table(frame, 150, 40, False))
        self.assertIn("Finite feedback", shown)
        self.assertIn("17 retained source rows", shown)
        self.assertIn("8 auxiliary; 3 full-U aliases", shown)

    def frame(self):
        return TELEMETRY.normalize_status({
            "state": "running", "elapsed_seconds": 150, "workers": 32,
            "hard_memory_bytes": 600_000_000_000, "soft_memory_bytes": 570_000_000_000,
            "resources": {"native_busy_cores": 25.3, "aggregate_rss_bytes": 1_230_000_000},
            "run_directory": "/campaign/master-reduction/scopes/abc/runs/test",
            "checkpoint": {"state": "saved", "generation": 2, "bytes": 12_345,
                           "directory": "/campaign/master-reduction/scopes/abc"},
            "master_reduction": {"stage": "elimination", "raw_terminals": 116,
                "normalized_terminals": 74, "remaining_terminals": 49,
                "eliminated_terminals": 25, "relation_rows": 760,
                "completed_work": 70, "total_work": 100, "seed_depth": 1,
                "scope_binding": "0123456789abcdef"}})

    def test_colored_tables_fit_small_and_large_terminal_and_keep_scientific_scope(self):
        frame = self.frame()
        for width, height in ((80, 24), (150, 40), (35, 12)):
            rows = DASHBOARD.render_table(frame, width, height, True)
            self.assertLessEqual(len(rows), height)
            self.assertTrue(all(DASHBOARD.cell_width(ANSI.sub("", row)) == width for row in rows))
            self.assertIn("\x1b[", "\n".join(rows))
        shown = "\n".join(DASHBOARD.render_table(frame, 80, 24, False))
        self.assertIn("MASTER REFINEMENT", shown)
        self.assertIn("nonminimal", shown)
        self.assertNotIn("ROOT CLOSURE", shown)
        self.assertNotIn("pending", shown)

    def test_non_tty_is_structured_json_not_ansi_and_unknown_counts_are_not_zero(self):
        frame = self.frame()
        frame["master_reduction"]["remaining_terminals"] = None
        stream = io.StringIO()
        DASHBOARD.Presenter(stream=stream).render_frame(frame, now=0, force=True)
        value = json.loads(stream.getvalue())
        self.assertEqual(value["telemetry"]["phase"], "Master refinement")
        self.assertIsNone(value["telemetry"]["master_reduction"]["remaining_terminals"])
        self.assertNotIn("\x1b", stream.getvalue())
        self.assertFalse(value["telemetry"]["master_reduction"]["master_minimality_claim"])

    def test_publication_has_its_own_coverage_display_and_no_relation_search(self):
        frame = self.frame()
        frame["master_reduction"]["operation"] = "publish"
        frame["state"] = "published_unrefined"
        frame["master_reduction"].update(publication_verification_mode="inventory", publication_assurance="trusted_saved_scope")
        for width, height in ((80, 24), (150, 32), (35, 12)):
            rows = DASHBOARD.render_table(frame, width, height, True)
            self.assertLessEqual(len(rows), height)
            self.assertTrue(all(DASHBOARD.cell_width(ANSI.sub("", row)) == width for row in rows))
        shown = "\n".join(DASHBOARD.render_table(frame, 80, 24, False))
        self.assertIn("ARTIFACT PUBLICATION", shown)
        self.assertIn("trusted saved", shown)
        self.assertNotIn("deep replay passed", shown)
        self.assertIn("not requested", shown)
        self.assertNotIn("ROOT CLOSURE", shown)
        self.assertNotIn("Relation work", shown)

    def test_partial_publication_frames_keep_unfinished_refinement_visible(self):
        for native in (
                {"operation": "finalize-refinement", "stage": "rebinding retained refinement"},
                {"operation": "publish", "stage": "collection", "finite_search_complete": False,
                 "partial_refinement_source": {"status": "failed"}}):
            frame = TELEMETRY.normalize_status({"state": "running", "master_reduction": native})
            self.assertEqual(frame["phase"], "Partial refinement publication")
            self.assertTrue(frame["master_reduction"]["partial_refinement"])
            for width, height in ((80, 24), (150, 32), (35, 12)):
                rows = DASHBOARD.render_table(frame, width, height, False)
                self.assertLessEqual(len(rows), height)
                self.assertTrue(all(DASHBOARD.cell_width(row) == width for row in rows))
            shown = "\n".join(DASHBOARD.render_table(frame, 150, 32, False)
                              + DASHBOARD.plain_summary(frame))
            self.assertIn("partial retained", shown)
            self.assertIn("finite search unfinished", shown)
            self.assertNotIn("not requested", shown)
            self.assertNotIn("bounded", shown)
            self.assertNotIn("Relation work", shown)
        # Starting another explicit search from a partial artifact is still a
        # refinement run, not a partial-publication operation.
        frame = TELEMETRY.normalize_status({"master_reduction": {
            "operation": "refine", "finite_search_complete": False,
            "partial_refinement_source": {"status": "failed"}}})
        self.assertFalse(frame["master_reduction"]["partial_refinement"])
        self.assertEqual(frame["phase"], "Master refinement")

    def test_deep_publication_only_reports_success_from_native_assurance(self):
        frame = TELEMETRY.normalize_status({"state": "published_unrefined", "master_reduction": {
            "operation": "publish", "publication_verification_mode": "independent_replay",
            "independently_verified": True, "publication_assurance": "independently_verified",
            "verification_progress": {"event":"verify_progress", "reinspected":7,"selected":9}}})
        shown = "\n".join(DASHBOARD.render_table(frame, 150, 40, False))
        self.assertIn("deep replay passed", shown)
        self.assertIn("Replay progress", shown)
        self.assertIn("7 / 9", shown)

    def test_inventory_progress_keeps_native_stages_without_claiming_deep_verification(self):
        for event in ("verify_loaded", "verify_prepared", "inventory_checked", "inventory_census_progress"):
            frame = TELEMETRY.normalize_status({"state":"running", "master_reduction": {
                "operation":"publish", "stage":event, "publication_verification_mode":"inventory",
                "verification_progress":{"event":event,"completed":3,"total":8}}})
            shown = "\n".join(DASHBOARD.render_table(frame, 150, 40, False))
            self.assertEqual(frame["master_reduction"]["stage"], event)
            self.assertIn("3 / 8", shown)
            self.assertNotIn("deep replay passed", shown)


if __name__ == "__main__":
    unittest.main()
