"""Semantics-compatible executable upgrade of a paused production campaign.

Fake binaries answer only the read-only walk-semantics-version probe; no
native solver, license, algebra or real campaign is involved.
"""
from contextlib import redirect_stderr, redirect_stdout
import fcntl
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


PRODUCTION = module("production_saved_owner_campaign")
PROBE = {"walk_semantics_version": 1, "checkpoint_format": "RUSTRED-WALK-CP5", "checkpoint_schema": 5}


def fake_executable(path, body=None, probe=PROBE):
    """A script that answers only the probe; any solver invocation fails loudly."""
    if body is None:
        body = f"print(json.dumps({probe!r}))\n"
    path.write_text(f"#!{sys.executable}\n# {path.name}\nimport json, sys, time\n"
                    "if sys.argv[1:] != ['walk-semantics-version']:\n"
                    "    raise SystemExit('must not execute')\n" + body)
    path.chmod(0o700)
    return path


MISSING_PROBE = ("sys.stderr.write('rustred: usage: unknown command \"walk-semantics-version\"\\n')\n"
                 "raise SystemExit(2)\n")


def run(campaign, *options):
    output, errors = io.StringIO(), io.StringIO()
    with redirect_stdout(output), redirect_stderr(errors), patch.object(PRODUCTION.os, "execv") as launch:
        try:
            status = PRODUCTION.main(["--campaign-directory", str(campaign), *options])
        except SystemExit as error:
            status = error.code
    return status, output.getvalue(), errors.getvalue(), launch


def snapshot(directory):
    return {str(path.relative_to(directory)): (path.lstat().st_mode, path.read_bytes() if path.is_file() else None)
            for path in sorted(directory.rglob("*"))}


def start_ticks(pid):
    stat = Path(f"/proc/{pid}/stat").read_text()
    return int(stat[stat.rfind(")") + 2:].split()[19])


def boot_id():
    return Path("/proc/sys/kernel/random/boot_id").read_text().strip()


def manifest(**changes):
    return dict({"schema": 5, "format": "RUSTRED-WALK-CP5", "kind": "state", "generation": 7,
                 "walk_semantics_version": 1, "executable": "blake3-old", "executable_first": "blake3-old"},
                **changes)


class ExecutableUpgradeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.campaign = self.root / "campaign"
        inputs = self.campaign / "inputs"
        inputs.mkdir(parents=True)
        (inputs / "selection.json").write_text("{}")
        (inputs / "queries.json").write_text(json.dumps({
            "schema": "rustred.owner-domain-queries.json.v2", "queries": [{}]}))
        (inputs / "input-receipt.json").write_text(json.dumps({
            "selection_sha256": PRODUCTION.digest(inputs / "selection.json"),
            "queries_sha256": PRODUCTION.digest(inputs / "queries.json"), "owners": []}))
        self.old = fake_executable(self.root / "old-rustred")
        status, _, errors, _ = run(self.campaign, "--executable", str(self.old), "--workers", "1")
        self.assertEqual(status, 0, errors)
        self.checkpoint = self.campaign / "checkpoints" / "main"
        self.checkpoint.mkdir(parents=True)
        (self.checkpoint / "latest.json").write_text(json.dumps(manifest()))
        (self.checkpoint / "checkpoint.lock").touch()
        self.new = fake_executable(self.root / "new-rustred")

    def tearDown(self):
        self.temporary.cleanup()

    def frozen(self, path):
        return (self.campaign / "bin" / ("rustred-" + PRODUCTION.digest(path))).resolve()

    def steering(self):
        path = self.campaign / "bin" / "steering.json"
        return path.read_bytes(), json.loads(path.read_text())

    def test_dry_run_prints_both_digests_and_versions_without_mutation(self):
        before = snapshot(self.campaign)
        status, output, errors, launch = run(self.campaign, "--resume", "--upgrade-executable", str(self.new),
                                             "--json")
        self.assertEqual(status, 0, errors)
        launch.assert_not_called()
        self.assertEqual(snapshot(self.campaign), before)
        plan = json.loads(output)
        upgrade = plan["executable_upgrade"]
        self.assertFalse(upgrade["applied"])
        self.assertFalse(plan["launch_requested"])
        self.assertEqual(upgrade["frozen"]["sha256"], PRODUCTION.digest(self.old))
        self.assertEqual(upgrade["new"]["sha256"], PRODUCTION.digest(self.new))
        self.assertEqual(upgrade["new"]["probe"], PROBE)
        self.assertEqual(upgrade["checkpoint"]["walk_semantics_version"], 1)
        self.assertEqual(upgrade["checkpoint"]["generation"], 7)
        self.assertEqual(upgrade["active_run_evidence"], [])
        self.assertEqual(upgrade["steering_sha256_before"], PRODUCTION.digest(self.campaign / "bin/steering.json"))
        self.assertIsNone(plan["steering_policy_sha256"])
        self.assertEqual(plan["executable_sha256"], PRODUCTION.digest(self.new))
        command = plan["command"]
        self.assertEqual(command[command.index("--executable") + 1], str(self.frozen(self.new)))
        self.assertEqual(command[-2:], ["--resume", str(self.checkpoint)])
        status, output, errors, launch = run(self.campaign, "--resume", "--upgrade-executable", str(self.new))
        self.assertEqual(status, 0, errors)
        launch.assert_not_called()
        self.assertIn(PRODUCTION.digest(self.old), output)
        self.assertIn(PRODUCTION.digest(self.new), output)
        self.assertIn("walk semantics: checkpoint 1 (generation 7, state), new executable 1", output)
        self.assertIn("nothing was changed", output)
        self.assertEqual(snapshot(self.campaign), before)

    def test_refusals_change_nothing(self):
        cases = [
            (fake_executable(self.root / "v2", probe=dict(PROBE, walk_semantics_version=2)),
             "walk semantics version differs (checkpoint 1, new executable 2)"),
            (fake_executable(self.root / "cp6", probe=dict(PROBE, checkpoint_format="RUSTRED-WALK-CP6")),
             "but the checkpoint is RUSTRED-WALK-CP5"),
            (fake_executable(self.root / "no-probe", body=MISSING_PROBE),
             "has no usable walk-semantics-version probe (exit status 2: rustred: usage"),
            (fake_executable(self.root / "garbage", body="print('not json')\n"), "must print one JSON object"),
            (fake_executable(self.root / "two-lines", body="print(json.dumps(%r)); print('{}')\n" % PROBE),
             "must print one JSON object"),
            (self.old, "plain --resume suffices"),
        ]
        before = snapshot(self.campaign)
        for executable, fragment in cases:
            for start in ((), ("--start",)):
                with self.subTest(executable=executable.name, start=start):
                    status, _, errors, launch = run(self.campaign, "--resume", "--upgrade-executable",
                                                    str(executable), *start)
                    self.assertEqual(status, 2)
                    self.assertIn(fragment, errors)
                    launch.assert_not_called()
                    self.assertEqual(snapshot(self.campaign), before)
        for options, fragment in ((("--upgrade-executable", str(self.new)), "requires --resume"),
                                  (("--resume", "--upgrade-executable", str(self.new), "--prepare-from",
                                    str(self.root)), "cannot be combined with --prepare-from"),
                                  (("--resume", "--upgrade-executable", str(self.new), "--executable",
                                    str(self.old)), "replaces --executable")):
            with self.subTest(options=options):
                status, _, errors, _ = run(self.campaign, *options)
                self.assertEqual(status, 2)
                self.assertIn(fragment, errors)
        latest = self.checkpoint / "latest.json"
        for document, fragment in ((manifest(schema=4, format="RUSTRED-WALK-CP4"), "only CP5 campaigns"),
                                   (manifest(schema=5.0), "only CP5 campaigns"),
                                   (manifest(kind="final"), "kind must be state or bootstrap"),
                                   (manifest(walk_semantics_version=True), "no walk_semantics_version"),
                                   (None, "requires a saved native checkpoint")):
            with self.subTest(document=document):
                if document is None:
                    latest.unlink()
                else:
                    latest.write_text(json.dumps(document))
                changed = snapshot(self.campaign)
                status, _, errors, _ = run(self.campaign, "--resume", "--upgrade-executable", str(self.new),
                                           "--start")
                self.assertEqual(status, 2)
                self.assertIn(fragment, errors)
                self.assertEqual(snapshot(self.campaign), changed)

    def test_live_run_or_held_checkpoint_lock_is_refused_before_mutation(self):
        run_directory = self.campaign / "runs" / "live"
        run_directory.mkdir(parents=True)
        PRODUCTION.write_json(self.campaign / "active-run.json", {"run_directory": str(run_directory)})
        identity = {"supervisor": {"pid": os.getpid(), "start_ticks": start_ticks(os.getpid())},
                    "boot_id": boot_id()}
        PRODUCTION.write_json(run_directory / "processes.json", identity)
        before = snapshot(self.campaign)
        status, output, errors, _ = run(self.campaign, "--resume", "--upgrade-executable", str(self.new), "--json")
        self.assertEqual(status, 0, errors)
        self.assertEqual(len(json.loads(output)["executable_upgrade"]["active_run_evidence"]), 1)
        status, _, errors, launch = run(self.campaign, "--resume", "--upgrade-executable", str(self.new), "--start")
        self.assertEqual(status, 2)
        self.assertIn("active run is alive", errors)
        self.assertIn(f"supervisor pid {os.getpid()}", errors)
        launch.assert_not_called()
        self.assertEqual(snapshot(self.campaign), before)
        (run_directory / "processes.json").unlink()
        # A dead identity is not a live run, but a held native lock still refuses.
        PRODUCTION.write_json(run_directory / "status.json", {"process_identity": dict(
            identity, supervisor={"pid": os.getpid(), "start_ticks": start_ticks(os.getpid()) + 1})})
        (run_directory / "run.status").write_text("4\n")
        self.assertEqual(PRODUCTION.active_run_liveness(self.campaign), [])
        before = snapshot(self.campaign)
        descriptor = os.open(self.checkpoint / "checkpoint.lock", os.O_RDWR)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
            status, _, errors, launch = run(self.campaign, "--resume", "--upgrade-executable", str(self.new),
                                            "--start")
        finally:
            os.close(descriptor)
        self.assertEqual(status, 2)
        self.assertIn("checkpoint is in use", errors)
        launch.assert_not_called()
        self.assertEqual(snapshot(self.campaign), before)

    def test_liveness_evidence_sources(self):
        run_directory = self.campaign / "runs" / "starting"
        PRODUCTION.write_json(self.campaign / "active-run.json", {"run_directory": str(run_directory)})
        self.assertEqual(PRODUCTION.active_run_liveness(self.campaign), [])  # never created
        run_directory.mkdir(parents=True)
        (run_directory / "request.json").write_text(json.dumps({"supervisor_pid": os.getpid()}))
        self.assertIn("identity is not yet published", PRODUCTION.active_run_liveness(self.campaign)[0])
        me = {"pid": os.getpid(), "start_ticks": start_ticks(os.getpid())}
        PRODUCTION.write_json(run_directory / "status.json", {"process_identity": {
            "supervisor": me, "boot_id": boot_id()}})
        self.assertEqual(len(PRODUCTION.active_run_liveness(self.campaign)), 1)
        PRODUCTION.write_json(run_directory / "status.json", {"process_identity": {
            "supervisor": me, "boot_id": "another-boot"}})
        self.assertEqual(PRODUCTION.active_run_liveness(self.campaign), [])
        (run_directory / "run.pid").write_text(f"{os.getpid()}\n")
        self.assertIn("run.pid", PRODUCTION.active_run_liveness(self.campaign)[0])
        (run_directory / "supervisor-result.json").write_text("{}")
        self.assertEqual(PRODUCTION.active_run_liveness(self.campaign), [])

    def test_upgrade_rewrites_only_the_executable_and_records_history(self):
        old_bytes, old_policy = self.steering()
        old_hash, new_hash = PRODUCTION.digest(self.old), PRODUCTION.digest(self.new)
        old_frozen = self.frozen(self.old)
        started = time.time()
        status, output, errors, launch = run(self.campaign, "--resume", "--upgrade-executable", str(self.new),
                                             "--start")
        self.assertIsNone(status, errors)
        self.assertIn(f"Executable upgraded to sha256 {new_hash}", output)
        launch.assert_called_once()
        command = launch.call_args.args[1]
        self.assertEqual(command[command.index("--executable") + 1], str(self.frozen(self.new)))
        self.assertEqual(command[-2:], ["--resume", str(self.checkpoint)])
        self.assertEqual(old_frozen.stat().st_mode & 0o7777, 0o555)
        self.assertEqual(PRODUCTION.digest(old_frozen), old_hash)
        self.assertEqual(self.frozen(self.new).stat().st_mode & 0o7777, 0o555)
        steering_path = self.campaign / "bin" / "steering.json"
        self.assertEqual(steering_path.stat().st_mode & 0o7777, 0o444)
        _, policy = self.steering()
        [note] = policy["executable_upgrades"]
        self.assertEqual({key: note[key] for key in ("replaced_sha256", "sha256", "walk_semantics_version", "reason")},
                         {"replaced_sha256": old_hash, "sha256": new_hash, "walk_semantics_version": 1,
                          "reason": "upgrade_executable"})
        self.assertGreaterEqual(note["replaced_unix_time"], started)
        restored = dict(policy)
        del restored["executable_upgrades"]
        index = restored["command_arguments"].index("--executable") + 1
        self.assertEqual(restored["command_arguments"][index], str(self.frozen(self.new)))
        restored["command_arguments"] = list(restored["command_arguments"])
        restored["command_arguments"][index] = str(old_frozen)
        self.assertEqual(restored, old_policy)
        self.assertEqual((json.dumps(restored, indent=2) + "\n").encode(), old_bytes)
        receipt = json.loads((self.campaign / "bin" / "executable.json").read_text())
        self.assertEqual({key: receipt[key] for key in ("sha256", "file", "source", "walk_semantics_version")},
                         {"sha256": new_hash, "file": "rustred-" + new_hash, "source": str(self.new.resolve()),
                          "walk_semantics_version": 1})
        [previous] = receipt["history"]
        self.assertEqual(previous, {"sha256": old_hash, "file": "rustred-" + old_hash,
                                    "source": str(self.old.resolve()), "walk_semantics_version": 1,
                                    "reason": "upgrade_executable",
                                    "replaced_unix_time": note["replaced_unix_time"]})
        active = json.loads((self.campaign / "active-run.json").read_text())
        self.assertTrue(active["executable_upgrade"]["applied"])
        self.assertEqual(active["executable_sha256"], new_hash)
        self.assertEqual(PRODUCTION.freeze_executable(self.campaign, None), (self.frozen(self.new), new_hash))
        with self.assertRaisesRegex(ValueError, "different frozen executable"):
            PRODUCTION.freeze_executable(self.campaign, self.old)
        # A later plain resume keeps the upgraded executable and steering.
        status, output, errors, launch = run(self.campaign, "--resume", "--json")
        self.assertEqual(status, 0, errors)
        plan = json.loads(output)
        self.assertEqual(plan["executable_sha256"], new_hash)
        self.assertEqual(plan["steering_policy"], policy)
        self.assertEqual(plan["command"][plan["command"].index("--executable") + 1], str(self.frozen(self.new)))
        status, _, errors, launch = run(self.campaign, "--resume", "--start")
        self.assertIsNone(status, errors)
        command = launch.call_args.args[1]
        self.assertEqual(command[command.index("--executable") + 1], str(self.frozen(self.new)))
        # A second upgrade appends to both histories.
        third = fake_executable(self.root / "third-rustred")
        status, _, errors, _ = run(self.campaign, "--resume", "--upgrade-executable", str(third), "--start")
        self.assertIsNone(status, errors)
        _, policy = self.steering()
        self.assertEqual([row["sha256"] for row in policy["executable_upgrades"]],
                         [new_hash, PRODUCTION.digest(third)])
        receipt = json.loads((self.campaign / "bin" / "executable.json").read_text())
        self.assertEqual([row["sha256"] for row in receipt["history"]], [old_hash, new_hash])
        self.assertNotIn("history", receipt["history"][1])

    def test_interrupted_upgrade_is_refused_by_plain_resume_and_completed_by_rerun(self):
        original = PRODUCTION.write_json

        def failing(path, value):
            if path.name == "executable.json":
                raise OSError("simulated crash before the receipt commit")
            original(path, value)

        with patch.object(PRODUCTION, "write_json", failing):
            status, _, errors, launch = run(self.campaign, "--resume", "--upgrade-executable", str(self.new),
                                            "--start")
        self.assertEqual(status, 2)
        self.assertIn("simulated crash", errors)
        launch.assert_not_called()
        status, _, errors, _ = run(self.campaign, "--resume")
        self.assertEqual(status, 2)
        self.assertIn("interrupted --upgrade-executable", errors)
        status, output, errors, _ = run(self.campaign, "--resume", "--upgrade-executable", str(self.new), "--json")
        self.assertEqual(status, 0, errors)
        status, _, errors, launch = run(self.campaign, "--resume", "--upgrade-executable", str(self.new), "--start")
        self.assertIsNone(status, errors)
        launch.assert_called_once()
        _, policy = self.steering()
        self.assertEqual(len(policy["executable_upgrades"]), 1)
        receipt = json.loads((self.campaign / "bin" / "executable.json").read_text())
        self.assertEqual(receipt["sha256"], PRODUCTION.digest(self.new))
        self.assertEqual(len(receipt["history"]), 1)
        self.assertEqual(run(self.campaign, "--resume")[0], 0)

    def test_v1_steering_is_upgraded_in_place_and_keeps_v1_defaults(self):
        _, v2 = self.steering()
        command = [argument for argument in v2["command_arguments"]]
        for flag in ("--publication-policy", "--transfer-unreserved-lookahead"):
            del command[command.index(flag):command.index(flag) + 2]
        v1 = {"schema": "rustred.production-steering.v1",
              "options": {name: v2["options"][name] for name in (
                  "workers", "cpus", "checkpoint_interval_seconds", "max_memory_bytes",
                  "ram_guard_margin_percent", "apply_subdivision_axis", "apply_subdivision_cut")},
              "command_arguments": command}
        PRODUCTION.write_json(self.campaign / "bin" / "steering.json", v1)
        status, output, errors, _ = run(self.campaign, "--resume", "--json")
        self.assertEqual(status, 0, errors)
        self.assertEqual(json.loads(output)["publication_policy"], "ordered")
        status, _, errors, _ = run(self.campaign, "--resume", "--upgrade-executable", str(self.new), "--start")
        self.assertIsNone(status, errors)
        _, upgraded = self.steering()
        self.assertEqual(upgraded["schema"], "rustred.production-steering.v1")
        self.assertEqual(PRODUCTION.frozen_options(upgraded), PRODUCTION.frozen_options(v1))
        status, output, errors, _ = run(self.campaign, "--resume", "--json")
        self.assertEqual(status, 0, errors)
        plan = json.loads(output)
        self.assertEqual((plan["publication_policy"], plan["transfer_unreserved_lookahead"]), ("ordered", 256))
        expected = list(command)
        expected[expected.index("--executable") + 1] = str(self.frozen(self.new))
        self.assertEqual(plan["command"][2:-4], expected)

    def test_probe_output_and_time_are_bounded(self):
        noisy = fake_executable(self.root / "noisy", body="sys.stdout.write('x' * 200000)\n")
        with self.assertRaisesRegex(ValueError, "exceeds 65536 bytes"):
            PRODUCTION.probe_walk_semantics(noisy)
        slow = fake_executable(self.root / "slow", body="time.sleep(30)\n")
        started = time.monotonic()
        with patch.object(PRODUCTION, "PROBE_TIMEOUT_SECONDS", 0.5), \
                self.assertRaisesRegex(ValueError, "did not finish within 0.5 s"):
            PRODUCTION.probe_walk_semantics(slow)
        self.assertLess(time.monotonic() - started, 10)
        self.assertEqual(PRODUCTION.probe_walk_semantics(self.new), PROBE)


if __name__ == "__main__":
    unittest.main()
