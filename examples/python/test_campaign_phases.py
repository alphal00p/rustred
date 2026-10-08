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
                        circuit_symmetry_assistance=False, normalization_profile=None, finite_feedback=True):
        path = self.campaign / "master-reduction/scopes/published"
        self.write("master-reduction/scopes/published/artifact.json", {"status": "published_unrefined",
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

    def test_default_resume_preserves_same_scope_refined_artifact_without_rerun(self):
        self.publish_fixture("refine", 1)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two") as second:
            self.assertEqual(PHASES.run(self.plan, self.policy, True, self.driver), 0)
        first.assert_not_called()
        second.assert_not_called()
        self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["operation"], "refine")

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

    def test_explicit_refinement_saves_and_resumes_without_replacing_publication_on_pause(self):
        self._scratch_native_pause_resume("refine")

    def test_assisted_refinement_forwards_mode_and_resumes_its_checkpoint(self):
        self._scratch_native_pause_resume("refine", saved_rule_assistance=True)

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

    def _scratch_native_pause_resume(self, operation, saved_rule_assistance=False,
                                    containing_sector_depth=0, inherited_containing_depth=0,
                                    circuit_symmetry_assistance=False, normalization_profile=None):
        # Fake native protocol only; this proves process/checkpoint orchestration,
        # not mathematical correctness or a successful native Rust solve.
        binary = self.campaign / "fake-native"
        binary.write_text(f"#!{sys.executable}\nexpected_assistance = {saved_rule_assistance and operation == 'refine'}\n"
                          f"expected_circuit = {circuit_symmetry_assistance and operation == 'refine'}\n"
                          f"expected_profile = {repr(normalization_profile if operation == 'refine' else None)}\n"
                          f"expected_containing_depth = {containing_sector_depth if operation == 'refine' else 0}\n" + '''import json, pathlib, sys, time
args = sys.argv
refine = args[1] == 'walk-master-reduce'
assert ('--seed-depth' in args) == refine
assert ('--saved-rule-assistance' in args) == expected_assistance
assert ('--circuit-symmetry-assistance' in args) == expected_circuit
assert ('--finite-feedback' in args) == refine
assert ('--normalization-profile' in args) == (expected_profile is not None)
if expected_profile is not None:
    assert args[args.index('--normalization-profile') + 1] == expected_profile
profile = (expected_profile or 'conservative') + '-v1'
metadata = {'normalization_profile': profile, 'normalization_limits': {'fixture': True}}
metadata.update(finite_feedback=refine, finite_feedback_recipe='finite-row-feedback-v1')
assert ('--containing-sector-depth' in args) == bool(expected_containing_depth)
if expected_containing_depth:
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
            'stage':'elimination','status':status,'remaining_terminals':3,'artifact':str(directory), **metadata,
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
                         "circuit_symmetry_assistance": circuit_symmetry_assistance,
                         "containing_sector_depth": containing_sector_depth,
                         "effective_containing_sector_depth": max(containing_sector_depth, inherited_containing_depth)}
        if normalization_profile is not None:
            native_policy["normalization_profile"] = normalization_profile
            native_policy["effective_normalization_profile"] = normalization_profile + "-v1"
        if operation == "refine":
            native_policy["source_artifact"] = str(self.publish_fixture())
        binding = self.binding()
        directory = self.campaign / "master-reduction/scopes" / binding["key"]
        directory.mkdir(parents=True)
        timer = threading.Timer(.4, lambda: os.kill(os.getpid(), signal.SIGINT))
        with patch.object(PHASES.sys, "stderr", io.StringIO()), patch.object(PHASES.sys, "stdout", io.StringIO()):
            timer.start()
            try:
                self.assertEqual(PHASES.phase_two(plan, native_policy, self.campaign / "runs/first/request.json", binding, directory, driver), 4)
            finally:
                timer.cancel()
                timer.join()
            self.assertTrue((directory / "latest.json").exists())
            if operation == "refine":
                self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["status"], "published_unrefined")
                with self.assertRaisesRegex(ValueError, "cannot change finite feedback on resume"):
                    PHASES.phase_two(plan, {**native_policy, "finite_feedback": False},
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
        self.assertTrue((directory / "artifact.json").exists())
        pointer = PHASES.read_json(self.campaign / "master-reduction/active-phase.json")
        observer = PRODUCTION.SUPERVISOR.MONITOR
        self.assertEqual(observer.active_status_directory(self.campaign), Path(pointer["run_directory"]))
        status = observer.read_status(self.campaign)
        expected = "published_unrefined" if operation == "publish" else "completed_nonminimal"
        self.assertEqual(status["state"], expected)
        self.assertEqual(status["master_reduction"]["remaining_terminals"], 3)
        self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["status"], expected)
        if operation == "refine":
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["normalization_profile"],
                             (normalization_profile or "conservative") + "-v1")
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"]["circuit_symmetry_assistance"],
                             circuit_symmetry_assistance)
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"]["saved_rule_assistance"],
                             saved_rule_assistance)
            self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["refinement"]["containing_sector_depth"],
                             max(containing_sector_depth, inherited_containing_depth))


class MasterDashboardTests(unittest.TestCase):
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
        for width, height in ((80, 24), (150, 32), (35, 12)):
            rows = DASHBOARD.render_table(frame, width, height, True)
            self.assertLessEqual(len(rows), height)
            self.assertTrue(all(DASHBOARD.cell_width(ANSI.sub("", row)) == width for row in rows))
        shown = "\n".join(DASHBOARD.render_table(frame, 80, 24, False))
        self.assertIn("ARTIFACT PUBLICATION", shown)
        self.assertIn("verified", shown)
        self.assertIn("not requested", shown)
        self.assertNotIn("ROOT CLOSURE", shown)
        self.assertNotIn("Relation work", shown)


if __name__ == "__main__":
    unittest.main()
