"""Fast bounded monitoring/staging tests; no native algebra or real campaign."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


MONITOR = module("campaign_monitor")
PRODUCTION = module("production_saved_owner_campaign")
STAGE = module("stage_saved_owner_campaign")


class MonitorTests(unittest.TestCase):
    def test_checkpoint_milestones_survive_later_heartbeat_and_plain_throttle(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "events"
            stream = io.StringIO()
            presenter = MONITOR.Presenter(stream, plain_seconds=30)
            presenter.render({"state": "running"}, now=0)
            saved = {"state": "saved", "generation": 3, "state_path": "/checkpoint/state-3",
                     "duration_seconds": 1.25, "saved_unix_time": 1790208000}
            started = {"event": "checkpoint_started", "checkpoint_write": saved}
            finished = {"event": "checkpoint_saved", "checkpoint": saved}
            path.write_text("\n".join(map(json.dumps, [started, finished,
                {"event": "heartbeat", "progress": finished},
                {"event": "heartbeat", "progress": {"event": "domain_progress", "checkpoint": saved}}])) + "\n")
            tail = MONITOR.EventTail(path)
            self.assertEqual(tail.poll(1)["event"], "heartbeat")
            self.assertEqual(tail.milestone_count, 2)
            self.assertEqual(tail.saved_checkpoint["generation"], 3)
            self.assertIsNone(tail.checkpoint_write)
            status = {"state": "running", "checkpoint_milestones": list(tail.milestones)}
            presenter.render(status, now=1)
            presenter.render(status, now=2)
            self.assertEqual(stream.getvalue().count("checkpoint started"), 1)
            self.assertEqual(stream.getvalue().count("checkpoint saved"), 1)
            self.assertIn("1.25s", stream.getvalue())
            self.assertIn("UTC", stream.getvalue())
            self.assertIn("/checkpoint/state-3", stream.getvalue())
            tail._checkpoint_milestone({"event": "checkpoint_saved", "checkpoint": {
                "generation": 2, "state_path": "/checkpoint/state-2"}})
            self.assertEqual(tail.saved_checkpoint["generation"], 3)
            for generation in range(4, 104):
                tail._checkpoint_milestone({"event": "checkpoint_saved", "checkpoint": {
                    "generation": generation, "state_path": f"/checkpoint/state-{generation}"}})
            self.assertEqual(len(tail.milestones), 64)
            self.assertEqual(tail.diagnostics()["checkpoint_milestones_history_dropped"], 39)

    def test_partial_malformed_and_oversize_jsonl_are_bounded_and_visible(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "events"
            tail = MONITOR.EventTail(path)
            self.assertEqual(tail.poll(0), {})
            path.write_bytes(b'{"event":"hel')
            self.assertEqual(tail.poll(1), {})
            with path.open("ab") as stream:
                stream.write(b'lo"}\nnot json\n' + b'x' * (MONITOR.MAX_RECORD_BYTES + 1) + b'\n{"event":"last"}\n')
            self.assertEqual(tail.poll(2), {"event": "last"})
            self.assertEqual(tail.invalid_records, 1)
            self.assertEqual(tail.oversized_records, 1)
            self.assertLessEqual(len(tail.pending), MONITOR.MAX_RECORD_BYTES)
            self.assertEqual(tail.observed_at, 2)

    def test_event_rotation_and_truncation_reset_offsets(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "events"
            path.write_text('{"event":"long event"}\n')
            tail = MONITOR.EventTail(path)
            tail.poll(0)
            path.write_text('{"a":1}\n')
            self.assertEqual(tail.poll(1), {"a": 1})
            path.rename(path.with_suffix(".old"))
            path.write_text('{"b":2}\n')
            self.assertEqual(tail.poll(2), {"b": 2})
            self.assertEqual(tail.rotations, 2)

    def test_no_entry_denominator_or_closure_eta_is_invented(self):
        event = {"progress_age_seconds": 2, "progress": {"completed_nodes": 100,
                 "scheduled_nodes": 200, "queued_nodes": 100}}
        result = MONITOR.progress_summary(event, 10, 15)
        self.assertIsNone(result["initial_entry_progress"]["total"])
        self.assertIsNone(result["initial_entry_progress"]["locally_inspected"])
        self.assertIsNone(result["work"]["pending_descendants"])
        self.assertEqual(result["progress_age_seconds"], 7)
        self.assertIsNone(result["closure_eta_seconds"])

    def test_publication_bar_keeps_native_inspection_and_reserved_workers_distinct(self):
        event = {"progress": {"initial_entry_domains_total": 20,
                 "initial_entry_domains_published": 15, "initial_entry_domains_inspected": 10,
                 "pending_descendant_domains": 99, "parallel": {"active_workers": 2,
                 "backpressured_workers": 1, "admission_preparation": {
                     "inspection_worker_limit": 25, "lookup_worker_limit": 24, "coordinator_worker_limit": 1}}}}
        progress = MONITOR.progress_summary(event, 0, 1)
        text = "\n".join(MONITOR.dashboard({"progress": progress, "workers": 50,
                    "resources": {"native_busy_cores": 1.4}, "hard_memory_bytes": 500_000_000_000}))
        self.assertIn("15 / 20 published", text)
        self.assertIn("initial native inspected 10", text)
        self.assertIn("2 native active, 1 blocked", text)
        self.assertIn("25 inspect + 24 admission + 1 coordinator", text)
        self.assertIn("1.4 observed cores", text)
        self.assertIn("not closure", text)

    def test_plain_output_retains_checkpoint_and_never_contains_escape_codes(self):
        stream = io.StringIO()
        presenter = MONITOR.Presenter(stream, plain_seconds=30)
        status = {"state": "running", "checkpoint": {"state": "saved", "generation": 3,
                   "directory": "/run/checkpoint"}, "run_directory": "bad\x1b[31m\ntext"}
        presenter.render(status, now=0)
        presenter.render(status, now=1)
        self.assertEqual(len(stream.getvalue().splitlines()), 1)
        self.assertIn("generation 3", stream.getvalue())
        self.assertIn("/run/checkpoint", stream.getvalue())
        self.assertNotIn("\x1b", stream.getvalue())
        self.assertIn("closure ETA unknown", stream.getvalue())

    def test_in_progress_save_does_not_replace_last_good_checkpoint(self):
        saved = {"state": "saved", "generation": 3, "directory": "/checkpoint",
                 "saved_unix_time": 1790208000, "duration_seconds": 1.25}
        writing = {"state": "writing", "generation": 4, "state_path": "/checkpoint/state-4"}
        progress = MONITOR.progress_summary({"progress": {"checkpoint": saved,
                    "checkpoint_write": writing}}, 0, 1)
        self.assertEqual(progress["checkpoint"]["generation"], 3)
        self.assertEqual(progress["checkpoint_write"]["generation"], 4)
        text = "\n".join(MONITOR.dashboard({"progress": progress}))
        self.assertIn("WRITING generation 4", text)
        self.assertIn("saved generation 3", text)
        self.assertIn("1.25s", text)
        self.assertIn("UTC", text)
        terminal = "\n".join(MONITOR.dashboard({"progress": progress, "checkpoint_write": None}))
        self.assertNotIn("WRITING", terminal)
        self.assertIn("saved generation 3", terminal)

    def test_tty_has_colors_and_no_color_disables_only_color(self):
        class Terminal(io.StringIO):
            def isatty(self):
                return True
        for no_color in (False, True):
            stream = Terminal()
            environment = {"TERM": "xterm"}
            if no_color:
                environment["NO_COLOR"] = "1"
            with patch.dict(os.environ, environment, clear=True):
                MONITOR.Presenter(stream).render({"state": "running"}, now=0)
            self.assertEqual("\x1b[1;36m" in stream.getvalue(), not no_color)
            self.assertIn("\x1b[2K", stream.getvalue())

    def test_atomic_status_and_stale_or_reused_pid_observation(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            status = {"heartbeat_unix_time": 1, "sample_seconds": 2,
                      "process_identity": {"native": {"pid": os.getpid(), "start_ticks": -1}}}
            MONITOR.atomic_json(directory / "status.json", status)
            self.assertEqual(json.loads((directory / "status.json").read_text()), status)
            read = MONITOR.read_status(directory)
            self.assertTrue(read["heartbeat_stale"])
            self.assertFalse(read["observed_processes_alive"]["native"])
            self.assertEqual([path.name for path in directory.iterdir()], ["status.json"])

    def test_stale_reader_checks_boot_and_shows_advancing_heartbeat_age(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            stat = Path(f"/proc/{os.getpid()}/stat").read_text()
            start = int(stat[stat.rfind(")") + 2:].split()[19])
            identity = {"pid": os.getpid(), "start_ticks": start}
            status = {"state": "running", "heartbeat_unix_time": time.time() - 20,
                      "progress": {"progress_age_seconds": 3},
                      "process_identity": {"boot_id": "previous boot", "supervisor": identity}}
            MONITOR.atomic_json(directory / "status.json", status)
            read = MONITOR.read_status(directory)
            self.assertFalse(read["observed_boot_id_matches"])
            self.assertFalse(read["observed_processes_alive"]["supervisor"])
            self.assertGreaterEqual(read["progress"]["progress_age_seconds"], 23)
            text = "\n".join(MONITOR.dashboard(read))
            self.assertIn("LAST REPORTED RUNNING", text)
            self.assertIn("STALE HEARTBEAT", text)
            self.assertIn("heartbeat age 00:00:20", text)
            status["process_identity"]["boot_id"] = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
            status["heartbeat_unix_time"] = time.time()
            MONITOR.atomic_json(directory / "status.json", status)
            self.assertTrue(MONITOR.read_status(directory)["observed_processes_alive"]["supervisor"])


class ProductionTests(unittest.TestCase):
    def test_policy_publication_syncs_file_before_rename_and_directory_after(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "policy.json"
            events = []
            replace = PRODUCTION.os.replace
            with patch.object(PRODUCTION.os, "fsync", side_effect=lambda _fd: events.append("file_sync")), \
                    patch.object(PRODUCTION.os, "replace", side_effect=lambda src, dst: (events.append("rename"), replace(src, dst))), \
                    patch.object(PRODUCTION, "sync_directory", side_effect=lambda _path: events.append("directory_sync")):
                PRODUCTION.write_json(path, {"frozen": True})
            self.assertEqual(events, ["file_sync", "rename", "directory_sync"])
            self.assertEqual(json.loads(path.read_text()), {"frozen": True})
            with patch.object(PRODUCTION.os, "fsync", side_effect=OSError("sync failure")):
                with self.assertRaisesRegex(OSError, "sync failure"):
                    PRODUCTION.write_json(path, {"frozen": False})
            self.assertEqual(json.loads(path.read_text()), {"frozen": True})
            self.assertEqual(list(path.parent.iterdir()), [path])

    def test_staging_preserves_query_and_payload_bytes_then_freezes_executable(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            owner = root / "owner.rrbin"
            owner.write_bytes(b"fixture payload, not native data")
            selection = {"family_fingerprint": "fixture", "owners": [
                {"mask": "01", "path": str(owner), "bytes": owner.stat().st_size}],
                "initial_frontier_routes": [], "load_limits": {"max_total_input_bytes": 1234}}
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps(selection))
            queries = root / "queries.json"
            queries.write_text('{"schema":"rustred.owner-domain-queries.json.v2", "queries":[{"id":"unchanged"}]}\n')
            campaign = root / "campaign"
            staged = campaign / "inputs"
            receipt = STAGE.stage(manifest, queries, staged, root)
            self.assertEqual((staged / "queries.json").read_bytes(), queries.read_bytes())
            copy = json.loads((staged / "selection.json").read_text())
            copy["owners"][0]["path"] = selection["owners"][0]["path"]
            self.assertEqual(copy, selection)
            self.assertEqual(PRODUCTION.verify_inputs(staged)[:2], (1, queries.stat().st_size))
            executable = root / "fake-rustred"
            executable.write_text("#!/bin/sh\nexit 0\n")
            executable.chmod(0o700)
            with patch.object(PRODUCTION.os, "fsync", wraps=os.fsync) as synced:
                frozen, frozen_hash = PRODUCTION.freeze_executable(campaign, executable)
            self.assertGreaterEqual(synced.call_count, 5)
            self.assertEqual(PRODUCTION.freeze_executable(campaign, None), (frozen, frozen_hash))
            executable.write_text("#!/bin/sh\nexit 1\n")
            with self.assertRaisesRegex(ValueError, "different frozen executable"):
                PRODUCTION.freeze_executable(campaign, executable)
            self.assertEqual(PRODUCTION.digest(frozen), frozen_hash)
            output = io.StringIO()
            with patch("sys.stdout", output), patch.object(PRODUCTION.os, "execv") as launch:
                self.assertEqual(PRODUCTION.main(["--campaign-directory", str(campaign), "--json"]), 0)
            launch.assert_not_called()
            plan = json.loads(output.getvalue())
            self.assertIn("--unbounded-work", plan["command"])
            self.assertNotIn("375000", plan["command"])
            self.assertNotIn("--child-address-space-bytes", plan["command"])
            self.assertFalse(plan["launch_requested"])
            self.assertEqual(plan["queries_sha256"], receipt["queries_sha256"])
            self.assertEqual(plan["checkpoint_interval_seconds"], 3600)
            self.assertEqual(plan["ram_guard_margin_percent"], 5)
            self.assertNotIn("--soft-memory-bytes", plan["command"])

    def test_resume_reuses_exact_frozen_policy_not_new_defaults(self):
        from argparse import Namespace
        with tempfile.TemporaryDirectory() as temporary:
            campaign = Path(temporary)
            (campaign / "bin").mkdir()
            cpu = min(os.sched_getaffinity(0))
            args = Namespace(workers=1, cpus=str(cpu), checkpoint_interval_seconds=1234,
                             max_memory_bytes=10_000_000_000, ram_guard_margin_percent=7,
                             apply_subdivision_axis=2, apply_subdivision_cut=3, resume=False)
            policy = PRODUCTION.frozen_policy(campaign, args, campaign / "bin/rustred",
                                               campaign / "inputs", 67, 123456)
            args = Namespace(**{name: None for name in policy["options"]}, resume=True)
            with patch.object(PRODUCTION.os, "sched_getaffinity", return_value={cpu + 1}):
                resumed = PRODUCTION.frozen_policy(campaign, args, campaign / "ignored",
                                                    campaign / "ignored", 1, 2)
            self.assertEqual(resumed, policy)
            self.assertIn("--apply-subdivision-axis", resumed["command_arguments"])
            self.assertEqual(resumed["options"]["workers"], 1)
            self.assertEqual(resumed["options"]["cpus"], str(cpu))
            args.apply_subdivision_axis = 5
            with self.assertRaisesRegex(ValueError, "differs from frozen policy"):
                PRODUCTION.frozen_policy(campaign, args, campaign / "ignored", campaign, 1, 2)


if __name__ == "__main__":
    unittest.main()
