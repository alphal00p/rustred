"""Rolling Epoch steering/binding tests; fake executables are not native proof."""
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

FLAG = "--epoch-rolling"


class EpochRollingSteeringTests(unittest.TestCase):
    def test_adaptive_policy_is_explicit_frozen_and_requires_rolling(self):
        for mode in (None, "fifo", "adaptive"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                executable = shared.production_fixture(directory)
                extra = [] if mode is None else ["--epoch-dispatch", mode]
                initial = shared.production_plan(directory, "--executable", str(executable),
                                                 "--workers", "1", *EPOCH, FLAG, *extra)
                self.assertEqual(initial["epoch_dispatch"], mode or "fifo")
                self.assertEqual(initial["command"].count("--epoch-dispatch"), int(mode == "adaptive"))
                before = snapshot(directory)
                resumed = shared.production_plan(directory, "--resume", *extra)
                self.assertEqual(resumed["steering_policy"], initial["steering_policy"])
                self.assertEqual(snapshot(directory), before)
                other = "fifo" if mode == "adaptive" else "adaptive"
                with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                    shared.production_plan(directory, "--resume", "--epoch-dispatch", other)
                self.assertEqual(snapshot(directory), before)
        with patch.object(PRODUCTION, "verify_inputs") as verify, \
                patch.object(PRODUCTION, "freeze_executable") as freeze, redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                PRODUCTION.main([*EPOCH, "--epoch-dispatch", "adaptive"])
            verify.assert_not_called()
            freeze.assert_not_called()

    def test_adaptive_argv_mismatch_refused_and_thin_launcher_propagates(self):
        for policy, rolling, values in [("adaptive", True, []), ("fifo", True, ["adaptive"]),
                ("adaptive", False, ["adaptive"]), ("invalid", True, ["invalid"])]:
            command = [FLAG] if rolling else []
            if values:
                command += ["--epoch-dispatch", *values]
            row = {"options": {"publication_policy": "epoch", "epoch_rolling": rolling,
                               "epoch_dispatch": policy}, "command_arguments": command}
            with self.subTest(policy=policy, rolling=rolling, values=values), \
                    self.assertRaisesRegex(ValueError, "dispatch policy and command disagree"):
                PRODUCTION.frozen_options(row)
        argv = ["match", "--executable", "native", "--manifest", "m", "--queries", "q",
                "--output", "o", "--follow-successors", *EPOCH, "--checkpoint", "cp",
                FLAG, "--epoch-dispatch", "adaptive"]
        with patch.object(sys, "argv", argv), patch.object(MATCH.os, "execve") as launch:
            MATCH.main()
        command = launch.call_args.args[1]
        self.assertEqual(command.count("--epoch-dispatch"), 1)
        self.assertEqual(command[command.index("--epoch-dispatch") + 1], "adaptive")

    def test_thin_launcher_forwards_only_explicit_opt_in_and_refuses_wrong_scope(self):
        base = ["match", "--executable", "native", "--manifest", "m", "--queries", "q",
                "--output", "o", "--follow-successors"]
        for operation in ("--checkpoint", "--resume"):
            for extra in ([], [FLAG]):
                with patch.object(sys, "argv", base + list(EPOCH) + [operation, "cp"] + extra), \
                        patch.object(MATCH.os, "execve") as launch:
                    MATCH.main()
                self.assertEqual(launch.call_args.args[1].count(FLAG), int(bool(extra)))
        for extra in ([FLAG], [*EPOCH, FLAG], [*EPOCH, "--checkpoint", "cp", FLAG, FLAG]):
            with patch.object(sys, "argv", base + extra), patch.object(MATCH.os, "execve") as launch, \
                    redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                MATCH.main()
            launch.assert_not_called()

    def test_production_freezes_rolling_without_changing_old_campaign_bytes(self):
        for rolling in (False, True):
            with self.subTest(rolling=rolling), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                executable = shared.production_fixture(directory)
                extra = [FLAG] if rolling else []
                initial = shared.production_plan(directory, "--executable", str(executable),
                                                 "--workers", "1", *EPOCH, *extra)
                self.assertEqual(initial["epoch_rolling"], rolling)
                self.assertEqual(initial["command"].count(FLAG), int(rolling))
                before = snapshot(directory)
                resumed = shared.production_plan(directory, "--resume")
                self.assertEqual(resumed["steering_policy"], initial["steering_policy"])
                self.assertEqual(resumed["epoch_checkpoint"]["rolling"], rolling)
                self.assertEqual(snapshot(directory), before)
                if not rolling:
                    with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                        shared.production_plan(directory, "--resume", FLAG)
                    self.assertEqual(snapshot(directory), before)

    def test_frozen_rolling_flag_is_boolean_and_matches_command(self):
        base = {"options": {"publication_policy": "epoch"}, "command_arguments": []}
        self.assertIs(PRODUCTION.frozen_options(base)["epoch_rolling"], False)
        self.assertNotIn("epoch_rolling", base["options"])
        for value, command, policy in [(True, [], "epoch"), (False, [FLAG], "epoch"),
                (True, [FLAG, FLAG], "epoch"), (True, [FLAG + "=true"], "epoch"),
                ("true", [FLAG], "epoch"), (1, [FLAG], "epoch"), (True, [FLAG], "ready")]:
            row = copy.deepcopy(base)
            row["options"].update(epoch_rolling=value, publication_policy=policy)
            row["command_arguments"] = command
            with self.subTest(value=value, command=command, policy=policy), \
                    self.assertRaisesRegex(ValueError, "rolling mode and command disagree"):
                PRODUCTION.frozen_options(row)

    def test_fake_supervisor_forwards_records_and_restarts_rolling(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with patch.object(rescue, "FAKE", FAKE_CP6):
                result = rescue.supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                                           *EPOCH, FLAG)
            self.assertEqual(result.returncode, 4, result.stderr + result.stdout)
            self.assertEqual(rescue.calls(directory)[0].count(FLAG), 1)
            for name in ("request.json", "status.json", "supervisor-result.json"):
                row = json.loads((directory / "run" / name).read_text())
                self.assertIs(row["epoch_rolling"], True)
            # Restart comes from the stored complete argument namespace, not a
            # second policy inference from the checkpoint's result summary.
            from argparse import Namespace
            args = Namespace(publication_policy="epoch", epoch_rolling=True,
                             checkpoint=directory / "checkpoint", resume=None,
                             run_directory=directory / "run", cpus="32", auto_rescue=False)
            command = SUPERVISOR.restart_command(args, directory / "run", str(directory / "checkpoint"), {32})
            self.assertEqual(command.count(FLAG), 1)


if __name__ == "__main__":
    unittest.main()
