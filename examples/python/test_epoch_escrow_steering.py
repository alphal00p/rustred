"""Escrow option parity and frozen steering; no native closure claim."""
from contextlib import redirect_stderr
import copy
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from test_epoch_lookup_steering import EPOCH, FAKE_CP6, MATCH, PRODUCTION
import test_frontier_rescue as rescue
import test_shared_owner_campaign as shared
import test_stage_saved_owner_campaign as staging
from test_g2_campaign_steering import snapshot


FLAGS = ["--epoch-rolling", "--epoch-window", "76",
         "--epoch-result-escrow-jobs", "16", "--epoch-result-escrow-bytes", "1048576"]


class EpochEscrowSteeringTests(unittest.TestCase):
    def test_documented_delivery_prepare_start_and_resume_without_native_launch(self):
        """Exercise the real copy/freeze bridge with synthetic owner bytes only."""
        workspace = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory(dir=workspace / "TMP") as temporary:
            root = Path(temporary)
            manifest, queries = staging.AnchorPlanningTests().fixture(root)
            document = json.loads(queries.read_text())
            document["query_roles"] = {"required": ["original"], "auxiliary": []}
            queries.write_text(json.dumps(document))
            source = root / "source"
            staging.STAGE.stage(manifest, queries, source / "inputs", root)
            original = snapshot(source)
            executable = root / "fake-rustred"
            executable.write_text(f"#!{sys.executable}\nraise SystemExit('must not execute')\n")
            executable.chmod(0o700)
            destination = root / "fresh"
            flags = ["--workers", "16", "--cpus", "64-79", "--inspection-workers", "15",
                     "--publication-policy", "epoch", "--epoch-inspector-lookup", "snapshot",
                     "--epoch-rolling", "--epoch-dispatch", "fifo",
                     "--epoch-publication-order", "oldest-prefix", "--epoch-window", "76",
                     "--epoch-cut-size", "16", "--epoch-result-escrow-jobs", "1024",
                     "--epoch-result-escrow-bytes", "268435456",
                     "--epoch-preparation-workers", "0",
                     "--epoch-preparation-max-obligations", "4294967295",
                     "--epoch-preparation-max-retirements", "4294967295",
                     "--transfer-unreserved-lookahead", "256", "--g2-residual-anchors", "union",
                     "--frontier-policy", "stop", "--no-auto-rescue",
                     "--checkpoint-interval-seconds", "3600",
                     "--max-memory-bytes", "400000000000", "--ram-guard-margin-percent", "5",
                     "--host-memory-reserve-bytes", "150000000000"]
            with patch.object(PRODUCTION.os, "sched_getaffinity", return_value=set(range(64, 80))), \
                    patch.object(PRODUCTION.os, "execv") as launch:
                initial = shared.production_plan(destination, "--prepare-from", str(source),
                    "--queries", str(queries), "--query-order", "preserve",
                    "--executable", str(executable), *flags)
                launch.assert_not_called()
                self.assertFalse((destination / "checkpoints").exists())
                self.assertEqual(snapshot(source), original)
                self.assertEqual((destination / "inputs/queries.json").read_bytes(), queries.read_bytes())
                self.assertEqual(initial["epoch_result_escrow_jobs"], 1024)
                self.assertEqual(initial["epoch_window"], 76)
                self.assertFalse(initial["auto_rescue"]["auto_rescue"])
                self.assertEqual(PRODUCTION.SUPERVISOR.parse_cpu_set(initial["cpus"]), set(range(64, 80)))
                self.assertIsNone(initial["hard_timeout_seconds"])
                frozen = (destination / "bin/steering.json").read_bytes()
                # Starting the prepared destination must not copy/regenerate inputs.
                PRODUCTION.main(["--campaign-directory", str(destination), "--start"])
                launch.assert_called_once()
                start = launch.call_args.args[1]
                self.assertIn("--checkpoint", start)
                self.assertNotIn("--resume", start)
                self.assertEqual(start[start.index("--epoch-result-escrow-jobs") + 1], "1024")
                # Only the plan is examined: the mocked exec produced no checkpoint.
                resumed = shared.production_plan(destination, "--resume")
                self.assertIn("--resume", resumed["command"])
                self.assertEqual(initial["steering_policy"], resumed["steering_policy"])
                self.assertEqual((destination / "bin/steering.json").read_bytes(), frozen)
                self.assertEqual(snapshot(source), original)
                self.assertFalse((destination / "checkpoints/main").exists())

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

    def test_prepared_start_ram_override_is_ephemeral_without_checkpoint(self):
        workspace = Path(__file__).resolve().parents[2]
        with tempfile.TemporaryDirectory(dir=workspace / "TMP") as temporary:
            directory = Path(temporary)
            executable = shared.production_fixture(directory)
            initial = shared.production_plan(directory, "--executable", str(executable),
                "--workers", "1", *EPOCH, *FLAGS, "--max-memory-bytes", "400000000000")
            frozen = (directory / "bin/steering.json").read_bytes()
            self.assertFalse((directory / "checkpoints/main/latest.json").exists())
            with patch.object(PRODUCTION.os, "execv") as launch:
                PRODUCTION.main(["--campaign-directory", str(directory), "--start",
                                 "--max-memory-bytes", "750000000000"])
            command = launch.call_args.args[1]
            self.assertEqual(command[command.index("--max-memory-bytes") + 1], "750000000000")
            self.assertIn("--checkpoint", command)
            self.assertNotIn("--resume", command)
            self.assertEqual((directory / "bin/steering.json").read_bytes(), frozen)
            planned = shared.production_plan(directory, "--resume")
            self.assertEqual(planned["requested_hard_memory_bytes"], 400_000_000_000)
            self.assertEqual(planned["steering_policy"], initial["steering_policy"])
            with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                PRODUCTION.main(["--campaign-directory", str(directory), "--start", "--workers", "2"])

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
