"""Derived heartbeat metrics on a synthetic stream; hand-computed expectations."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


METRICS = module("heartbeat_metrics")


def heartbeat(elapsed, completed, queued, discovered, rss, prep, commit, computing=None, closed=None,
              rank=7, duty=None):
    parallel = {"active_workers": 3, "admission_preparation": {"preparation_wall_seconds": prep,
                                                               "ordered_commit_wall_seconds": commit}}
    if computing is not None:
        parallel["computing_workers"] = computing
    if duty is not None:
        parallel["coordinator_duty"] = duty
    progress = {"event": "domain_progress", "completed_nodes": completed, "queued_nodes": queued,
                "scheduled_nodes": discovered, "max_scheduled_finite_rank": rank, "parallel": parallel}
    if closed is not None:
        progress["descendant_closure"] = {"available": True, "initial_closed": closed, "initial_total": 67}
    return {"event": "heartbeat", "elapsed_seconds": elapsed, "process_rss_bytes": rss,
            "currently_discovered_nodes": discovered, "progress": progress}


def saved(generation, duration, size, bootstrap=False):
    checkpoint = {"state": "saved", "generation": generation, "bytes": size, "saved_unix_time": 1790000000 + generation}
    if bootstrap:
        checkpoint["bootstrap"] = True
    else:
        checkpoint["duration_seconds"] = duration
    return {"event": "checkpoint_saved", "checkpoint": checkpoint}


def synthetic_stream():
    # elapsed: 0, 10, 40, 41, 100 → intervals 10 (Δ2), 30 (Δ0 stalled ≥5 and ≥20), 1 (Δ0), 59 (Δ4)
    return [
        heartbeat(0.0, 0, 10, 100, 1_000_000, 0.0, 0.0, computing=2, closed=1),
        saved(1, None, 25, bootstrap=True),
        heartbeat(10.0, 2, 12, 110, 1_100_000, 1.0, 0.5, computing=4, closed=1),
        heartbeat(40.0, 2, 13, 115, 1_200_000, 4.0, 2.0, computing=3, closed=2),
        saved(2, 3.0, 5_000),
        heartbeat(41.0, 2, 13, 115, 1_200_000, 4.1, 2.0, closed=2),
        {"event": "heartbeat", "elapsed_seconds": 80.0, "progress": {"event": "preparation", "completed": 3}},
        heartbeat(100.0, 6, 20, 120, 1_800_000, 10.0, 5.0, computing=3, closed=3, rank=9),
        saved(3, 2.0, 6_000),
    ]


class HeartbeatWindowTests(unittest.TestCase):
    def test_hand_computed_window_metrics(self):
        window = METRICS.HeartbeatWindow(window_seconds=3600, retained_seconds=7200)
        for record in synthetic_stream():
            window.observe(record)
        derived = window.derived()
        self.assertEqual(derived["schema"], METRICS.SCHEMA)
        self.assertEqual(derived["samples_in_window"], 5)
        self.assertEqual(derived["window_wall_seconds"], 100.0)
        self.assertAlmostEqual(derived["completions_per_hour_1h"], 6 / 100 * 3600)
        self.assertAlmostEqual(derived["stall_share_5s"], 30 / 100)
        self.assertAlmostEqual(derived["stall_share_20s"], 30 / 100)
        self.assertAlmostEqual(derived["pending_growth_per_completion_1h"], (20 - 10) / 6)
        self.assertAlmostEqual(derived["rss_bytes_per_discovered_domain"], 1_800_000 / 120)
        self.assertAlmostEqual(derived["coordinator_duty_1h"], 15.0 / 100)
        self.assertAlmostEqual(derived["computing_inspectors_mean_1h"], (2 + 4 + 3 + 3) / 4)
        self.assertEqual(derived["computing_workers"], 3)
        self.assertEqual(derived["max_scheduled_finite_rank"], 9)
        self.assertEqual((derived["roots_closed"], derived["roots_total"]), (3, 67))
        self.assertEqual(derived["checkpoint_saves_counted"], 2)
        self.assertAlmostEqual(derived["checkpoint_save_seconds"], 5.0)
        self.assertAlmostEqual(derived["checkpoint_duty"], 5.0 / 100)
        self.assertEqual(derived["last_checkpoint"], {"generation": 3, "bytes": 6_000, "duration_seconds": 2.0})
        self.assertEqual(derived["completed_nodes"], 6)
        self.assertEqual(derived["pending_nodes"], 20)
        self.assertFalse(any(part in ("eta", "estimate", "estimated") for key in derived for part in key.split("_")))

    def test_absent_fields_are_null_and_window_bounds_apply(self):
        window = METRICS.HeartbeatWindow(window_seconds=50, retained_seconds=60)
        self.assertIsNone(window.derived()["completions_per_hour_1h"])
        for record in synthetic_stream():
            record = json.loads(json.dumps(record))
            if record["event"] == "heartbeat" and "parallel" in record["progress"]:
                record["progress"]["parallel"].pop("computing_workers", None)
                record["progress"].pop("descendant_closure", None)
            window.observe(record)
        derived = window.derived()
        self.assertIsNone(derived["computing_inspectors_mean_1h"])
        self.assertIsNone(derived["roots_closed"])
        # Retained 60 s keeps the samples at 40, 41 and 100; a 50 s window from 100 keeps only the last.
        self.assertEqual(len(window.samples), 3)
        self.assertEqual(derived["samples_in_window"], 1)
        derived = window.derived(now=100.0, window_seconds=60)
        self.assertEqual(derived["samples_in_window"], 3)
        self.assertAlmostEqual(derived["completions_per_hour_1h"], 4 / 60 * 3600)
        self.assertAlmostEqual(derived["stall_share_5s"], 0.0)
        self.assertIsNone(METRICS.sample_from_record({"event": "resources"}))
        self.assertIsNone(METRICS.sample_from_record({"event": "heartbeat", "progress": {"completed_nodes": True}}))
        self.assertIsNone(METRICS.checkpoint_save_from_record(
            {"event": "checkpoint_started", "checkpoint_write": {"state": "writing", "generation": 4}}))
        self.assertIsNone(METRICS.checkpoint_save_from_record(
            {"event": "heartbeat", "progress": {"checkpoint": {"state": "writing", "generation": 4}}}))
        with self.assertRaises(ValueError):
            METRICS.HeartbeatWindow(window_seconds=10, retained_seconds=5)

    def test_optional_coordinator_duty_breakdown(self):
        window = METRICS.HeartbeatWindow()
        window.observe(heartbeat(0.0, 0, 0, 1, 1, 0.0, 0.0, duty={"dispatch": 0.0, "wait": 0.0, "coordinator_elapsed_seconds": 0.0}))
        window.observe(heartbeat(10.0, 5, 0, 5, 5, 1.0, 1.0, duty={"dispatch": 2.0, "wait": 6.0, "coordinator_elapsed_seconds": 10.0}))
        derived = window.derived()
        self.assertEqual(derived["coordinator_duty_breakdown_1h"], {"dispatch": 0.2, "wait": 0.6, "coordinator_elapsed_seconds": 1.0})
        window = METRICS.HeartbeatWindow()
        window.observe(heartbeat(0.0, 0, 0, 1, 1, 0.0, 0.0))
        window.observe(heartbeat(10.0, 5, 0, 5, 5, 1.0, 1.0))
        self.assertIsNone(window.derived()["coordinator_duty_breakdown_1h"])


class OfflineComputationTests(unittest.TestCase):
    def test_offline_window_matches_live_arithmetic_and_tolerates_partial_tail(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "events.jsonl"
            lines = [json.dumps(record) for record in synthetic_stream()]
            path.write_text("\n".join(lines) + "\nnot json\n" + '{"event": "heartbeat", "elapsed_seconds": 200, "progress": {"comple')
            live = METRICS.HeartbeatWindow()
            for record in synthetic_stream():
                live.observe(record)
            offline = METRICS.compute(path)
            for key in ("completions_per_hour_1h", "stall_share_5s", "stall_share_20s", "pending_growth_per_completion_1h",
                        "coordinator_duty_1h", "checkpoint_duty", "computing_inspectors_mean_1h", "last_checkpoint"):
                self.assertEqual(offline[key], live.derived()[key], key)
            self.assertTrue(offline["partial_last_line"])
            self.assertEqual(offline["invalid_records"], 1)
            self.assertEqual(offline["records"], len(lines))
            windowed = METRICS.compute(path, start=10, end=41)
            self.assertEqual(windowed["samples_in_window"], 3)
            self.assertEqual(windowed["records_before_start"], 2)
            self.assertEqual(windowed["records_after_end"], 3)
            self.assertAlmostEqual(windowed["completions_per_hour_1h"], 0.0)
            self.assertAlmostEqual(windowed["stall_share_5s"], 30 / 31)
            self.assertEqual(windowed["checkpoint_saves_counted"], 1)
            self.assertAlmostEqual(windowed["checkpoint_duty"], 3.0 / 31)
            self.assertEqual(windowed["last_checkpoint"]["generation"], 2)
            self.assertEqual(windowed["window_seconds"], 31)
            result = subprocess.run([sys.executable, "-B", METRICS.__file__, str(path), "--start", "10", "--end", "41"],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)["samples_in_window"], 3)
            result = subprocess.run([sys.executable, "-B", METRICS.__file__, str(path), "--window", "0"],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            with self.assertRaises(ValueError):
                METRICS.compute(path, start=50, end=10)


if __name__ == "__main__":
    unittest.main()
