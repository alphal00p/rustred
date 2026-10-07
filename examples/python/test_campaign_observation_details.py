"""Fast observation-only regressions: no algebra, campaigns or checkpoint writes."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def temporary_directory():
    root = Path(__file__).resolve().parents[2] / "TMP"
    root.mkdir(exist_ok=True)
    return tempfile.TemporaryDirectory(prefix="monitor-observation-test-", dir=root)


MONITOR = module("campaign_monitor")
TELEMETRY = module("campaign_telemetry")
DASHBOARD = module("campaign_dashboard")
METRICS = module("heartbeat_metrics")


def scan(at, discovered, closed, sequence):
    return dict(completed_unix_seconds=at, total_domains=discovered,
                total_closed=closed, initial_closed=1, refresh_count=sequence, scan_seconds=5)


class CompletedScanTests(unittest.TestCase):
    def test_extended_required_scope_does_not_relabel_base_closure_as_stage_closure(self):
        status = self.status()
        status["progress"]["query_admission"] = {
            "required": 232, "original_required": 116, "appended_required": 116,
            "admitted_required": 232, "unadmitted": 0,
        }
        frame = TELEMETRY.normalize_status(status)
        rendered = "\n".join(DASHBOARD.render_table(frame, width=160, height=42, color=False))
        self.assertIn("BASE ROOT CLOSURE", rendered)
        self.assertIn("Required requests 232 cumulative", rendered)
        self.assertIn("cold verification required", rendered)
        compact = "\n".join(DASHBOARD.render_table(frame, width=80, height=24, color=False))
        self.assertIn("BASE ROOT CLOSURE", compact)
        self.assertIn("base bar ≠ stage closure; cold verify", compact)
        self.assertIn("Checkpoint size", compact)
        self.assertIn("Completed-scan D/C", compact)
        plain = "\n".join(DASHBOARD.plain_summary(frame))
        self.assertIn("116 appended", plain)
        native = MONITOR.progress_summary({"query_admission": status["progress"]["query_admission"]}, None, 0)
        self.assertEqual(native["query_admission"]["required"], 232)
        wrapped = {"query_admission": status["progress"]["query_admission"], "snapshot": {"phase": "inspect"}}
        self.assertEqual(MONITOR.progress_summary(wrapped, None, 0)["query_admission"]["required"], 232)
        tail = MONITOR.EventTail(Path("unused-scope-test.jsonl"))
        tail.pending = json.dumps(wrapped).encode()
        tail._finish(0)
        tail.pending = json.dumps({"event": "epoch_heartbeat", "snapshot": {"phase": "inspect"}}).encode()
        tail._finish(1)
        self.assertEqual(tail.query_admission["required"], 232)
        self.assertEqual(tail.query_admission["appended_required"], 116)
        self.assertNotIn("required_closed", tail.query_admission)

    def status(self):
        return {"heartbeat_unix_time": 11000, "heartbeat_age_seconds": 50,
            "derived": {"pending_growth_per_completion_1h": .22},
            "progress": {"descendant_closure": {
                "available": True, "initial_total": 2, "initial_closed": 1,
                "total_domains": 1000, "total_closed": 250, "unresolved_domains": 750,
                "snapshot_stale": True, "snapshot_age_seconds": 10000,
                "scan_history": {"previous": scan(100, 100, 50, 1), "latest": scan(1100, 500, 250, 2)},
                "refresh_policy": {"status": "throttled", "next_refresh_seconds": 10,
                    "earliest_refresh_unix_seconds": 11010}}}}

    def test_two_actual_scans_remain_visible_after_hour_window_expires(self):
        frame = TELEMETRY.normalize_status(self.status())
        ratio = frame["rates"]["discovery_per_recursive_closure_scans"]
        self.assertEqual((ratio["value"], ratio["discovered_delta"], ratio["closed_delta"]), (2, 400, 200))
        self.assertEqual((ratio["covered_seconds"], ratio["age_seconds"]), (1000, 9950))
        self.assertEqual(frame["rates"]["pending_growth_per_completion_1h"], .22)
        self.assertEqual(frame["closure_snapshot"]["refresh_policy"]["status"], "eligible")
        lines = "\n".join(DASHBOARD.plain_summary(frame))
        self.assertIn("Completed-scan D/C 2.000", lines)
        self.assertIn("ΔD 400 / ΔC 200", lines)
        self.assertIn("D 100→500 · C 50→250", lines)
        self.assertIn("1970-01-01 00:01:40 UTC", lines)
        self.assertIn("due; awaiting coordinator safe-point", lines)
        self.assertIn("pending +0.22 per completion", lines)
        for width in (55, 80, 120, 160):
            rendered = DASHBOARD.render_table(frame, width=width, height=40, color=False)
            self.assertTrue(all(DASHBOARD.cell_width(line) == width for line in rendered))
            self.assertLessEqual(len(rendered), 40)
        compact = "\n".join(DASHBOARD.render_table(frame, width=80, height=24, color=False))
        self.assertIn("Encountered rank", compact)
        self.assertIn("Checkpoint size", compact)
        json.dumps(frame, allow_nan=False)

    def test_zero_closures_empty_interval_reset_and_missing_are_distinct(self):
        history = {"previous": scan(100, 100, 50, 1), "latest": scan(200, 150, 50, 2)}
        ratio = TELEMETRY.completed_scan_ratio(history, 10000)
        self.assertTrue(ratio["infinite"])
        self.assertEqual(ratio["state"], "no_observed_closures")
        history["latest"] = scan(200, 100, 50, 2)
        ratio = TELEMETRY.completed_scan_ratio(history)
        self.assertFalse(ratio["infinite"])
        self.assertEqual(ratio["state"], "empty_scan_interval")
        history["latest"] = scan(200, 110, 20, 2)
        self.assertEqual(TELEMETRY.completed_scan_ratio(history)["state"], "scan_counter_reset")
        self.assertEqual(TELEMETRY.completed_scan_ratio({})["state"], "awaiting_two_scans")
        history["latest"] = scan(200, True, 0, 2)
        self.assertEqual(TELEMETRY.completed_scan_ratio(history)["state"], "invalid_scan")

    def test_restored_snapshot_age_uses_persisted_completed_scan(self):
        status = self.status()
        status["progress"]["descendant_closure"]["snapshot_age_seconds"] = None
        frame = TELEMETRY.normalize_status(status)
        self.assertEqual(frame["closure_snapshot"]["snapshot_age_seconds"], 9950)
        self.assertIn("age 02:45:50", "\n".join(DASHBOARD.derived_lines(status)))

    def test_restored_stale_scan_cannot_become_measured_zero_hourly_closure(self):
        status = self.status()
        status["progress"]["descendant_closure"]["snapshot_age_seconds"] = None
        status["derived"]["discovery_closure_net_1h"] = {
            "state": "valid", "covered_seconds": 3600, "window_seconds": 3600,
            "first_elapsed_seconds": 6400, "last_elapsed_seconds": 10000,
            "discovered_delta": 1000, "closed_delta": 0,
            "discovered_per_second": 1000 / 3600, "closed_per_second": 0,
            "per_second": 1000 / 3600}
        frame = TELEMETRY.normalize_status(status)
        hourly = frame["rates"]["discovery_per_recursive_closure_1h"]
        self.assertEqual(hourly["state"], "awaiting_closure_scan")
        self.assertFalse(hourly["infinite"])
        self.assertIsNone(hourly["value"])
        self.assertIsNone(frame["rates"]["recursive_closure"]["per_second"])
        self.assertEqual(frame["rates"]["discovery_per_recursive_closure_scans"]["value"], 2)

    def test_effective_rank_does_not_confuse_unknown_with_unbounded(self):
        for status, maximum, expected in (("finite", 0, "≤0 (bound)"),
                                          ("finite", 30, "≤30 (bound)"),
                                          ("unbounded", None, "unbounded"),
                                          ("unknown", None, "unknown")):
            frame = TELEMETRY.normalize_status({"progress": {"encountered_numerator_rank": {
                "status": status, "maximum": maximum, "scope": "scheduled domain geometry"}}})
            self.assertEqual(DASHBOARD.rank_text(frame["rates"]["encountered_numerator_rank"]), expected)
            text = DASHBOARD.derived_lines({"progress": {"encountered_numerator_rank": {
                "status": status, "maximum": maximum}}})
            self.assertIn("encountered rank " + expected, "\n".join(text))
        legacy = TELEMETRY.encountered_rank_summary({}, {"max_scheduled_finite_rank": 12})
        self.assertEqual(legacy["status"], "unknown")
        self.assertIn("finite cap 12", DASHBOARD.rank_text(legacy))
        uncapped = TELEMETRY.encountered_rank_summary(
            {"max_scheduled_finite_rank": 12, "unbounded_rank_domains": 1}, {})
        self.assertEqual(uncapped["status"], "unknown")
        self.assertNotEqual(DASHBOARD.rank_text(uncapped), "unbounded")
        exact = 2 ** 80 + 1
        native = TELEMETRY.encountered_rank_summary({"encountered_numerator_rank": {
            "status": "finite", "maximum": str(exact)}}, {})
        self.assertEqual(native["maximum"], exact)
        self.assertEqual(DASHBOARD.rank_text(native), f"≤{exact:,} (bound)")


class CheckpointObservationTests(unittest.TestCase):
    def test_malformed_observation_metadata_never_raises_into_supervisor(self):
        with temporary_directory() as temporary:
            path = Path(temporary) / "latest.json"
            for malformed in ({"manifest": []},
                              {"manifest": {"format": "RUSTRED-WALK-CP6", "generation": 7, "files": [None]}},
                              {"manifest": {"format": "RUSTRED-WALK-CP6", "generation": 7, "files": {}}}):
                with self.subTest(malformed=malformed):
                    path.write_text(json.dumps(malformed))
                    observed = MONITOR.CheckpointSizeCache().enrich({"state": "saved", "generation": 7,
                                                                   "directory": temporary})
                    self.assertIsNone(observed["bytes"])
                    self.assertTrue(observed["bytes_observation_error"])

    def test_cp6_metadata_and_sealed_record_payloads_cached_once(self):
        with temporary_directory() as temporary:
            directory = Path(temporary)
            segments = [{"file": "records-1.bin", "bytes": 1234}, {"file": "records-2.bin", "bytes": 5678}]
            (directory / "segments.json").write_text(json.dumps(segments))
            manifest = {"format": "RUSTRED-WALK-CP6", "generation": 7, "files": [
                {"key": "state-1", "file": "state.bin", "bytes": 7000},
                {"key": "record-segments", "file": "segments.json", "bytes": 50}]}
            (directory / "latest.json").write_text(json.dumps({"manifest": manifest}))
            checkpoint = {"state": "saved", "generation": 7, "directory": temporary}
            cache = MONITOR.CheckpointSizeCache()
            with patch.object(cache, "_json", wraps=cache._json) as reads:
                self.assertEqual(cache.enrich(checkpoint)["bytes"], 13962)
                for _ in range(50):
                    self.assertEqual(cache.enrich(checkpoint)["bytes"], 13962)
                self.assertEqual(reads.call_count, 2)
                self.assertEqual(cache.enrich({**checkpoint, "bytes": 0})["bytes"], 0)
                self.assertEqual(reads.call_count, 2)

    def test_cp5_previous_generation_and_untrusted_segment_path(self):
        with temporary_directory() as temporary:
            directory = Path(temporary)
            (directory / "latest.json").write_text(json.dumps({"generation": 8}))
            (directory / "previous.json").write_text(json.dumps({"format": "RUSTRED-WALK-CP5",
                "generation": 7, "sections": {"nodes": {"file": "nodes.bin", "bytes": 12},
                "records": {"segments": [{"file": "records.bin", "bytes": 30}]}}}))
            result = MONITOR.CheckpointSizeCache().enrich({"state": "saved", "generation": 7, "directory": temporary})
            self.assertEqual(result["bytes"], 42)
            (directory / "latest.json").write_text(json.dumps({"manifest": {"format": "RUSTRED-WALK-CP6", "generation": 8,
                "files": [{"key": "record-segments", "file": "../escape.json", "bytes": 1}]}}))
            result = MONITOR.CheckpointSizeCache().enrich({"state": "saved", "generation": 8, "directory": temporary})
            self.assertIsNone(result["bytes"])
            self.assertIn("invalid checkpoint file name", result["bytes_observation_error"])

    def test_fresh_monitor_attachment_recovers_nested_persisted_checkpoint_size(self):
        with temporary_directory() as temporary:
            directory = Path(temporary)
            (directory / "latest.json").write_text(json.dumps({"format": "RUSTRED-WALK-CP5",
                "generation": 7, "sections": {"nodes": {"file": "nodes.bin", "bytes": 12}}}))
            (directory / "status.json").write_text(json.dumps({"state": "stopped",
                "progress": {"checkpoint": {"state": "saved", "generation": 7, "directory": temporary}}}))
            observed = MONITOR.read_status(directory)
            self.assertEqual(observed["checkpoint"]["bytes"], 12)

    def test_checkpoint_duty_uses_reported_phase_wall_when_save_duration_absent(self):
        metrics = METRICS.HeartbeatWindow()
        metrics.observe({"event": "heartbeat", "elapsed_seconds": 100,
            "progress": {"completed_nodes": 10, "telemetry": {
                "phase_wall_seconds": {"inspect": 70, "checkpoint": 20, "restore": 10}}}})
        derived = metrics.derived()
        self.assertEqual(derived["checkpoint_duty"], .2)
        self.assertEqual(derived["checkpoint_duty_scope"], "native current-invocation coordinator phase wall")


if __name__ == "__main__":
    unittest.main()
