"""Escrow option parity and frozen steering; no native closure claim."""
from contextlib import redirect_stderr
import copy
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from test_epoch_lookup_steering import EPOCH, FAKE_CP6, MATCH, PRODUCTION
import test_frontier_rescue as rescue
import test_shared_owner_campaign as shared
from test_g2_campaign_steering import snapshot


FLAGS = ["--epoch-rolling", "--epoch-window", "76",
         "--epoch-result-escrow-jobs", "16", "--epoch-result-escrow-bytes", "1048576"]


class EpochEscrowSteeringTests(unittest.TestCase):
    def test_validation_and_native_forwarding(self):
        MATCH.validate_epoch_batch(None, None, None, False, False, "ready", False, 0, None)
        for jobs, size, order, window in [(1, None, None, 76), (0, 1, None, 76),
                                         (1, 0, None, 76), (4096, 1, None, 76),
                                         (1, 1, "oldest-ready", 76), (1, 1, None, 4096),
                                         (True, 1, None, 76)]:
            with self.subTest(jobs=jobs, size=size, order=order), self.assertRaises(ValueError):
                MATCH.validate_epoch_batch(order, None, window, True, True, "epoch", True, jobs, size)
        argv = ["match", "--executable", "native", "--manifest", "m", "--queries", "q",
                "--output", "o", "--follow-successors", *EPOCH, "--checkpoint", "cp", *FLAGS]
        with patch.object(sys, "argv", argv), patch.object(MATCH.os, "execve") as launch:
            MATCH.main()
        command = launch.call_args.args[1]
        for flag, value in [("--epoch-result-escrow-jobs", "16"), ("--epoch-result-escrow-bytes", "1048576")]:
            self.assertEqual(command.count(flag), 1)
            self.assertEqual(command[command.index(flag) + 1], value)

    def test_frozen_production_resume_binds_both_fields(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            executable = shared.production_fixture(directory)
            initial = shared.production_plan(directory, "--executable", str(executable),
                                             "--workers", "1", *EPOCH, *FLAGS)
            self.assertEqual(initial["epoch_result_escrow_jobs"], 16)
            self.assertEqual(initial["epoch_result_escrow_bytes"], 1048576)
            before = snapshot(directory)
            resumed = shared.production_plan(directory, "--resume")
            self.assertEqual(initial["steering_policy"], resumed["steering_policy"])
            for flag, value in [("--epoch-result-escrow-jobs", "8"), ("--epoch-result-escrow-bytes", "2097152")]:
                with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                    shared.production_plan(directory, "--resume", flag, value)
                self.assertEqual(snapshot(directory), before)
            altered = copy.deepcopy(initial["steering_policy"])
            command = altered["command_arguments"]
            command[command.index("--epoch-result-escrow-bytes") + 1] = "2097152"
            with self.assertRaisesRegex(ValueError, "batch options and command"):
                PRODUCTION.frozen_options(altered)

    def test_supervisor_forwarding_and_default_omission(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with patch.object(rescue, "FAKE", FAKE_CP6):
                result = rescue.supervise(directory, "--checkpoint", str(directory / "checkpoint"), *EPOCH, *FLAGS)
            self.assertEqual(result.returncode, 4, result.stderr + result.stdout)
            command = rescue.calls(directory)[0]
            self.assertEqual(command.count("--epoch-result-escrow-jobs"), 1)
            self.assertEqual(command.count("--epoch-result-escrow-bytes"), 1)
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            executable = shared.production_fixture(directory)
            initial = shared.production_plan(directory, "--executable", str(executable), "--workers", "1", *EPOCH)
            for option in MATCH.EPOCH_ESCROW_OPTIONS:
                self.assertNotIn(option.replace("-", "_"), initial["steering_policy"]["options"])
                self.assertNotIn("--" + option, initial["command"])


if __name__ == "__main__":
    unittest.main()
