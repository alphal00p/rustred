"""Scratch-only phase transitions; no production runs or algebra in Python."""
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import signal
import sys
import tempfile
import threading
import unittest
from types import SimpleNamespace
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


PHASES = module("campaign_phases")
PRODUCTION = module("production_saved_owner_campaign")
TELEMETRY = module("campaign_telemetry")
DASHBOARD = module("campaign_dashboard")
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
ROOT = Path(__file__).resolve().parents[2]


class PhaseTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "TMP").mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix="campaign-phases-test-", dir=ROOT / "TMP")
        self.addCleanup(self.temp.cleanup)
        self.campaign = Path(self.temp.name)
        self.checkpoint = self.campaign / "checkpoints/main"
        for name in ("inputs", "checkpoints/main", "runs/first", "amendments"):
            (self.campaign / name).mkdir(parents=True, exist_ok=True)
        self.write("inputs/selection.json", {"owners": []})
        self.write("inputs/queries.json", {"queries": ["original"]})
        self.write("checkpoints/main/latest.json", {"manifest": {
            "format": "RUSTRED-WALK-CP6", "generation": 1, "resumable": True}, "blake3": [0] * 32})
        self.command = ["rustred", "owner-domain-match", "--queries", str(self.campaign / "inputs/queries.json"),
                        "--manifest", str(self.campaign / "inputs/selection.json")]
        self.write("runs/first/request.json", {"command": self.command})
        self.result = {"recursive_worklist_exhausted": True, "admission_complete": True,
                       "admission_failure": None, "failed_nodes": 0, "frontiers": 0,
                       "checkpoint": {"state": "saved", "generation": 1, "resumable": True,
                                      "directory": str(self.checkpoint), "manifest_blake3": "00" * 32}}
        self.write("runs/first/result.json", self.result)
        self.driver = SimpleNamespace(campaign_runs=lambda _: [self.campaign / "runs/first"],
                                      campaign_run_liveness=lambda _: [], write_json=PRODUCTION.write_json,
                                      checkpoint_lock=PRODUCTION.checkpoint_lock)
        self.plan = {"campaign_directory": str(self.campaign), "checkpoint_directory": str(self.checkpoint),
                     "run_directory": str(self.campaign / "runs/next"), "command": ["supervisor"]}
        self.policy = PHASES.configuration(self.campaign)

    def write(self, name, value):
        path = self.campaign / name
        path.parent.mkdir(parents=True, exist_ok=True)
        PRODUCTION.write_json(path, value)

    def binding(self):
        return PHASES.scope_binding(self.campaign, self.checkpoint)

    def ready(self):
        return PHASES.completed_request(self.campaign, self.checkpoint, self.binding(), self.driver)

    def test_default_publishes_and_legacy_opt_in_never_automatically_refines(self):
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        self.write("master-reduction/policy.json", {**self.policy, "operation": "refine"})
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")
        deeper = PHASES.configuration(self.campaign, enabled=True, seed_depth=1)
        self.assertEqual(deeper["seed_depth"], 1)
        self.assertEqual(deeper["operation"], "refine")
        self.assertNotEqual(self.binding()["key"], PHASES.scope_binding(self.campaign, self.checkpoint, 1, "refine")["key"])
        self.write("master-reduction/policy.json", deeper)
        with self.assertRaisesRegex(ValueError, "cannot lower"):
            PHASES.configuration(self.campaign, enabled=True, seed_depth=0)
        with self.assertRaisesRegex(ValueError, "explicit"):
            PHASES.configuration(self.campaign, seed_depth=1)
        self.assertEqual(PHASES.configuration(self.campaign)["operation"], "publish")

    def test_separate_native_executable_freezes_without_changing_phase_one(self):
        binary = self.campaign / "new-native"
        binary.write_bytes(b"#!/bin/sh\nexit 0\n")
        binary.chmod(0o700)
        policy = PHASES.configuration(self.campaign, True, executable=binary)
        self.assertFalse((self.campaign / "master-reduction").exists())
        PHASES.freeze_master_executable(self.campaign / "master-reduction", policy, PRODUCTION)
        frozen = self.campaign / "master-reduction" / policy["executable"]["path"]
        self.assertEqual(frozen.read_bytes(), binary.read_bytes())
        self.assertFalse((self.campaign / "bin").exists())
        self.write("master-reduction/policy.json", policy)
        binary.write_bytes(b"#!/bin/sh\nexit 1\n")
        self.assertEqual(PHASES.configuration(self.campaign)["executable"], policy["executable"])
        explicit = PHASES.configuration(self.campaign, executable=binary)
        self.assertNotEqual(explicit["executable"]["sha256"], policy["executable"]["sha256"])
        PHASES.freeze_master_executable(self.campaign / "master-reduction", explicit, PRODUCTION)
        self.assertTrue(frozen.is_file())

    def test_drained_receipt_bound_to_exact_checkpoint_not_cached_rootbar(self):
        self.assertIsNotNone(self.ready())
        self.result["recursive_worklist_exhausted"] = False
        self.result["descendant_closure"] = {"initial_closed": 67, "initial_total": 67}
        self.write("runs/first/result.json", self.result)
        self.assertIsNone(self.ready())
        self.result["recursive_worklist_exhausted"] = True
        self.result["checkpoint"]["manifest_blake3"] = "11" * 32
        self.write("runs/first/result.json", self.result)
        self.assertIsNone(self.ready())

    def test_new_rank_or_D_amendment_changes_binding_and_requires_phase_one(self):
        original = self.binding()["key"]
        self.write("amendments/amendment-0001.json", {"rank": 1, "max_D": 9})
        self.assertNotEqual(original, self.binding()["key"])
        self.assertIsNone(self.ready())
        rank = self.binding()["key"]
        self.write("amendments/amendment-0001.json", {"rank": 1, "max_D": 10})
        self.assertNotEqual(rank, self.binding()["key"])
        self.command += ["--amend-queries", str(self.campaign / "amendments/amendment-0001.json")]
        self.write("runs/first/request.json", {"command": self.command})
        self.assertIsNotNone(self.ready())

    def test_resume_goes_directly_back_to_master_reduction_for_same_scope(self):
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as second:
            self.assertEqual(PHASES.run(self.plan, self.policy, True, self.driver), 4)
        first.assert_not_called()
        self.assertEqual(second.call_args.args[3]["key"], self.binding()["key"])
        self.assertFalse((self.campaign / "active-run.json").exists())

    def test_pending_scope_runs_phase_one_and_does_not_start_old_master_scope(self):
        self.write("amendments/amendment-0001.json", {"rank": 1})
        with patch.object(PHASES, "phase_one", return_value=(4, False)) as first, patch.object(PHASES, "phase_two") as second:
            self.assertEqual(PHASES.run(self.plan, self.policy, True, self.driver), 4)
        first.assert_called_once()
        second.assert_not_called()

    def test_postprocess_only_never_starts_solve_when_incomplete(self):
        self.result["recursive_worklist_exhausted"] = False
        self.write("runs/first/result.json", self.result)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two") as second:
            with self.assertRaisesRegex(ValueError, "postprocess-only"):
                PHASES.run(self.plan, self.policy, True, self.driver, postprocess_only=True)
        first.assert_not_called()
        second.assert_not_called()

    def publish_fixture(self, operation="publish", depth=0):
        path = self.campaign / "master-reduction/scopes/published"
        self.write("master-reduction/scopes/published/artifact.json", {"status": "published_unrefined"})
        PHASES.publish_pointer(self.campaign, path, self.binding(), operation, self.driver,
                               {"seed_depth": depth} if operation == "refine" else None)
        return path

    def test_refine_is_explicit_and_reads_published_artifact_without_walking(self):
        source = self.publish_fixture()
        policy = PHASES.configuration(self.campaign, enabled=True)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two", return_value=4) as second:
            self.assertEqual(PHASES.run(self.plan, policy, True, self.driver, postprocess_only=True), 4)
        first.assert_not_called()
        self.assertEqual(second.call_args.args[1]["source_artifact"], str(source))
        pointer = PHASES.read_json(self.campaign / "artifacts/latest.json")
        self.assertEqual(pointer["operation"], "publish")  # Paused refinement preserves usable output.
        self.assertFalse(Path(pointer["directory"]).is_absolute())

    def test_default_resume_preserves_same_scope_refined_artifact_without_rerun(self):
        self.publish_fixture("refine", 1)
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two") as second:
            self.assertEqual(PHASES.run(self.plan, self.policy, True, self.driver), 0)
        first.assert_not_called()
        second.assert_not_called()
        self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["operation"], "refine")

    def test_extension_prevents_refining_old_scope(self):
        self.publish_fixture()
        self.write("amendments/amendment-0001.json", {"rank": 1})
        with patch.object(PHASES, "phase_one") as first, patch.object(PHASES, "phase_two") as second:
            with self.assertRaisesRegex(ValueError, "current scope"):
                PHASES.run(self.plan, PHASES.configuration(self.campaign, enabled=True), True,
                           self.driver, postprocess_only=True)
        first.assert_not_called()
        second.assert_not_called()

    def test_interruption_at_phase_one_boundary_does_not_begin_phase_two(self):
        for native_status in (0, 4):
            with patch.object(PHASES, "phase_one", return_value=(native_status, True)), patch.object(PHASES, "phase_two") as second:
                self.assertEqual(PHASES.run(self.plan, self.policy, False, self.driver), 4)
            second.assert_not_called()

    def test_dispatcher_lock_rejects_parallel_launch(self):
        target = self.campaign / "master-reduction"
        with PHASES.dispatcher_lock(target):
            with self.assertRaisesRegex(ValueError, "running phase dispatcher"):
                with PHASES.dispatcher_lock(target):
                    self.fail("second dispatcher admitted")

    def test_initial_dispatcher_creation_is_serialized_with_scope_extension(self):
        (self.checkpoint / "checkpoint.lock").touch()
        with PRODUCTION.checkpoint_lock(self.checkpoint):
            with self.assertRaisesRegex(ValueError, "in use"):
                PHASES.run(self.plan, self.policy, True, self.driver)
        self.assertFalse((self.campaign / "master-reduction/dispatcher.lock").exists())

    def test_phase_one_ctrl_c_forwards_to_only_its_owned_supervisor(self):
        child = unittest.mock.Mock(pid=456)
        child.poll.return_value = None
        installed = {}
        def install(sig, handler):
            previous = installed.get(sig)
            installed[sig] = handler
            return previous
        def wait():
            installed[signal.SIGINT](signal.SIGINT, None)
            return 4
        child.wait.side_effect = wait
        with patch.object(PHASES.subprocess, "Popen", return_value=child), patch.object(PHASES.signal, "signal", side_effect=install), patch.object(PHASES.os, "killpg") as send:
            self.assertEqual(PHASES.phase_one(["owned-supervisor"]), (4, True))
        send.assert_called_once_with(456, signal.SIGINT)

    def test_interrupt_during_spawn_is_forwarded_after_child_exists(self):
        child = unittest.mock.Mock(pid=456)
        child.poll.return_value = None
        child.wait.return_value = 4
        installed = {}
        def install(sig, handler):
            previous = installed.get(sig)
            installed[sig] = handler
            return previous
        def spawn(*args, **kwargs):
            installed[signal.SIGINT](signal.SIGINT, None)
            return child
        with patch.object(PHASES.subprocess, "Popen", side_effect=spawn), patch.object(PHASES.signal, "signal", side_effect=install), patch.object(PHASES.os, "killpg") as send:
            self.assertEqual(PHASES.phase_one(["owned-supervisor"]), (4, True))
        send.assert_called_once_with(456, signal.SIGINT)

    def test_real_scratch_child_saves_on_sigint_and_resume_returns_to_phase_two(self):
        self._scratch_native_pause_resume("publish")

    def test_explicit_refinement_saves_and_resumes_without_replacing_publication_on_pause(self):
        self._scratch_native_pause_resume("refine")

    def _scratch_native_pause_resume(self, operation):
        # Fake native protocol only; this proves process/checkpoint orchestration,
        # not mathematical correctness or a successful native Rust solve.
        binary = self.campaign / "fake-native"
        binary.write_text(f"#!{sys.executable}\n" + '''import json, pathlib, sys, time
args = sys.argv
refine = args[1] == 'walk-master-reduce'
assert ('--seed-depth' in args) == refine
assert ('--artifact' in args) == refine
assert ('--command' in args) != refine
directory = pathlib.Path(args[args.index('--directory') + 1])
events = pathlib.Path(args[args.index('--events') + 1])
stop = pathlib.Path(args[args.index('--stop-file') + 1])
assert all(__import__('os').environ[x] == '1' for x in ['RAYON_NUM_THREADS','OMP_NUM_THREADS'])
def event(status):
    with events.open('a') as out:
        out.write(json.dumps({'event':'master_reduction_progress','phase':'Master reduction',
            'stage':'elimination','status':status,'remaining_terminals':3,'artifact':str(directory),
            'checkpoint':{'state':'saved','generation':1,'directory':str(directory),'bytes':20}})+'\\n')
if '--resume' not in args:
    while not stop.exists(): time.sleep(.01)
    (directory/'latest.json').write_text('{}')
    event('paused')
    sys.exit(4)
(directory/'artifact.json').write_text('{}')
event('completed_nonminimal' if refine else 'published_unrefined')
''')
        binary.chmod(0o700)
        cpus = sorted(os.sched_getaffinity(0))[:1]
        policy = {"command_arguments": ["--executable", str(binary)]}
        plan = {**self.plan, "steering_policy": policy, "requested_workers": 1, "cpus": str(cpus[0]),
                "checkpoint_interval_seconds": 3600,
                "supervisor_ram_policy": {"max_memory_bytes": 2_000_000_000,
                    "ram_guard_margin_percent": 5, "host_memory_reserve_bytes": 1_000_000,
                    "swap_growth_stop_bytes_per_second": 0, "swap_growth_stop_seconds": 120}}
        driver = SimpleNamespace(**vars(PRODUCTION))
        native_policy = {**self.policy, "operation": operation}
        if operation == "refine":
            native_policy["source_artifact"] = str(self.publish_fixture())
        binding = self.binding()
        directory = self.campaign / "master-reduction/scopes" / binding["key"]
        directory.mkdir(parents=True)
        timer = threading.Timer(.4, lambda: os.kill(os.getpid(), signal.SIGINT))
        with patch.object(PHASES.sys, "stderr", io.StringIO()), patch.object(PHASES.sys, "stdout", io.StringIO()):
            timer.start()
            try:
                self.assertEqual(PHASES.phase_two(plan, native_policy, self.campaign / "runs/first/request.json", binding, directory, driver), 4)
            finally:
                timer.cancel()
                timer.join()
            self.assertTrue((directory / "latest.json").exists())
            if operation == "refine":
                self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["status"], "published_unrefined")
            self.assertEqual(PHASES.phase_two(plan, native_policy, self.campaign / "runs/first/request.json", binding, directory, driver), 0)
        self.assertTrue((directory / "artifact.json").exists())
        pointer = PHASES.read_json(self.campaign / "master-reduction/active-phase.json")
        observer = PRODUCTION.SUPERVISOR.MONITOR
        self.assertEqual(observer.active_status_directory(self.campaign), Path(pointer["run_directory"]))
        status = observer.read_status(self.campaign)
        expected = "published_unrefined" if operation == "publish" else "completed_nonminimal"
        self.assertEqual(status["state"], expected)
        self.assertEqual(status["master_reduction"]["remaining_terminals"], 3)
        self.assertEqual(PHASES.read_json(self.campaign / "artifacts/latest.json")["status"], expected)


class MasterDashboardTests(unittest.TestCase):
    def frame(self):
        return TELEMETRY.normalize_status({
            "state": "running", "elapsed_seconds": 150, "workers": 32,
            "hard_memory_bytes": 600_000_000_000, "soft_memory_bytes": 570_000_000_000,
            "resources": {"native_busy_cores": 25.3, "aggregate_rss_bytes": 1_230_000_000},
            "run_directory": "/campaign/master-reduction/scopes/abc/runs/test",
            "checkpoint": {"state": "saved", "generation": 2, "bytes": 12_345,
                           "directory": "/campaign/master-reduction/scopes/abc"},
            "master_reduction": {"stage": "elimination", "raw_terminals": 116,
                "normalized_terminals": 74, "remaining_terminals": 49,
                "eliminated_terminals": 25, "relation_rows": 760,
                "completed_work": 70, "total_work": 100, "seed_depth": 1,
                "scope_binding": "0123456789abcdef"}})

    def test_colored_tables_fit_small_and_large_terminal_and_keep_scientific_scope(self):
        frame = self.frame()
        for width, height in ((80, 24), (150, 40), (35, 12)):
            rows = DASHBOARD.render_table(frame, width, height, True)
            self.assertLessEqual(len(rows), height)
            self.assertTrue(all(DASHBOARD.cell_width(ANSI.sub("", row)) == width for row in rows))
            self.assertIn("\x1b[", "\n".join(rows))
        shown = "\n".join(DASHBOARD.render_table(frame, 80, 24, False))
        self.assertIn("MASTER REFINEMENT", shown)
        self.assertIn("nonminimal", shown)
        self.assertNotIn("ROOT CLOSURE", shown)
        self.assertNotIn("pending", shown)

    def test_non_tty_is_structured_json_not_ansi_and_unknown_counts_are_not_zero(self):
        frame = self.frame()
        frame["master_reduction"]["remaining_terminals"] = None
        stream = io.StringIO()
        DASHBOARD.Presenter(stream=stream).render_frame(frame, now=0, force=True)
        value = json.loads(stream.getvalue())
        self.assertEqual(value["telemetry"]["phase"], "Master refinement")
        self.assertIsNone(value["telemetry"]["master_reduction"]["remaining_terminals"])
        self.assertNotIn("\x1b", stream.getvalue())
        self.assertFalse(value["telemetry"]["master_reduction"]["master_minimality_claim"])

    def test_publication_has_its_own_coverage_display_and_no_relation_search(self):
        frame = self.frame()
        frame["master_reduction"]["operation"] = "publish"
        frame["state"] = "published_unrefined"
        for width, height in ((80, 24), (150, 32), (35, 12)):
            rows = DASHBOARD.render_table(frame, width, height, True)
            self.assertLessEqual(len(rows), height)
            self.assertTrue(all(DASHBOARD.cell_width(ANSI.sub("", row)) == width for row in rows))
        shown = "\n".join(DASHBOARD.render_table(frame, 80, 24, False))
        self.assertIn("ARTIFACT PUBLICATION", shown)
        self.assertIn("verified", shown)
        self.assertIn("not requested", shown)
        self.assertNotIn("ROOT CLOSURE", shown)
        self.assertNotIn("Relation work", shown)


if __name__ == "__main__":
    unittest.main()
