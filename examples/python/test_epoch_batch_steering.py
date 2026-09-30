"""Publication-batch steering tests; mocked launch is not native closure proof."""
import copy
from contextlib import redirect_stderr
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from test_epoch_lookup_steering import EPOCH, FAKE_CP6, MATCH, PRODUCTION, SUPERVISOR
import test_frontier_rescue as rescue
import test_shared_owner_campaign as shared
from test_g2_campaign_steering import snapshot


class EpochBatchSteeringTests(unittest.TestCase):
    def test_full_options_are_frozen_and_omitted_resume_reuses_them(self):
        for order in ("oldest-prefix", "oldest-ready"):
            with self.subTest(order=order), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                executable = shared.production_fixture(directory)
                flags = ["--epoch-rolling", "--epoch-publication-order", order,
                         "--epoch-cut-size", "16", "--epoch-window", "800"]
                initial = shared.production_plan(directory, "--executable", str(executable),
                                                 "--workers", "1", *EPOCH, *flags)
                self.assertEqual(initial["epoch_publication_order"], order)
                self.assertEqual(initial["epoch_cut_size"], 16)
                self.assertEqual(initial["epoch_window"], 800)
                before = snapshot(directory)
                resumed = shared.production_plan(directory, "--resume")
                self.assertEqual(initial["steering_policy"], resumed["steering_policy"])
                self.assertEqual(resumed["epoch_checkpoint"]["publication_order"], order)
                for option, wrong in (("epoch-publication-order", "oldest-prefix" if order == "oldest-ready" else "oldest-ready"),
                                      ("epoch-cut-size", "32"), ("epoch-window", "1600")):
                    with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                        shared.production_plan(directory, "--resume", "--" + option, wrong)
                    self.assertEqual(snapshot(directory), before)

    def test_frozen_options_reject_malformed_or_conflicting_command(self):
        base = {"options": {"publication_policy": "epoch", "epoch_rolling": True,
                            "epoch_publication_order": "oldest-ready", "epoch_cut_size": 16,
                            "epoch_window": 32},
                "command_arguments": ["--epoch-rolling", "--epoch-publication-order", "oldest-ready",
                                      "--epoch-cut-size", "16", "--epoch-window", "32"]}
        self.assertEqual(PRODUCTION.frozen_options(base)["epoch_window"], 32)
        for command in (base["command_arguments"][:-2], base["command_arguments"] + ["--epoch-window", "32"],
                        base["command_arguments"] + ["--epoch-window=32"],
                        [*base["command_arguments"][:-1], "64"]):
            row = copy.deepcopy(base)
            row["command_arguments"] = command
            with self.subTest(command=command), self.assertRaisesRegex(ValueError, "batch options and command"):
                PRODUCTION.frozen_options(row)

    def test_invalid_scope_values_and_duplicates_fail_before_preparation(self):
        for bad in (["--epoch-publication-order", "oldest-ready"],
                    [*EPOCH, "--epoch-cut-size", "16"],
                    [*EPOCH, "--epoch-rolling", "--epoch-window", "15"],
                    [*EPOCH, "--epoch-rolling", "--epoch-cut-size", "0"],
                    [*EPOCH, "--epoch-rolling", "--epoch-window", "4097"],
                    [*EPOCH, "--epoch-rolling", "--epoch-window", "32", "--epoch-window", "64"]):
            with self.subTest(bad=bad), patch.object(PRODUCTION, "verify_inputs") as verify, \
                    patch.object(PRODUCTION, "freeze_executable") as freeze, \
                    redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                PRODUCTION.main(bad)
            verify.assert_not_called()
            freeze.assert_not_called()

    def test_native_and_supervisor_forward_controls_once(self):
        flags = ["--epoch-rolling", "--epoch-publication-order", "oldest-ready",
                 "--epoch-cut-size", "8", "--epoch-window", "64"]
        argv = ["match", "--executable", "native", "--manifest", "m", "--queries", "q",
                "--output", "o", "--follow-successors", *EPOCH, "--checkpoint", "cp", *flags]
        with patch.object(sys, "argv", argv), patch.object(MATCH.os, "execve") as launch:
            MATCH.main()
        command = launch.call_args.args[1]
        for name, value in (("epoch-publication-order", "oldest-ready"), ("epoch-cut-size", "8"), ("epoch-window", "64")):
            self.assertEqual(command.count("--" + name), 1)
            self.assertEqual(command[command.index("--" + name) + 1], value)
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with patch.object(rescue, "FAKE", FAKE_CP6):
                result = rescue.supervise(directory, "--checkpoint", str(directory / "checkpoint"), *EPOCH, *flags)
            self.assertEqual(result.returncode, 4, result.stderr + result.stdout)
            for name in ("request.json", "status.json", "supervisor-result.json"):
                row = json.loads((directory / "run" / name).read_text())
                self.assertEqual(row["epoch_publication_order"], "oldest-ready")
                self.assertEqual(row["epoch_cut_size"], 8)
                self.assertEqual(row["epoch_window"], 64)
            command = rescue.calls(directory)[0]
            self.assertEqual(command.count("--epoch-window"), 1)

    def test_default_production_policy_does_not_add_batch_fields(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            executable = shared.production_fixture(directory)
            initial = shared.production_plan(directory, "--executable", str(executable), "--workers", "1", *EPOCH)
            for name in MATCH.EPOCH_BATCH_OPTIONS:
                self.assertNotIn(name.replace("-", "_"), initial["steering_policy"]["options"])
                self.assertNotIn("--" + name, initial["command"])


if __name__ == "__main__":
    unittest.main()
