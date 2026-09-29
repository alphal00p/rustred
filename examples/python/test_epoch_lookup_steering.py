"""Epoch lookup frontend controls: fake executables only, no native solver."""
import argparse
import copy
from contextlib import redirect_stderr
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
from test_epoch_checkpoint_steering import MATCH
from test_g2_campaign_steering import snapshot

PRODUCTION = shared.PRODUCTION
SUPERVISOR = shared.CAMPAIGN
FLAG = "--epoch-inspector-lookup"
EPOCH = ("--publication-policy", "epoch", "--transfer-unreserved-lookahead", "16")
FAKE_CP6 = r'''
import json, sys
from pathlib import Path
here = Path(sys.argv[0]).resolve().parent
with (here / "calls.jsonl").open("a") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")
arg = lambda name: sys.argv[sys.argv.index(name) + 1] if name in sys.argv else None
checkpoint = arg("--checkpoint") or arg("--resume")
Path(checkpoint).mkdir(exist_ok=True)
row = {"event": "finished", "status": "incomplete", "finalization": "not_evaluated",
       "full_result_in_output_document": False, "full_state_in_checkpoint": True,
       "recursive_worklist_exhausted": True, "all_scheduled_domains_resolved": False,
       "checkpoint": {"state": "saved", "format": "RUSTRED-WALK-CP6", "schema": 1,
                      "generation": 1, "resumable": True, "directory": checkpoint}}
Path(arg("--output")).write_text(json.dumps(row))
Path(arg("--events")).write_text(json.dumps({"event": "progress", "progress": row}) + "\n")
raise SystemExit(4)
'''


class EpochLookupSteeringTests(unittest.TestCase):
    def test_thin_launcher_fresh_resume_explicit_modes_and_absent_default(self):
        base = ["match", "--executable", "native", "--manifest", "m", "--queries", "q",
                "--output", "o", "--follow-successors"]
        for operation in ("--checkpoint", "--resume"):
            for mode in (None, "all-miss", "snapshot"):
                extra = [] if mode is None else [FLAG, mode]
                with patch.object(sys, "argv", base + list(EPOCH) + [operation, "cp"] + extra), \
                        patch.object(MATCH.os, "execve") as launch:
                    MATCH.main()
                command = launch.call_args.args[1]
                self.assertEqual(command.count(FLAG), int(mode is not None))
                if mode is not None:
                    self.assertEqual(command[command.index(FLAG) + 1], mode)
                self.assertEqual(command[command.index(operation) + 1], "cp")
        for extra in ([FLAG, "snapshot"], [*EPOCH, FLAG, "all-miss"],
                      [*EPOCH, "--checkpoint", "cp", FLAG, "snapshot", FLAG, "snapshot"],
                      [*EPOCH, "--checkpoint", "cp", "--epoch-inspector-l=snapshot"]):
            with patch.object(sys, "argv", base + extra), patch.object(MATCH.os, "execve") as launch, \
                    redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                MATCH.main()
            launch.assert_not_called()

    def test_fresh_snapshot_plan_and_resume_freeze_mode_without_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            executable = shared.production_fixture(directory)
            with patch.object(PRODUCTION.os, "execv") as launch:
                initial = shared.production_plan(directory, "--executable", str(executable),
                                                 "--workers", "1", *EPOCH, FLAG, "snapshot")
                self.assertEqual(initial["epoch_checkpoint"]["inspector_lookup_mode"], "snapshot")
                before = snapshot(directory)
                for extra in ((), (FLAG, "snapshot")):
                    resumed = shared.production_plan(directory, "--resume", *extra)
                    self.assertEqual(resumed["epoch_inspector_lookup"], "snapshot")
                    self.assertEqual(resumed["steering_policy"], initial["steering_policy"])
                    self.assertEqual(resumed["command"].count(FLAG), 1)
                    self.assertEqual(resumed["command"][resumed["command"].index(FLAG) + 1], "snapshot")
                    self.assertEqual(snapshot(directory), before)
                with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                    shared.production_plan(directory, "--resume", FLAG, "all-miss")
                self.assertEqual(snapshot(directory), before)
                launch.assert_not_called()

    def test_legacy_epoch_all_miss_and_cp5_default_keep_frozen_bytes(self):
        for publication in ("epoch", "ready"):
            with self.subTest(publication=publication), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                executable = shared.production_fixture(directory)
                initial = shared.production_plan(directory, "--executable", str(executable),
                                                 "--workers", "1", "--publication-policy", publication)
                self.assertNotIn(FLAG, initial["command"])
                path = directory / "bin" / "steering.json"
                policy = json.loads(path.read_text())
                if publication == "epoch":
                    self.assertEqual(policy["options"].pop("epoch_inspector_lookup"), "all-miss")
                    path.chmod(0o644)
                    path.write_text(json.dumps(policy))
                    path.chmod(0o444)
                else:
                    self.assertNotIn("epoch_inspector_lookup", policy["options"])
                    self.assertNotIn("epoch_inspector_lookup", initial)
                before = snapshot(directory)
                for extra in ((), (FLAG, "all-miss")) if publication == "epoch" else ((),):
                    resumed = shared.production_plan(directory, "--resume", *extra)
                    self.assertEqual(resumed["steering_policy"], policy)
                    self.assertNotIn(FLAG, resumed["command"])
                    self.assertEqual(resumed["command"][2:-4], initial["steering_policy"]["command_arguments"])
                    self.assertEqual(snapshot(directory), before)
                    if publication == "epoch":
                        self.assertEqual(resumed["epoch_inspector_lookup"], "all-miss")
                with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                    shared.production_plan(directory, "--resume", FLAG, "snapshot")
                self.assertEqual(snapshot(directory), before)

    def test_frozen_mode_and_canonical_command_must_agree(self):
        base = {"options": {"publication_policy": "epoch"}, "command_arguments": []}
        self.assertEqual(PRODUCTION.frozen_options(base)["epoch_inspector_lookup"], "all-miss")
        self.assertNotIn("epoch_inspector_lookup", base["options"])
        for mode, argv, publication in (
                ("snapshot", [], "epoch"), ("all-miss", [FLAG, "snapshot"], "epoch"),
                ("snapshot", [FLAG, "all-miss"], "epoch"), ("invalid", [], "epoch"),
                ("snapshot", [FLAG], "epoch"),
                ("snapshot", [FLAG, "snapshot", FLAG, "snapshot"], "epoch"),
                ("snapshot", [FLAG + "=snapshot"], "epoch"),
                ("all-miss", [FLAG + "=snapshot"], "epoch"),
                ("snapshot", [FLAG, "snapshot"], "ready"), ("all-miss", [], "ready")):
            with self.subTest(mode=mode, argv=argv, publication=publication):
                policy = copy.deepcopy(base)
                policy["options"].update(epoch_inspector_lookup=mode, publication_policy=publication)
                policy["command_arguments"] = argv
                with self.assertRaisesRegex(ValueError, "mode and command disagree"):
                    PRODUCTION.frozen_options(policy)

    def test_prepare_snapshot_preserves_inputs_and_never_starts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest, queries = staging.AnchorPlanningTests().fixture(root)
            document = json.loads(queries.read_text())
            document["query_roles"] = {"required": ["original"], "auxiliary": []}
            queries.write_text(json.dumps(document))
            source = root / "source"
            staging.STAGE.stage(manifest, queries, source / "inputs", root)
            before = snapshot(source)
            executable = root / "fake-rustred"
            executable.write_text(f"#!{sys.executable}\nraise SystemExit('must not execute')\n")
            executable.chmod(0o700)
            destination = root / "fresh"
            with patch.object(PRODUCTION.os, "execv") as launch:
                plan = shared.production_plan(destination, "--prepare-from", str(source),
                                              "--executable", str(executable), "--workers", "1",
                                              *EPOCH, FLAG, "snapshot")
                launch.assert_not_called()
            self.assertEqual(plan["epoch_inspector_lookup"], "snapshot")
            self.assertEqual(json.loads((destination / "inputs" / "queries.json").read_text()), document)
            self.assertEqual(snapshot(source), before)
            self.assertFalse((destination / "runs").exists())
            self.assertFalse((destination / "checkpoints").exists())

    def test_fake_supervisor_forwards_modes_and_records_effective_policy_and_restart(self):
        for mode in (None, "all-miss", "snapshot"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                extra = [] if mode is None else [FLAG, mode]
                with patch.object(rescue, "FAKE", FAKE_CP6):
                    result = rescue.supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                                               *EPOCH, *extra)
                self.assertEqual(result.returncode, 4, result.stderr + result.stdout)
                command = rescue.calls(directory)[0]
                self.assertEqual(command.count(FLAG), int(mode is not None))
                if mode is not None:
                    self.assertEqual(command[command.index(FLAG) + 1], mode)
                for filename in ("request.json", "status.json", "supervisor-result.json"):
                    row = json.loads((directory / "run" / filename).read_text())
                    self.assertEqual(row["epoch_inspector_lookup"], mode or "all-miss")
                status = json.loads((directory / "run" / "status.json").read_text())
                self.assertEqual(status["state"], "checkpoint_only")
                args = argparse.Namespace(publication_policy="epoch", epoch_inspector_lookup=mode,
                                          checkpoint=directory / "checkpoint", resume=None,
                                          run_directory=directory / "run", cpus="32", auto_rescue=False)
                restarted = SUPERVISOR.restart_command(args, directory / "run", str(directory / "checkpoint"), {32})
                self.assertEqual(restarted.count(FLAG), int(mode is not None))
                if mode is not None:
                    self.assertEqual(restarted[restarted.index(FLAG) + 1], mode)

    def test_invalid_scope_duplicate_and_abbreviation_refused_before_launch(self):
        invalid = [
            ("--targets", "missing", FLAG, "snapshot"),
            ("--queries", "missing", FLAG, "all-miss"),
            ("--queries", "missing", *EPOCH, FLAG, "snapshot"),
            ("--queries", "missing", *EPOCH, "--checkpoint", "cp", FLAG, "snapshot", FLAG, "snapshot"),
            ("--queries", "missing", *EPOCH, "--checkpoint", "cp", "--epoch-inspector-l=snapshot"),
        ]
        for extra in invalid:
            with self.subTest(extra=extra), patch.object(sys, "argv", ["shared", "--executable", "missing", "--manifest", "missing", *extra]), \
                    patch.object(SUPERVISOR, "owned_process") as launch, redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as refusal:
                    SUPERVISOR.main()
                self.assertEqual(refusal.exception.code, 2)
                launch.assert_not_called()
        for extra in ((FLAG, "snapshot"), (FLAG, "all-miss", "--publication-policy", "ready"),
                      (*EPOCH, FLAG, "snapshot", FLAG, "snapshot"), ("--epoch-inspector-l=snapshot",)):
            with self.subTest(extra=extra), patch.object(PRODUCTION, "verify_inputs") as verify, \
                    patch.object(PRODUCTION, "freeze_executable") as freeze, redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as refusal:
                    PRODUCTION.main(list(extra))
                self.assertEqual(refusal.exception.code, 2)
                verify.assert_not_called()
                freeze.assert_not_called()


if __name__ == "__main__":
    unittest.main()
