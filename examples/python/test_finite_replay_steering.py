"""Explicit finite-query steering and immutable resume; no native campaign runs."""
import argparse
import copy
import importlib.util
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


def sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PRODUCTION = sibling("production_saved_owner_campaign")
SHARED = PRODUCTION.SUPERVISOR
POLICY = SHARED.FINITE_REPLAY
FLAG = "--" + POLICY.MODE
TMP = Path(__file__).resolve().parents[2] / "TMP"


class FiniteReplaySteeringTests(unittest.TestCase):
    def options(self, enabled=True):
        return dict(finite_replay_initial_queries=enabled, publication_policy="epoch",
                    epoch_rolling=True, epoch_dispatch="fifo", epoch_publication_order="oldest-prefix",
                    auto_rescue=False)

    def parser(self):
        parser = argparse.ArgumentParser(allow_abbrev=False)
        POLICY.add_arguments(parser, SHARED.DOMAIN)
        return parser

    def test_explicit_mode_and_zero_caps_are_parsed_without_invented_ceiling(self):
        options = vars(self.parser().parse_args([FLAG, "--finite-replay-max-nodes", "0",
                                               "--reduction-max-rule-applications", str(2**64 - 1)]))
        POLICY.validate({**self.options(), **options})
        self.assertEqual(POLICY.arguments(options), [FLAG, "--finite-replay-max-nodes", "0",
                         "--reduction-max-rule-applications", str(2**64 - 1)])
        absent = vars(self.parser().parse_args([]))
        self.assertEqual(POLICY.arguments(absent), [])
        POLICY.omit_absent(absent)
        self.assertEqual(absent, {})

    def test_duplicate_or_negative_arguments_fail_before_launch(self):
        for arguments in ([FLAG, FLAG], ["--finite-replay-max-nodes", "-1"],
                          ["--finite-replay-max-nodes", "1", "--finite-replay-max-nodes", "2"]):
            with self.subTest(arguments=arguments), patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    self.parser().parse_args(arguments)

    def test_unsupported_combinations_and_forged_caps_fail(self):
        for mutation in ({"publication_policy": "ready"}, {"epoch_rolling": False},
                         {"epoch_dispatch": "adaptive"}, {"epoch_publication_order": "oldest-ready"},
                         {"auto_rescue": True}, {"finite_replay_initial_queries": 1},
                         {"finite_replay_max_nodes": True}, {"finite_replay_max_nodes": "4"},
                         {"finite_replay_max_nodes": -1},
                         {"finite_replay_initial_queries": False, "finite_replay_max_nodes": 0}):
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                POLICY.validate({**self.options(), **mutation})
        for arguments in ({"symbolic": False}, {"checkpoint": False}):
            with self.assertRaises(ValueError):
                POLICY.validate(self.options(), **arguments)

    def test_frozen_enabled_values_survive_resume_and_cannot_be_changed(self):
        with tempfile.TemporaryDirectory(dir=TMP) as temporary:
            campaign = Path(temporary)
            (campaign / "bin").mkdir()
            args = argparse.Namespace(**self.options(), workers=1,
                cpus=str(min(os.sched_getaffinity(0))), finite_replay_max_nodes=0,
                finite_replay_max_transport_endpoints=20_000_000, resume=False)
            policy = PRODUCTION.frozen_policy(campaign, args, campaign / "bin/native",
                                               campaign / "inputs", 2, 1024)
            self.assertEqual(policy["command_arguments"].count(FLAG), 1)
            before = (campaign / "bin/steering.json").read_bytes()
            replay = argparse.Namespace(resume=True)
            self.assertEqual(PRODUCTION.frozen_policy(campaign, replay, campaign, campaign, 5, 2048), policy)
            replay.finite_replay_max_nodes = 1
            with self.assertRaisesRegex(ValueError, "differs from frozen policy"):
                PRODUCTION.frozen_policy(campaign, replay, campaign, campaign, 5, 2048)
            self.assertEqual((campaign / "bin/steering.json").read_bytes(), before)
            self.assertEqual(PRODUCTION.frozen_options(policy)["finite_replay_max_nodes"], 0)

    def test_default_off_keeps_new_keys_and_native_flags_absent(self):
        with tempfile.TemporaryDirectory(dir=TMP) as temporary:
            campaign = Path(temporary)
            (campaign / "bin").mkdir()
            args = argparse.Namespace(workers=1, cpus=str(min(os.sched_getaffinity(0))),
                                      auto_rescue=False, resume=False)
            policy = PRODUCTION.frozen_policy(campaign, args, campaign, campaign, 1, 10)
            self.assertTrue(set(POLICY.KEYS).isdisjoint(policy["options"]))
            self.assertNotIn(FLAG, policy["command_arguments"])
            original = copy.deepcopy(policy)
            PRODUCTION.frozen_options(policy)
            self.assertEqual(policy, original)

    def test_tampered_frozen_argv_is_rejected(self):
        options = {**self.options(), "finite_replay_max_nodes": 7}
        good = POLICY.arguments(options)
        POLICY.validate_frozen(options, good)
        for command in (good[1:], good + [FLAG], [FLAG, "--finite-replay-max-nodes", "8"],
                        [FLAG, "--finite-replay-max-nodes=7"], good + ["--finite-replay-max-seed-points", "9"]):
            with self.subTest(command=command), self.assertRaises(ValueError):
                POLICY.validate_frozen(options, command)

    def test_supervisor_restart_retains_explicit_policy_and_zero_caps(self):
        args = argparse.Namespace(**self.options(), finite_replay_max_nodes=0,
                                  checkpoint=Path("old"), resume=None, run_directory=None, cpus="0")
        command = SHARED.restart_command(args, TMP / "finite-policy-test", "checkpoint", {0})
        self.assertIn(FLAG, command)
        self.assertEqual(command[command.index("--finite-replay-max-nodes") + 1], "0")
        self.assertEqual(command[command.index("--resume") + 1], "checkpoint")

    def test_production_prepare_and_resume_plan_forward_the_native_mode(self):
        helpers = sibling("test_shared_owner_campaign")
        with tempfile.TemporaryDirectory(dir=TMP) as temporary:
            campaign = Path(temporary)
            executable = helpers.production_fixture(campaign)
            prepared = helpers.production_plan(campaign, "--executable", str(executable),
                "--workers", "1", "--publication-policy", "epoch", "--epoch-rolling", FLAG,
                "--finite-replay-max-seed-points", "0", "--reduction-max-rule-applications", "0")
            steering = campaign / "bin/steering.json"
            before = steering.read_bytes()
            resumed = helpers.production_plan(campaign, "--resume")
            self.assertEqual(prepared["steering_policy"], resumed["steering_policy"])
            for plan in (prepared, resumed):
                command = plan["command"]
                self.assertEqual(command.count(FLAG), 1)
                for flag in ("--finite-replay-max-seed-points", "--reduction-max-rule-applications"):
                    self.assertEqual(command.count(flag), 1)
                    self.assertEqual(command[command.index(flag) + 1], "0")
            self.assertEqual(steering.read_bytes(), before)
            self.assertFalse((campaign / "active-run.json").exists())
            self.assertFalse((campaign / "checkpoints").exists())

    def test_shared_supervisor_rejects_unsupported_modes_before_spawning(self):
        base = ["shared", "--manifest", "missing", "--queries", "missing", "--executable", "missing",
                "--checkpoint", "missing", "--publication-policy", "epoch", "--epoch-rolling",
                "--transfer-unreserved-lookahead", "16", FLAG]
        for suffix in (["--epoch-dispatch", "adaptive"], ["--auto-rescue"],
                       ["--epoch-publication-order", "oldest-ready"]):
            with patch("sys.argv", base + suffix), patch.object(SHARED, "owned_process") as launch, \
                    patch("sys.stderr", new_callable=io.StringIO), self.assertRaises(SystemExit):
                SHARED.main()
            launch.assert_not_called()


if __name__ == "__main__":
    unittest.main()
