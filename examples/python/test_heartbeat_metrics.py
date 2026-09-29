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
    @staticmethod
    def closure_heartbeat(elapsed, total, closed, **changes):
        # The unrelated top-level inventory must not enter the new metric.
        record = heartbeat(elapsed, elapsed, 10, 999999, 1000, 0, 0)
        record["progress"]["descendant_closure"] = dict(
            available=True, total_domains=total, total_closed=closed,
            snapshot_revision=10, snapshot_stale=True, **changes)
        return record

    def test_signed_conservative_gap_uses_same_telemetry_counters(self):
        for closed, expected in ((25, 0.5), (40, -1.0), (30, 0.0)):
            with self.subTest(closed=closed):
                window = METRICS.HeartbeatWindow()
                window.observe(self.closure_heartbeat(0, 100, 20))
                window.observe(self.closure_heartbeat(10, 110, closed))
                result = window.derived()["discovery_closure_net_1h"]
                self.assertEqual(result["per_second"], expected)
                self.assertEqual(result["covered_seconds"], 10)
                self.assertEqual(result["state"], "valid")
                self.assertTrue(result["warmup"])
                self.assertTrue(result["snapshot_stale"])
                self.assertFalse(result["snapshot_advanced"])
                self.assertIsNone(result["snapshot_age_seconds"])

    def test_net_rolling_hour_boundary_and_repeated_snapshot_age(self):
        window = METRICS.HeartbeatWindow()
        for elapsed, total in ((0, 100), (1, 110), (3600, 200)):
            record = self.closure_heartbeat(elapsed, total, 20, snapshot_age_seconds=5)
            record["progress_age_seconds"] = 2
            window.observe(record)
        result = window.derived()["discovery_closure_net_1h"]
        self.assertAlmostEqual(result["per_second"], 100 / 3600)
        self.assertFalse(result["warmup"])
        self.assertFalse(result["snapshot_advanced"])
        result = window.derived(now=3601)["discovery_closure_net_1h"]
        self.assertEqual(result["samples"], 2)
        self.assertEqual(result["covered_seconds"], 3599)
        self.assertAlmostEqual(result["per_second"], 90 / 3599)
        self.assertEqual(result["snapshot_age_seconds"], 8)
        self.assertFalse(result["warmup"])
        result = window.derived(now=7201)["discovery_closure_net_1h"]
        self.assertEqual(result["state"], "no_recent_samples")
        self.assertIsNone(result["per_second"])

    def test_net_missing_invalid_and_snapshot_refresh(self):
        window = METRICS.HeartbeatWindow()
        self.assertEqual(window.derived()["discovery_closure_net_1h"]["state"], "missing_closure")
        window.observe(self.closure_heartbeat(0, 100, 20))
        self.assertIsNone(window.derived()["discovery_closure_net_1h"]["per_second"])
        record = self.closure_heartbeat(10, 110, 50)
        record["progress"]["descendant_closure"].update(snapshot_revision=11, snapshot_stale=False)
        window.observe(record)
        self.assertTrue(window.derived()["discovery_closure_net_1h"]["snapshot_advanced"])
        self.assertFalse(window.derived()["discovery_closure_net_1h"]["snapshot_stale"])
        for total, closed in ((True, 0), (100, None), (100, 101), (-1, 0)):
            window.observe(self.closure_heartbeat(20, total, closed))
            result = window.derived()["discovery_closure_net_1h"]
            self.assertEqual(result["state"], "invalid_counts")
            self.assertIsNone(result["per_second"])
        record = self.closure_heartbeat(30, 100, 20)
        record["progress"].pop("descendant_closure")
        window.observe(record)
        self.assertEqual(window.derived()["discovery_closure_net_1h"]["state"], "missing_current")
        window.observe(self.closure_heartbeat(40, 100, 20))
        self.assertEqual(window.derived()["discovery_closure_net_1h"]["samples"], 1)

    def test_net_optional_telemetry_does_not_discard_valid_history(self):
        window = METRICS.HeartbeatWindow()
        window.observe(self.closure_heartbeat(0, 100, 20))
        window.observe(self.closure_heartbeat(10, 110, 20))
        # This is the real live Epoch scalar shape: counters, but no closure.
        lean = heartbeat(20, 20, 10, 200, 1000, 0, 0)
        lean["progress"]["event"] = "epoch_heartbeat"
        window.observe(lean)
        result = window.derived()["discovery_closure_net_1h"]
        self.assertEqual(result["state"], "missing_current")
        self.assertEqual(result["samples"], 2)
        self.assertEqual(result["per_second"], 1)
        window.observe(self.closure_heartbeat(30, 115, 30))
        self.assertEqual(window.derived()["discovery_closure_net_1h"]["samples"], 3)
        self.assertEqual(window.derived()["discovery_closure_net_1h"]["per_second"], 5 / 30)
        # A resume clock reset is still observed in an optional-telemetry gap.
        lean["elapsed_seconds"] = 0
        window.observe(lean)
        self.assertEqual(window.derived()["discovery_closure_net_1h"]["reset_reason"], "nonmonotonic_time")
        window.observe(self.closure_heartbeat(40, 120, 35))
        self.assertEqual(window.derived()["discovery_closure_net_1h"]["samples"], 1)

    def test_net_non_aligned_full_hour_is_not_perpetual_warmup(self):
        window = METRICS.HeartbeatWindow()
        for elapsed in (0, 17, 1801, 3607, 3613):
            window.observe(self.closure_heartbeat(elapsed, elapsed + 100, 20))
        result = window.derived()["discovery_closure_net_1h"]
        self.assertEqual(result["covered_seconds"], 3596)
        self.assertFalse(result["warmup"])
        self.assertEqual(result["per_second"], 1)

    def test_net_invalid_time_and_refresh_count_only_marker(self):
        window = METRICS.HeartbeatWindow()
        for elapsed, refresh in ((0, 1), (10, 2)):
            record = self.closure_heartbeat(elapsed, 100 + elapsed, 20, refresh_count=refresh)
            record["progress"]["descendant_closure"].pop("snapshot_revision")
            window.observe(record)
        self.assertTrue(window.derived()["discovery_closure_net_1h"]["snapshot_advanced"])
        for invalid in (None, -1, True, float("nan")):
            window.observe(self.closure_heartbeat(invalid, 100, 20))
            result = window.derived()["discovery_closure_net_1h"]
            self.assertEqual(result["state"], "invalid_time")
            self.assertIsNone(result["per_second"])

    def test_net_resets_only_its_segment_on_resume_or_counter_regression(self):
        cases = ((0, 110, 30, {}, "nonmonotonic_time"),
                 (10, 110, 30, {}, "nonmonotonic_time"),
                 (20, 99, 30, {}, "counter_reset"),
                 (20, 110, 19, {}, "counter_reset"),
                 (20, 110, 30, {"snapshot_revision": 9}, "snapshot_reset"),
                 (20, 110, 30, {"refresh_count": 1}, "snapshot_reset"))
        for elapsed, total, closed, changes, reason in cases:
            with self.subTest(reason=reason, changes=changes):
                window = METRICS.HeartbeatWindow()
                window.observe(self.closure_heartbeat(0, 100, 20, refresh_count=2))
                window.observe(self.closure_heartbeat(10, 110, 30, refresh_count=2))
                record = self.closure_heartbeat(elapsed, total, closed)
                record["progress"]["descendant_closure"].update(changes)
                window.observe(record)
                result = window.derived()["discovery_closure_net_1h"]
                self.assertEqual(result["reset_reason"], reason)
                self.assertEqual(result["samples"], 1)
                self.assertIsNone(result["per_second"])
                self.assertEqual(len(window.samples), 3)  # Legacy arithmetic untouched.
                window.observe(self.closure_heartbeat(elapsed + 10, total + 5, closed + 10))
                self.assertEqual(window.derived()["discovery_closure_net_1h"]["per_second"], -0.5)

    def test_epoch_unknown_activity_stops_carryover_but_preserves_historical_mean(self):
        for observation in ("unavailable", "inline_call_not_pollable", "saved_cut_observation_not_live"):
            with self.subTest(observation=observation):
                window = METRICS.HeartbeatWindow()
                for elapsed, computing in ((0, 2), (10, 4)):
                    window.observe(heartbeat(elapsed, elapsed, 10, 100, 1000, 0, 0, computing=computing))
                unknown = heartbeat(20, 20, 10, 100, 1000, 0, 0)
                unknown["progress"]["parallel"] = {
                    "activity_observation": observation, "computing_workers": None, "active_workers": None,
                    "admission_preparation": {"inspection_worker_limit": 49, "lookup_worker_limit": 0,
                                              "coordinator_worker_limit": 1, "requested_worker_budget": 50}}
                window.observe(unknown)
                result = window.derived()
                self.assertIsNone(result["computing_workers"])
                self.assertIsNone(result["active_workers"])
                self.assertEqual(result["computing_inspectors_mean_1h"], 3)
                self.assertIsNone(result["coordinator_duty_1h"])
                # A later lean event cannot reach back through explicit unknown.
                window.observe(heartbeat(30, 30, 10, 100, 1000, 0, 0))
                self.assertIsNone(window.derived()["computing_workers"])
                self.assertEqual(window.derived()["computing_inspectors_mean_1h"], 3)
                joined = heartbeat(40, 40, 10, 100, 1000, 0, 0, computing=0)
                joined["progress"]["parallel"]["activity_observation"] = "joined"
                window.observe(joined)
                self.assertEqual(window.derived()["computing_workers"], 0)
                self.assertEqual(window.derived()["computing_inspectors_mean_1h"], 2)

    def test_legacy_missing_computing_field_keeps_latest_detailed_count(self):
        window = METRICS.HeartbeatWindow()
        window.observe(heartbeat(0, 0, 10, 100, 1000, 0, 0, computing=4))
        window.observe(heartbeat(10, 1, 10, 100, 1000, 0, 0))
        self.assertEqual(window.derived()["computing_workers"], 4)
        self.assertEqual(window.derived()["computing_inspectors_mean_1h"], 4)

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


# Four real heartbeat lines journaled by campaigns/five-loop-qcd-feynman-d9d10-v2
# (run 20260926T151353.794886Z, binary 102adcc3..., Ready, W100): domain_progress,
# domain_delegated, domain_progress, domain_delegated. Trimmed only of
# progress.checkpoint and parallel.{containment_prefilter,closure_refresh_policy}.
LIVE_FIXTURE = Path(__file__).with_name("fixtures") / "five_loop_ready_w100_heartbeats.jsonl"


class LiveHeartbeatFixtureTests(unittest.TestCase):
    def records(self):
        return [json.loads(line) for line in LIVE_FIXTURE.read_text().splitlines() if line]

    def test_real_key_paths_lean_and_detailed_events(self):
        records = self.records()
        self.assertEqual([record["progress"]["event"] for record in records],
                         ["domain_progress", "domain_delegated", "domain_progress", "domain_delegated"])
        for record in records:
            parallel = record["progress"]["parallel"]
            detailed = record["progress"]["event"] == "domain_progress"
            # The one duty object lives at progress.parallel.coordinator_duty,
            # and only on the periodic domain_progress events.
            self.assertEqual("coordinator_duty" in parallel, detailed)
            self.assertEqual("computing_workers" in parallel, detailed)
            self.assertNotIn("coordinator_duty", record["progress"])
            self.assertNotIn("coordinator_duty", parallel["admission_preparation"])

    def test_lean_last_heartbeat_keeps_computing_workers_and_duty_breakdown(self):
        records = self.records()
        window = METRICS.HeartbeatWindow()
        for record in records:
            window.observe(record)
        derived = window.derived()
        self.assertEqual(derived["samples_in_window"], 4)
        latest = records[2]["progress"]["parallel"]
        self.assertEqual(derived["computing_workers"], latest["computing_workers"])
        self.assertEqual(derived["active_workers"], records[3]["progress"]["parallel"]["active_workers"])
        self.assertAlmostEqual(derived["computing_inspectors_mean_1h"],
                               (records[0]["progress"]["parallel"]["computing_workers"]
                                + latest["computing_workers"]) / 2)
        breakdown = derived["coordinator_duty_breakdown_1h"]
        self.assertIsNotNone(breakdown)
        earliest = records[0]["progress"]["parallel"]["coordinator_duty"]
        span = records[2]["elapsed_seconds"] - records[0]["elapsed_seconds"]
        for key in ("ordered_commit_seconds", "preparation_seconds", "dispatch_seconds", "poll_seconds",
                    "publication_seconds", "wait_seconds", "progress_json_seconds", "ready_service_seconds"):
            self.assertAlmostEqual(breakdown[key], (latest["coordinator_duty"][key] - earliest[key]) / span, msg=key)
        # Coordinator wall tracks heartbeat wall; nested counts and the scope
        # string are not shares.
        self.assertAlmostEqual(breakdown["coordinator_elapsed_seconds"], 1.0, delta=0.01)
        self.assertNotIn("ready_service", breakdown)
        self.assertNotIn("scope", breakdown)
        # The lean tail still supplies the lean counters.
        self.assertEqual(derived["completed_nodes"], records[3]["progress"]["completed_nodes"])
        self.assertIsNotNone(derived["coordinator_duty_1h"])
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "events.jsonl"
            path.write_text(LIVE_FIXTURE.read_text())
            offline = METRICS.compute(path)
            for key in ("computing_workers", "coordinator_duty_breakdown_1h", "computing_inspectors_mean_1h"):
                self.assertEqual(offline[key], derived[key], key)

    def test_single_detailed_sample_gives_no_breakdown(self):
        records = self.records()
        window = METRICS.HeartbeatWindow()
        for record in records[1:]:
            window.observe(record)
        derived = window.derived()
        self.assertIsNone(derived["coordinator_duty_breakdown_1h"])
        self.assertEqual(derived["computing_workers"], records[2]["progress"]["parallel"]["computing_workers"])


if __name__ == "__main__":
    unittest.main()
