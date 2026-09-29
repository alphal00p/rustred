"""G2 Python policy/argv tests; fake children only, no solver or algebra."""
from contextlib import redirect_stderr
import copy
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import test_frontier_rescue as rescue
import test_shared_owner_campaign as shared
import test_stage_saved_owner_campaign as staging


PRODUCTION = shared.PRODUCTION
CAMPAIGN = shared.CAMPAIGN
FLAG = "--g2-residual-anchors"


def snapshot(directory):
    return {str(path.relative_to(directory)): (path.stat().st_mode, path.read_bytes())
            for path in directory.rglob("*") if path.is_file()}


class G2SteeringTests(unittest.TestCase):
    def test_fresh_union_is_frozen_and_normal_resume_retains_it_without_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            executable = shared.production_fixture(directory)
            with patch.object(PRODUCTION.os, "execv") as launch:
                initial = shared.production_plan(directory, "--executable", str(executable),
                                                 "--workers", "1", FLAG, "union")
                before = snapshot(directory)
                for extra in ((), (FLAG, "union")):
                    resumed = shared.production_plan(directory, "--resume", *extra)
                    self.assertEqual(resumed["g2_residual_anchors"], "union")
                    self.assertEqual(resumed["steering_policy"], initial["steering_policy"])
                    self.assertEqual(resumed["command"].count(FLAG), 1)
                    self.assertEqual(resumed["command"][resumed["command"].index(FLAG) + 1], "union")
                    self.assertNotIn("--g2-activate-on-resume", resumed["command"])
                    self.assertEqual(snapshot(directory), before)
                for options in (("--resume", FLAG, "off"), (FLAG, "off")):
                    with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as refusal:
                        shared.production_plan(directory, *options)
                    self.assertEqual(refusal.exception.code, 2)
                    self.assertEqual(snapshot(directory), before)
                launch.assert_not_called()

    def test_legacy_off_policy_is_read_only_and_cannot_be_activated_here(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            executable = shared.production_fixture(directory)
            initial = shared.production_plan(directory, "--executable", str(executable), "--workers", "1")
            self.assertEqual(initial["g2_residual_anchors"], "off")
            self.assertNotIn(FLAG, initial["command"])
            path = directory / "bin" / "steering.json"
            policy = json.loads(path.read_text())
            del policy["options"]["g2_residual_anchors"]  # Existing pre-option steering.
            path.chmod(0o644)
            path.write_text(json.dumps(policy))
            path.chmod(0o444)
            before = snapshot(directory)
            for extra in ((), (FLAG, "off")):
                resumed = shared.production_plan(directory, "--resume", *extra)
                self.assertEqual(resumed["g2_residual_anchors"], "off")
                self.assertEqual(resumed["steering_policy"], policy)
                self.assertNotIn("g2_residual_anchors", PRODUCTION.frozen_options(policy))
                self.assertNotIn(FLAG, resumed["command"])
                self.assertEqual(resumed["command"][2:-4], initial["steering_policy"]["command_arguments"])
                self.assertEqual(snapshot(directory), before)
            for options in (("--resume", FLAG, "union"), (FLAG, "union"),
                            ("--resume", "--g2-activate-on-resume")):
                with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as refusal:
                    shared.production_plan(directory, *options)
                self.assertEqual(refusal.exception.code, 2)
                self.assertEqual(snapshot(directory), before)

    def test_prepare_from_union_preserves_input_objects_and_source_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, queries = staging.AnchorPlanningTests().fixture(root)
            document = json.loads(queries.read_text())
            document["query_roles"] = {"required": ["original"], "auxiliary": []}
            queries.write_text(json.dumps(document))
            source = root / "source"
            staging.STAGE.stage(manifest, queries, source / "inputs", root)
            source_before = snapshot(source)
            executable = root / "fake-rustred"
            executable.write_text(f"#!{sys.executable}\nraise SystemExit('must not execute')\n")
            executable.chmod(0o700)
            destination = root / "fresh"
            with patch.object(PRODUCTION.os, "execv") as launch:
                plan = shared.production_plan(destination, "--prepare-from", str(source),
                                              "--executable", str(executable), "--workers", "1", FLAG, "union")
                launch.assert_not_called()
            self.assertEqual(plan["g2_residual_anchors"], "union")
            self.assertEqual(json.loads((destination / "inputs" / "queries.json").read_text()), document)
            self.assertEqual(snapshot(source), source_before)
            old = PRODUCTION.verify_inputs(source / "inputs")[2]
            new = PRODUCTION.verify_inputs(destination / "inputs")[2]
            self.assertEqual([(row["mask"], row["sha256"], row["bytes"]) for row in old["owners"]],
                             [(row["mask"], row["sha256"], row["bytes"]) for row in new["owners"]])
            self.assertFalse((destination / "runs").exists())
            self.assertFalse((destination / "checkpoints").exists())

    def test_frozen_mode_and_forwarded_argv_cannot_disagree(self):
        base = {"options": {}, "command_arguments": []}
        self.assertNotIn("g2_residual_anchors", PRODUCTION.frozen_options(base))
        for mode, argv in (("union", []), ("off", [FLAG, "union"]),
                           ("union", [FLAG, "off"]), ("invalid", []),
                           ("union", [FLAG]), ("union", [FLAG, "union", FLAG, "union"]),
                           ("off", [FLAG + "=union"]), ("union", [FLAG + "=union"])):
            with self.subTest(mode=mode, argv=argv):
                policy = copy.deepcopy(base)
                policy["options"]["g2_residual_anchors"] = mode
                policy["command_arguments"] = argv
                with self.assertRaisesRegex(ValueError, "mode and command disagree"):
                    PRODUCTION.frozen_options(policy)

    def test_native_invalid_combinations_are_refused_before_shared_launch(self):
        invalid = [
            ("--targets", "missing", FLAG, "union"),
            ("--queries", "missing", FLAG, "union"),
            ("--queries", "missing", FLAG, "union", "--transfer-unreserved-lookahead", "2",
             "--publication-policy", "owner-batched"),
            ("--queries", "missing", FLAG, "union", "--transfer-unreserved-lookahead", "2",
             "--apply-subdivision-axis", "0", "--apply-subdivision-cut", "1"),
            ("--queries", "missing", FLAG, "union", "--transfer-unreserved-lookahead", "2",
             "--max-containment-checks", "10"),
            ("--queries", "missing", FLAG, "union", FLAG, "off"),
            ("--queries", "missing", "--g2-activate-on-resume"),
            ("--queries", "missing", "--g2-residual-a=union"),
            ("--queries", "missing", "--g=union"),
        ]
        for extra in invalid:
            command = [str(shared.SOURCE), "--manifest", "missing", "--executable", "missing", *extra]
            with self.subTest(extra=extra), patch.object(sys, "argv", command), \
                    patch.object(CAMPAIGN, "owned_process") as launch, redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as refusal:
                    CAMPAIGN.main()
                self.assertEqual(refusal.exception.code, 2)
                launch.assert_not_called()
        for extra in ((FLAG, "union", "--publication-policy", "ordered",
                       "--apply-subdivision-axis", "0", "--apply-subdivision-cut", "1"),
                      ("--g2-residual-a=union",), ("--g=union",), (FLAG, "union", FLAG, "off")):
            with self.subTest(extra=extra), patch.object(PRODUCTION, "verify_inputs") as verify, \
                    patch.object(PRODUCTION, "freeze_executable") as freeze, redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as refusal:
                    PRODUCTION.main(list(extra))
                self.assertEqual(refusal.exception.code, 2)
                verify.assert_not_called()
                freeze.assert_not_called()

    def test_shared_off_native_argv_remains_exact(self):
        commands = []
        for extra in ((), (FLAG, "off")):
            with tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                result = rescue.supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                                           *extra, env={"FAKE_STOPS": "0"})
                self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
                command = rescue.calls(directory)[0]
                self.assertNotIn(FLAG, command)
                commands.append([value.replace(str(directory), "<fixture>") for value in command])
        self.assertEqual(*commands)

    def test_auto_rescue_replays_union_on_every_native_restart(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result = rescue.supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                                       "--frontier-policy", "stop", "--auto-rescue",
                                       "--publication-policy", "ready", "--transfer-unreserved-lookahead", "3",
                                       FLAG, "union", env={"FAKE_STOPS": "2"})
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
            native = [call for call in rescue.calls(directory) if call[0] == "owner-domain-match"]
            self.assertEqual(len(native), 3)
            for command in native:
                self.assertEqual(command.count(FLAG), 1)
                self.assertEqual(command[command.index(FLAG) + 1], "union")
                self.assertEqual(command[command.index("--publication-policy") + 1], "ready")
                self.assertEqual(command[command.index("--transfer-unreserved-lookahead") + 1], "3")
                self.assertNotIn("--g2-activate-on-resume", command)
            for output in [directory / "run", *directory.glob("run.resume-*")]:
                request = json.loads((output / "request.json").read_text())
                self.assertEqual(request["g2_residual_anchors"], "union")
                self.assertEqual(json.loads((output / "supervisor-result.json").read_text())["g2_residual_anchors"], "union")


if __name__ == "__main__":
    unittest.main()
