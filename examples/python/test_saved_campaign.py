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


if __name__ == "__main__":
    unittest.main()
