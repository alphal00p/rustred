"""Pure mocked scheduler/resource tests; no native CAS or solver launch."""
from contextlib import ExitStack
import json
import os
from pathlib import Path
import signal
import tempfile
import types
import unittest
from unittest.mock import patch

import generation_scheduler as scheduler
import generation_state as state


class SchedulerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=scheduler.ROOT / "TMP")
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)
        self.resources = dict(workers=4, cpus="64-67", generation_jobs=2,
            max_memory_bytes=1_000_000_000, host_memory_reserve_bytes=500_000_000,
            ram_guard_margin_percent=5)
        self.tick, self.processes, self.signals, self.launches = 0, {}, [], []
        self.rss_each = 10_000_000
        self.failure = None
        self.stack = self.enterContext(ExitStack())
        self.stack.enter_context(patch.object(scheduler.os, "sched_getaffinity", return_value={64,65,66,67}))
        self.stack.enter_context(patch.object(scheduler.SUPERVISOR, "host_memory", return_value={"available_bytes":20_000_000_000}))
        self.stack.enter_context(patch.object(scheduler.SUPERVISOR, "tree_swap_bytes", return_value=0))
        self.stack.enter_context(patch.object(scheduler.time, "sleep"))
        self.stack.enter_context(patch.object(scheduler.subprocess, "Popen", side_effect=self.spawn))
        self.stack.enter_context(patch.object(scheduler, "leader_status", side_effect=lambda p: p.code if self.tick >= p.end else None))
        self.stack.enter_context(patch.object(scheduler, "group_running", side_effect=lambda pid: self.tick < self.processes[pid].end))
        self.stack.enter_context(patch.object(scheduler.os, "killpg", side_effect=self.kill))
        collector = types.SimpleNamespace(identities={}, register=lambda pid: collector.identities.update({pid:pid+100}), sample=self.sample)
        self.stack.enter_context(patch.object(scheduler.SUPERVISOR, "ProcessTreeCollector", return_value=collector))

    def spawn(self, command, **options):
        pid = 1000000 + len(self.processes)
        self.launches.append((command[0], options["preexec_fn"].__defaults__[0]))
        process = types.SimpleNamespace(pid=pid, end=self.tick+int(command[1]),
            code=1 if command[0] == self.failure else 0, reaped=False)
        def wait():
            self.assertGreaterEqual(self.tick, process.end)
            process.reaped = True
            return process.code
        process.wait = wait
        self.processes[pid] = process
        return process

    def kill(self, pid, sig):
        process = self.processes[pid]
        self.assertFalse(process.reaped, "must never signal a reaped/reusable leader PID")
        self.signals.append((pid, sig))
        process.end = self.tick
        process.code = -sig

    def sample(self):
        self.tick += 1
        rows = {os.getpid(): dict(rss_bytes=1024, start=10, ppid=0, pgrp=0, cpu_seconds=0)}
        for pid, process in self.processes.items():
            rows[pid] = dict(rss_bytes=self.rss_each if self.tick < process.end else 0,
                start=pid+100, ppid=os.getpid(), pgrp=pid, cpu_seconds=self.tick)
        return rows, {}

    def jobs(self):
        return [scheduler.Job("A", ("A", "5"), {}), scheduler.Job("B", ("B", "2"), {}),
                scheduler.Job("C", ("C", "2"), {})]

    def test_slots_refill_disjointly_with_one_aggregate_guard(self):
        completed = []
        def complete(job):
            self.assertTrue(all(process.reaped for process in self.processes.values()))
            completed.append(job.id)
        previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
        result = scheduler.run(self.jobs(), self.path / "run", self.resources,
            complete, poll_seconds=0)
        self.assertEqual(self.launches, [("A", (64,65)), ("B", (66,67)), ("C", (66,67))])
        self.assertEqual(completed, ["A", "B", "C"])
        self.assertTrue(all(process.reaped for process in self.processes.values()))
        self.assertEqual(result["stop_reason"], None)
        self.assertEqual(previous, {sig: signal.getsignal(sig) for sig in previous})
        events = [json.loads(row) for row in (self.path / "run/events.jsonl").read_text().splitlines()]
        sequences = [event["snapshot"]["snapshot_seq"] for event in events]
        self.assertEqual(sequences, sorted(set(sequences)))
        final = json.loads((self.path / "run/snapshot.json").read_text())
        self.assertEqual(final["state"], "completed")
        self.assertEqual(final["resources"]["workers_per_job"], 2)
        self.assertEqual(final["checkpoint"]["in_sector_resume"], False)
        self.assertFalse(final["family_closure_claim"])

    def test_first_failure_stops_siblings_and_does_not_launch_pending_job(self):
        self.failure = "B"
        with self.assertRaises(RuntimeError):
            scheduler.run(self.jobs(), self.path / "failed", self.resources, lambda _:None, poll_seconds=0)
        self.assertEqual([name for name,_ in self.launches], ["A","B"])
        self.assertTrue(all(process.reaped for process in self.processes.values()))
        self.assertTrue(self.signals)
        result = json.loads((self.path / "failed/result.json").read_text())
        self.assertTrue(result["owned_groups_drained"])
        self.assertIn("native_job_failed:B", result["stop_reason"])
        snapshot = json.loads((self.path / "failed/snapshot.json").read_text())
        states = {row["id"]: row["state"] for row in snapshot["jobs"]}
        self.assertEqual(states, {"A":"interrupted", "B":"failed", "C":"not_started"})
        self.assertEqual(snapshot["state"], "failed")

    def test_ram_is_sum_of_simultaneous_jobs_not_individual_limits(self):
        self.rss_each = 600_000_000
        with self.assertRaises(RuntimeError):
            scheduler.run(self.jobs(), self.path / "ram", self.resources, lambda _:None, poll_seconds=0)
        result = json.loads((self.path / "ram/result.json").read_text())
        self.assertIn("aggregate_rss", result["stop_reason"])
        self.assertGreater(result["sampled_peak_tree_rss_bytes"], 1_200_000_000)
        self.assertEqual(len(self.launches), 2)
        self.assertTrue(all(process.reaped for process in self.processes.values()))

    def test_operator_signal_drains_every_owned_group_and_restores_handlers(self):
        previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
        def observe(event):
            if event["event"] == "job_started" and len(self.launches) == 2:
                signal.getsignal(signal.SIGTERM)(signal.SIGTERM, None)
        with self.assertRaises(RuntimeError):
            scheduler.run(self.jobs(), self.path / "signal", self.resources, lambda _:None,
                          observer=observe, poll_seconds=0)
        result = json.loads((self.path / "signal/result.json").read_text())
        self.assertEqual(result["stop_reason"], "operator_signal_15")
        self.assertEqual(len(self.launches), 2)
        self.assertTrue(all(process.reaped for process in self.processes.values()))
        self.assertEqual(previous, {sig: signal.getsignal(sig) for sig in previous})

    def test_completion_callback_failure_drains_siblings(self):
        def fail(_):
            raise OSError("receipt write failed")
        with self.assertRaisesRegex(RuntimeError, "owned-group drain"):
            scheduler.run(self.jobs(), self.path / "callback", self.resources, fail, poll_seconds=0)
        self.assertEqual(len(self.launches), 3)
        self.assertTrue(all(process.reaped for process in self.processes.values()))

    def test_clean_parent_before_failure_is_validated_after_all_groups_drain(self):
        self.failure = "B"
        completed = []
        jobs = [scheduler.Job("A", ("A","1"), {}), scheduler.Job("B", ("B","3"), {}),
                scheduler.Job("C", ("C","50"), {})]
        def complete(job):
            self.assertTrue(all(process.reaped for process in self.processes.values()))
            completed.append(job.id)
            raise OSError("secondary validation problem")
        with self.assertRaises(RuntimeError):
            scheduler.run(jobs, self.path / "partial", self.resources, complete, poll_seconds=0)
        self.assertEqual(completed, ["A"])
        result = json.loads((self.path / "partial/result.json").read_text())
        self.assertIn("native_job_failed:B", result["stop_reason"])
        self.assertIn("secondary validation problem", result["finalization_error"])
        self.assertTrue(result["owned_groups_drained"])

    def test_invalid_budget_refuses_before_spawn(self):
        for count in (0,3,5):
            with self.assertRaises(ValueError):
                scheduler.schedule(dict(self.resources, generation_jobs=count))
        self.assertEqual(self.launches, [])

    def test_optional_renderer_failure_does_not_stop_native_jobs(self):
        def broken(_):
            raise ValueError("presentation unavailable")
        result = scheduler.run(self.jobs(), self.path / "renderer", self.resources, lambda _:None,
                               observer=broken, poll_seconds=0)
        self.assertIsNone(result["stop_reason"])
        self.assertEqual(len(result["completed_jobs"]), 3)
        self.assertTrue(all(process.reaped for process in self.processes.values()))
        status = json.loads((self.path / "renderer/snapshot.json").read_text())
        self.assertIn("presentation unavailable", status["last_observer_error"])

    def test_native_observation_is_absolute_typed_and_invocation_bound(self):
        path = self.path / "native.json"
        value = dict(schema=state.NATIVE_SCHEMA, pid=123, process_start_ticks=456,
            revision=7, counts=dict(generated=3), last_event_elapsed_seconds=1)
        path.write_text(json.dumps(value))
        stamp = path.stat().st_mtime
        for _ in range(2):
            observed = state.native_observation(path, stamp-1, now=stamp+2, pid=123, pid_start=456)
            self.assertEqual(observed["snapshot"]["counts"]["generated"], 3)
            self.assertEqual(observed["age_seconds"], 2)
        self.assertEqual(state.native_observation(path, stamp+1, now=stamp+2)["status"], "stale")
        self.assertEqual(state.native_observation(path, stamp-1, now=stamp+100)["status"], "stale")
        self.assertEqual(state.native_observation(path, stamp-1, now=stamp, pid=124)["status"], "wrong_invocation")

    def test_malformed_or_unbounded_telemetry_is_unknown_not_a_scheduler_failure(self):
        path = self.path / "native.json"
        for raw in ('{"value":NaN}', '{"value":Infinity}', '{"value":1e9999}', '{bad json',
                    ' ' * (state.MAX_NATIVE_SNAPSHOT_BYTES+1)):
            path.write_text(raw)
            result = state.native_observation(path, 0)
            self.assertEqual(result["status"], "invalid")
            self.assertIsNone(result["snapshot"])


if __name__ == "__main__":
    unittest.main()
