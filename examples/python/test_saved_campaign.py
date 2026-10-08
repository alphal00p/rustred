"""Public saved-campaign commands: scratch metadata only, no native solves."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location("saved_campaign", Path(__file__).with_name("saved_campaign.py"))
WRAPPER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(WRAPPER)
ROOT = Path(__file__).resolve().parents[2]


class SavedCampaignTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "TMP").mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="saved-campaign-test-", dir=ROOT / "TMP")
        self.addCleanup(self.temporary.cleanup)
        self.campaign = Path(self.temporary.name)
        self.binary = self.campaign / "rustred"
        self.binary.write_text("#!/bin/sh\nexit 99\n")
        self.binary.chmod(0o700)

    def invoke(self, action, *options):
        with patch.object(WRAPPER.os, "execv") as launch:
            WRAPPER.main([action, "--campaign", str(self.campaign), "--executable", str(self.binary), *options])
        return launch.call_args.args[1] if launch.called else None

    def test_run_starts_real_solve_and_publish_without_refinement(self):
        command = self.invoke("run", "--workers", "4")
        self.assertIn("--start", command)
        self.assertIn("--workers", command)
        self.assertNotIn("--refine-masters", command)
        self.assertNotIn("--resume", command)
        self.assertIn("--master-reduction-executable", command)

    def test_run_autoresumes_but_does_not_refine(self):
        path = self.campaign / "checkpoints/main/latest.json"
        path.parent.mkdir(parents=True)
        path.write_text("{}")
        command = self.invoke("run")
        self.assertIn("--resume", command)
        self.assertNotIn("--refine-masters", command)

    def test_refine_is_only_explicit_postprocess_and_seed_depth_is_forwarded(self):
        command = self.invoke("refine", "--seed-depth", "2")
        self.assertIn("--refine-masters", command)
        self.assertIn("--resume", command)
        self.assertEqual(command[command.index("--master-seed-depth") + 1], "2")
        self.assertNotIn("--executable", command)  # Never replace the phase-one engine.
        self.assertNotIn("--master-saved-rule-assistance", command)
        self.assertNotIn("--no-master-saved-rule-assistance", command)
        self.assertNotIn("--master-containing-sector-depth", command)
        self.assertNotIn("--master-circuit-symmetry-assistance", command)
        self.assertNotIn("--no-master-circuit-symmetry-assistance", command)
        self.assertNotIn("--master-normalization-profile", command)

    def test_collection_peers_are_repeatable_and_only_refine(self):
        first, second = self.campaign / "first", self.campaign / "second"
        command = self.invoke("refine", "--collection-artifact", str(first), "--collection-artifact", str(second))
        self.assertEqual([command[i + 1] for i, value in enumerate(command) if value == "--master-collection-artifact"], [str(first), str(second)])
        with patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
            self.invoke("publish", "--collection-artifact", str(first))

    def test_normalization_profile_is_optional_explicit_refinement_only(self):
        for profile in ("conservative", "standard"):
            command = self.invoke("refine", "--normalization-profile", profile)
            self.assertEqual(command[command.index("--master-normalization-profile") + 1], profile)
            self.assertIn("--refine-masters", command)
        for action, profile in (("publish", "standard"), ("inspect", "standard"), ("refine", "unlimited")):
            with self.subTest(action=action, profile=profile), patch.object(WRAPPER.sys, "stderr", io.StringIO()):
                with self.assertRaises(SystemExit):
                    self.invoke(action, "--normalization-profile", profile)

    def test_containing_sector_depth_is_refine_only_and_forwarded(self):
        for depth in (0, 1, 2):
            command = self.invoke("refine", "--containing-sector-depth", str(depth))
            self.assertEqual(command[command.index("--master-containing-sector-depth") + 1], str(depth))
            self.assertIn("--refine-masters", command)
        for action, options in (("refine", ["--containing-sector-depth", "-1"]),
                                ("publish", ["--containing-sector-depth", "1"]),
                                ("inspect", ["--containing-sector-depth", "1"])):
            with self.subTest(action=action), patch.object(WRAPPER.sys, "stderr", io.StringIO()):
                with self.assertRaises(SystemExit):
                    self.invoke(action, *options)

    def test_refine_saved_rule_assistance_can_be_enabled_or_explicitly_disabled(self):
        for supplied, forwarded in (("--saved-rule-assistance", "--master-saved-rule-assistance"),
                                    ("--no-saved-rule-assistance", "--no-master-saved-rule-assistance")):
            with self.subTest(supplied=supplied):
                command = self.invoke("refine", supplied)
                self.assertIn(forwarded, command)
                self.assertIn("--refine-masters", command)
                self.assertIn("--start", command)

    def test_circuit_symmetry_assistance_is_independent_and_refine_only(self):
        for supplied, forwarded in (("--circuit-symmetry-assistance", "--master-circuit-symmetry-assistance"),
                                    ("--no-circuit-symmetry-assistance", "--no-master-circuit-symmetry-assistance")):
            command = self.invoke("refine", supplied, "--saved-rule-assistance")
            self.assertIn(forwarded, command)
            self.assertIn("--master-saved-rule-assistance", command)
            self.assertIn("--refine-masters", command)
        for action in ("publish", "inspect"):
            with self.subTest(action=action), patch.object(WRAPPER.sys, "stderr", io.StringIO()):
                with self.assertRaises(SystemExit):
                    self.invoke(action, "--circuit-symmetry-assistance")

    def test_finite_feedback_mode_is_forwarded_only_by_explicit_refinement(self):
        self.assertNotIn("--master-finite-feedback", self.invoke("refine"))
        for supplied, forwarded in (("--finite-feedback", "--master-finite-feedback"),
                                    ("--no-finite-feedback", "--no-master-finite-feedback")):
            self.assertIn(forwarded, self.invoke("refine", supplied))
            for action in ("publish", "inspect"):
                with patch.object(WRAPPER.sys, "stderr", io.StringIO()), self.assertRaises(SystemExit):
                    self.invoke(action, supplied)

    def test_publish_is_only_postprocess_and_inspect_calls_native(self):
        self.assertIn("--publish-only", self.invoke("publish"))
        command = self.invoke("inspect", "--format", "json")
        self.assertEqual(command, [str(self.binary), "artifact-inspect", "--campaign-directory", str(self.campaign), "--format", "json"])

    def test_extend_dry_run_prints_wrapper_not_unprepared_resume(self):
        helper = SimpleNamespace(PRESERVE_DIFFERENCE=object(), extend=unittest.mock.Mock(return_value={
            "rank": 1, "max_power_difference": 10, "new_required_queries": 116}))
        output = io.StringIO()
        with patch.object(WRAPPER, "sibling", return_value=helper), patch.object(WRAPPER.sys, "stdout", output):
            self.assertIsNone(self.invoke("extend", "--rank", "1", "--max-power-difference", "10", "--dry-run"))
        self.assertTrue(helper.extend.call_args.kwargs["dry_run"])
        command = output.getvalue().split("Command: ", 1)[1]
        self.assertIn("saved_campaign.py extend", command)
        self.assertNotIn("--dry-run", command)
        self.assertNotIn("production_saved_owner_campaign.py", command)

    def test_extend_publishes_without_implicitly_refining(self):
        helper = SimpleNamespace(PRESERVE_DIFFERENCE=object(), extend=unittest.mock.Mock(return_value={
            "rank": 1, "max_power_difference": 9, "new_required_queries": 67}))
        with patch.object(WRAPPER, "sibling", return_value=helper), patch.object(WRAPPER.sys, "stdout", io.StringIO()):
            command = self.invoke("extend", "--rank", "1")
        self.assertFalse(helper.extend.call_args.kwargs["dry_run"])
        self.assertIn("--resume", command)
        self.assertIn("--start", command)
        self.assertNotIn("--refine-masters", command)

    def test_paused_phase_reuses_frozen_binary_despite_new_local_build(self):
        phases = WRAPPER.sibling("campaign_phases")
        root = self.campaign / "master-reduction"
        root.mkdir()
        native = root / "bin/rustred-frozen"
        native.parent.mkdir()
        native.write_text("#!/bin/sh\nexit 0\n")
        native.chmod(0o700)
        phase = root / "scopes/paused"
        phase.mkdir(parents=True)
        (phase / "latest.json").write_text("{}")
        (root / "policy.json").write_text(json.dumps({
            "schema": phases.SCHEMA, "enabled": True, "seed_depth": 0,
            "executable": {"path": "bin/rustred-frozen", "sha256": "previous"}}))
        (root / "active-phase.json").write_text(json.dumps({"operation": "refine", "directory": str(phase)}))
        self.assertEqual(WRAPPER.native_executable(self.campaign), native)
        self.assertEqual(WRAPPER.native_executable(self.campaign, self.binary), self.binary)

    def test_completed_phase_does_not_pin_an_obsolete_refinement_binary(self):
        self.test_paused_phase_reuses_frozen_binary_despite_new_local_build()
        latest = self.campaign / "master-reduction/scopes/paused/latest.json"
        latest.write_text(json.dumps({"status": "completed_nonminimal"}))
        # Simulate an existing repository build without changing real binaries.
        original_is_file = Path.is_file
        current = ROOT / "target/release/rustred"
        with patch.object(Path, "is_file", lambda path: True if path == current else original_is_file(path)), \
                patch.object(WRAPPER.os, "access", return_value=True):
            self.assertEqual(WRAPPER.native_executable(self.campaign), current)


if __name__ == "__main__":
    unittest.main()
