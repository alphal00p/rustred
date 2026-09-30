"""Runtime S5 configuration contracts; fake launches are not solver evidence."""
import copy
from contextlib import redirect_stderr
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from test_epoch_lookup_steering import EPOCH, FAKE_CP6, MATCH, PRODUCTION
from test_g2_campaign_steering import snapshot
import test_frontier_rescue as rescue
import test_shared_owner_campaign as shared


class EpochPreparationTests(unittest.TestCase):
    def test_partition_and_limits_are_generic_runtime_data(self):
        for workers in (1, 2, 6, 50, 200):
            available = max(1, workers - 1)
            for helpers in range(available):
                options = {"epoch_preparation_workers": helpers,
                           "epoch_preparation_max_obligations": 123,
                           "epoch_preparation_max_retirements": (1 << 32) - 1}
                MATCH.validate_epoch_preparation(options, True, "epoch", workers)
                MATCH.validate_epoch_preparation(options, True, "epoch", workers, available - helpers)
                with self.assertRaises(ValueError):
                    MATCH.validate_epoch_preparation(options, True, "ready", workers)
            for invalid in (available, -1, True, 1 << 60):
                with self.subTest(workers=workers, invalid=invalid), self.assertRaises(ValueError):
                    MATCH.validate_epoch_preparation({"epoch_preparation_workers": invalid}, True, "epoch", workers)
        for name in MATCH.EPOCH_PREPARATION_OPTIONS[1:]:
            for invalid in (0, -1, True, 1 << 32):
                with self.subTest(name=name, invalid=invalid), self.assertRaises(ValueError):
                    MATCH.validate_epoch_preparation({name.replace("-", "_"): invalid}, True, "epoch", 6)
        with self.assertRaises(ValueError):
            MATCH.validate_epoch_preparation({"epoch_preparation_workers": 1}, True, "epoch", 6, 5)
        with self.assertRaises(ValueError):
            MATCH.validate_epoch_preparation({"epoch_preparation_workers": 1}, True, "epoch", 6, None, 7)

    def test_fresh_and_resumed_policy_freezes_configuration_without_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            executable = shared.production_fixture(directory)
            flags = ["--epoch-preparation-workers", "0", "--epoch-preparation-max-obligations", "123",
                     "--epoch-preparation-max-retirements", "456"]
            with patch.object(PRODUCTION.os, "execv") as launch:
                initial = shared.production_plan(directory, "--executable", str(executable),
                                                 "--workers", "1", *EPOCH, *flags)
                before = snapshot(directory)
                resumed = shared.production_plan(directory, "--resume")
                self.assertEqual(initial["steering_policy"], resumed["steering_policy"])
                self.assertEqual(snapshot(directory), before)
                self.assertEqual(initial["epoch_checkpoint"]["schema"], 3)
                self.assertEqual(initial["epoch_checkpoint"]["walk_semantics_version"], 4)
                for name, expected in zip(MATCH.EPOCH_PREPARATION_OPTIONS, (0, 123, 456)):
                    self.assertEqual(initial[name.replace("-", "_")], expected)
                    self.assertEqual(initial["command"].count("--" + name), 1)
                    with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                        shared.production_plan(directory, "--resume", "--" + name, str(expected + 1))
                    self.assertEqual(snapshot(directory), before)
                launch.assert_not_called()

    def test_wrong_scope_and_duplicate_options_refuse_before_staging(self):
        for bad in (["--epoch-preparation-workers", "0"],
                    [*EPOCH, "--workers", "1", "--epoch-preparation-workers", "1"],
                    [*EPOCH, "--epoch-preparation-max-obligations", "0"],
                    [*EPOCH, "--epoch-preparation-max-retirements", str(1 << 32)],
                    [*EPOCH, "--epoch-preparation-workers", "0", "--epoch-preparation-workers", "0"]):
            with self.subTest(bad=bad), patch.object(PRODUCTION, "verify_inputs") as verify, \
                    patch.object(PRODUCTION, "freeze_executable") as freeze, \
                    redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                PRODUCTION.main(bad)
            verify.assert_not_called()
            freeze.assert_not_called()

    def test_native_supervisor_and_frozen_command_forward_exactly_once(self):
        flags = ["--epoch-preparation-workers", "0", "--epoch-preparation-max-obligations", "123",
                 "--epoch-preparation-max-retirements", "456"]
        argv = ["match", "--executable", "native", "--manifest", "m", "--queries", "q",
                "--output", "o", "--follow-successors", "--workers", "1", *EPOCH, *flags]
        with patch.object(sys, "argv", argv), patch.object(MATCH.os, "execve") as launch:
            MATCH.main()
        for name, expected in zip(MATCH.EPOCH_PREPARATION_OPTIONS, ("0", "123", "456")):
            command = launch.call_args.args[1]
            self.assertEqual(command.count("--" + name), 1)
            self.assertEqual(command[command.index("--" + name) + 1], expected)
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with patch.object(rescue, "FAKE", FAKE_CP6):
                result = rescue.supervise(directory, "--checkpoint", str(directory / "checkpoint"), *EPOCH, *flags)
            self.assertEqual(result.returncode, 4, result.stderr + result.stdout)
            for filename in ("request.json", "status.json", "supervisor-result.json"):
                row = json.loads((directory / "run" / filename).read_text())
                for name, value in zip(MATCH.EPOCH_PREPARATION_OPTIONS, (0, 123, 456)):
                    self.assertEqual(row[name.replace("-", "_")], value)
        base = {"options": {"publication_policy": "epoch", "workers": 1,
                            "epoch_preparation_workers": 0}, "command_arguments": flags[:2]}
        self.assertEqual(PRODUCTION.frozen_options(base)["epoch_preparation_workers"], 0)
        for command in ([], flags[:2] * 2, ["--epoch-preparation-workers=0"]):
            bad = copy.deepcopy(base)
            bad["command_arguments"] = command
            with self.assertRaises(ValueError):
                PRODUCTION.frozen_options(bad)


if __name__ == "__main__":
    unittest.main()
