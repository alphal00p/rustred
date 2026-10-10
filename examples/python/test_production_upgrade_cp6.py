"""CP6 executable replacement metadata and existing launch gates; fake binaries only."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import test_production_upgrade as fixtures

PRODUCTION = fixtures.PRODUCTION
EPOCH = {"format": "RUSTRED-WALK-CP6", "schema": 3, "walk_semantics_version": 4, "resumable": True}
PROBE = dict(fixtures.PROBE, epoch_checkpoint=EPOCH)
NORMALIZED = {"checkpoint_format": EPOCH["format"], "checkpoint_schema": EPOCH["schema"],
              "walk_semantics_version": EPOCH["walk_semantics_version"]}


def envelope(**changes):
    return {"manifest": {**EPOCH, "generation": 7, "arity": 1, "files": [], **changes}, "blake3": [0] * 32}


class EpochExecutableUpgradeTests(unittest.TestCase):
    prepare = fixtures.ExecutableUpgradeTests.prepare
    frozen = fixtures.ExecutableUpgradeTests.frozen
    steering = fixtures.ExecutableUpgradeTests.steering
    tearDown = fixtures.ExecutableUpgradeTests.tearDown

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.old = fixtures.fake_executable(self.root / "old-rustred", probe=PROBE)
        self.campaign = self.prepare(self.root / "campaign", self.old, publication_policy="epoch",
                                     checkpoint_manifest=envelope())
        self.checkpoint = self.campaign / "checkpoints/main"
        self.new = fixtures.fake_executable(self.root / "new-rustred", probe=PROBE)

    # The CP5 launch guards must apply unchanged to the Epoch lane as well.
    test_live_run_or_held_checkpoint_lock_is_refused_before_mutation = (
        fixtures.ExecutableUpgradeTests.test_live_run_or_held_checkpoint_lock_is_refused_before_mutation)
    test_start_validates_every_frozen_and_ram_option_before_upgrading = (
        fixtures.ExecutableUpgradeTests.test_start_validates_every_frozen_and_ram_option_before_upgrading)
    test_interrupted_upgrade_is_refused_by_plain_resume_and_completed_by_rerun = (
        fixtures.ExecutableUpgradeTests.test_interrupted_upgrade_is_refused_by_plain_resume_and_completed_by_rerun)

    def test_dry_run_selects_epoch_probe_and_changes_nothing(self):
        before = fixtures.snapshot(self.campaign)
        status, output, errors, launch = fixtures.run(
            self.campaign, "--resume", "--upgrade-executable", str(self.new), "--json")
        self.assertEqual(status, 0, errors)
        launch.assert_not_called()
        self.assertEqual(fixtures.snapshot(self.campaign), before)
        plan = json.loads(output)
        self.assertTrue(plan["epoch_checkpoint"]["executable_upgrade_supported"])
        upgrade = plan["executable_upgrade"]
        self.assertEqual(upgrade["new"]["probe"], NORMALIZED)
        self.assertEqual(upgrade["checkpoint"]["format"], EPOCH["format"])
        self.assertEqual(upgrade["checkpoint"]["walk_semantics_version"], 4)
        self.assertNotIn("executable_blake3", upgrade["checkpoint"])
        self.assertEqual(plan["command"][-2:], ["--resume", str(self.checkpoint)])

    def test_upgrade_and_history_rollback_keep_checkpoint_and_request_unchanged(self):
        checkpoint_before = fixtures.snapshot(self.checkpoint)
        _, policy_before = self.steering()
        status, _, errors, launch = fixtures.run(
            self.campaign, "--resume", "--upgrade-executable", str(self.new), "--start")
        self.assertIsNone(status, errors)
        launch.assert_called_once()
        self.assertEqual(fixtures.snapshot(self.checkpoint), checkpoint_before)
        _, policy = self.steering()
        policy.pop("executable_upgrades")
        policy["command_arguments"][policy["command_arguments"].index("--executable") + 1] = str(self.frozen(self.old))
        self.assertEqual(policy, policy_before)
        receipt = json.loads((self.campaign / "bin/executable.json").read_text())
        for row in [receipt, *receipt["history"]]:
            self.assertEqual(row["checkpoint_format"], EPOCH["format"])
            self.assertEqual(row["checkpoint_schema"], 3)
            self.assertEqual(row["walk_semantics_version"], 4)
        with patch.object(PRODUCTION, "probe_walk_semantics", side_effect=AssertionError("history rollback needs no probe")):
            status, _, errors, launch = fixtures.run(
                self.campaign, "--resume", "--upgrade-executable", str(self.frozen(self.old)), "--start")
        self.assertIsNone(status, errors)
        launch.assert_called_once()
        self.assertEqual(fixtures.snapshot(self.checkpoint), checkpoint_before)
        receipt = json.loads((self.campaign / "bin/executable.json").read_text())
        self.assertEqual(receipt["sha256"], PRODUCTION.digest(self.old))
        self.assertEqual(receipt["history"][-1]["reason"], "rollback_executable")

    def test_refuses_wrong_or_missing_epoch_identity_without_mutation(self):
        cases = [
            (dict(PROBE, epoch_checkpoint=dict(EPOCH, walk_semantics_version=5)), "walk semantics version differs"),
            (dict(PROBE, epoch_checkpoint=dict(EPOCH, format="RUSTRED-WALK-CP5")), "but the checkpoint is RUSTRED-WALK-CP6"),
            (dict(PROBE, epoch_checkpoint=dict(EPOCH, schema=2)), "but the checkpoint is RUSTRED-WALK-CP6"),
            (dict(PROBE, epoch_checkpoint=dict(EPOCH, schema=True)), "must print one JSON object"),
            (dict(PROBE, epoch_checkpoint=dict(EPOCH, walk_semantics_version=True)), "must print one JSON object"),
            (dict(PROBE, epoch_checkpoint=dict(EPOCH, resumable=False)), "no resumable epoch_checkpoint"),
            (fixtures.PROBE, "no resumable epoch_checkpoint"),
            (dict(fixtures.PROBE, checkpoint_format=EPOCH["format"], checkpoint_schema=3,
                  walk_semantics_version=4), "no resumable epoch_checkpoint"),
        ]
        for index, (probe, fragment) in enumerate(cases):
            executable = fixtures.fake_executable(self.root / f"wrong-{index}", probe=probe)
            for start in ((), ("--start",)):
                with self.subTest(index=index, start=start):
                    before = fixtures.snapshot(self.campaign)
                    status, _, errors, launch = fixtures.run(
                        self.campaign, "--resume", "--upgrade-executable", str(executable), *start)
                    self.assertEqual(status, 2, errors)
                    self.assertIn(fragment, errors)
                    launch.assert_not_called()
                    self.assertEqual(fixtures.snapshot(self.campaign), before)

    def test_malformed_checkpoint_metadata_is_refused_without_mutation(self):
        cases = [
            {"manifest": []}, {"manifest": envelope()["manifest"]},
            dict(envelope(), blake3=[False] * 32), dict(envelope(), unexpected=True),
            envelope(schema=2), envelope(schema=3.0), envelope(resumable=False),
            envelope(generation=0), envelope(generation=True), envelope(walk_semantics_version=True),
            envelope()["manifest"],
        ]
        for document in cases:
            with self.subTest(document=document):
                (self.checkpoint / "latest.json").write_text(json.dumps(document))
                before = fixtures.snapshot(self.campaign)
                status, _, _, launch = fixtures.run(
                    self.campaign, "--resume", "--upgrade-executable", str(self.new), "--start")
                self.assertEqual(status, 2)
                launch.assert_not_called()
                self.assertEqual(fixtures.snapshot(self.campaign), before)

    def test_cp5_history_does_not_vouch_for_epoch_with_the_same_version(self):
        legacy = fixtures.fake_executable(self.root / "no-probe", body=fixtures.MISSING_PROBE)
        path = self.campaign / "bin/executable.json"
        for identity in ({}, {"checkpoint_format": "RUSTRED-WALK-CP5", "checkpoint_schema": 5}):
            receipt = json.loads(path.read_text())
            receipt["history"] = [{"sha256": PRODUCTION.digest(legacy), "walk_semantics_version": 4, **identity}]
            PRODUCTION.write_json(path, receipt)
            before = fixtures.snapshot(self.campaign)
            status, _, errors, launch = fixtures.run(
                self.campaign, "--resume", "--upgrade-executable", str(legacy), "--start")
            self.assertEqual(status, 2)
            self.assertIn("has no usable walk-semantics-version probe", errors)
            launch.assert_not_called()
            self.assertEqual(fixtures.snapshot(self.campaign), before)

    def test_format_change_under_lock_is_refused_even_with_same_semantics_version(self):
        original = PRODUCTION.plan_executable_upgrade
        changed = []

        def planned(*arguments):
            result = original(*arguments)
            (self.checkpoint / "latest.json").write_text(json.dumps(fixtures.manifest(walk_semantics_version=4)))
            changed.append(fixtures.snapshot(self.campaign))
            return result

        with patch.object(PRODUCTION, "plan_executable_upgrade", side_effect=planned):
            status, _, errors, launch = fixtures.run(
                self.campaign, "--resume", "--upgrade-executable", str(self.new), "--start")
        self.assertEqual(status, 2)
        self.assertIn("checkpoint format/schema changed during the upgrade", errors)
        launch.assert_not_called()
        self.assertEqual(fixtures.snapshot(self.campaign), changed[0])

    def test_frozen_copy_epoch_probe_is_rechecked(self):
        original = PRODUCTION.probe_walk_semantics
        calls = []

        def probe(executable, checkpoint_format):
            result = original(executable, checkpoint_format)
            calls.append(checkpoint_format)
            return result if len(calls) == 1 else dict(result, walk_semantics_version=5)

        before = fixtures.snapshot(self.campaign)
        with patch.object(PRODUCTION, "probe_walk_semantics", side_effect=probe):
            status, _, errors, launch = fixtures.run(
                self.campaign, "--resume", "--upgrade-executable", str(self.new), "--start")
        self.assertEqual(status, 2)
        self.assertIn("frozen copy of the new executable reports a different", errors)
        launch.assert_not_called()
        self.assertEqual(calls, [EPOCH["format"], EPOCH["format"]])
        self.assertEqual(fixtures.snapshot(self.campaign), before)


if __name__ == "__main__":
    unittest.main()
