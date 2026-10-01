"""No-native monitoring tests: normalized stream, terminal consumer, SVG utility."""
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tracemalloc
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


TELEMETRY = module("campaign_telemetry")
DASHBOARD = module("campaign_dashboard")
METRICS = module("heartbeat_metrics")
PLOT = module("plot_campaign_rates")
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


def sample_status():
    return {
        "state": "running", "elapsed_seconds": 1200, "heartbeat_unix_time": 1790720400,
        "workers": 50, "hard_memory_bytes": 400_000_000_000, "soft_memory_bytes": 380_000_000_000,
        "heartbeat_age_seconds": 0, "heartbeat_stale": False,
        "run_directory": "/campaigns/example/campaign_runs/20260929T220000Z",
        "resources": {"native_busy_cores": 39.2, "aggregate_rss_bytes": 31_200_000_000,
                      "host_available_bytes": 700_000_000_000},
        "progress": {"phase": "Route", "progress_age_seconds": 1.2,
            "active_native_slots": 47, "backpressured_native_slots": 2,
            "finished_native_awaiting_publication": 1,
            "worker_reservations": {"inspectors": 49, "admission_helpers": 0, "coordinator": 1},
            "initial_entry_progress": {"published": 67, "total": 67, "locally_inspected": 67},
            "work": {"scheduled": 6_500_000, "locally_completed": 1_830_000,
                     "pending": 1_450_000, "frontiers": 0},
            "descendant_closure": {"available": True, "initial_total": 67, "initial_closed": 12,
                "total_domains": 6_500_000, "total_closed": 1_440_000, "unresolved_domains": 5_060_000,
                "snapshot_stale": True, "snapshot_age_seconds": 2, "refresh_count": 81,
                "snapshot_revision": 130, "graph_revision": 140}},
        "derived": {"window_seconds": 3600, "window_wall_seconds": 600,
            "first_elapsed_seconds": 600, "last_elapsed_seconds": 1200, "now_seconds": 1200,
            "completions_per_hour_1h": 5_400_000, "completions_delta_1h": 900_000,
            "pending_growth_per_completion_1h": .22,
            "discovery_closure_net_1h": {"state": "valid", "per_second": -12,
                "covered_seconds": 600, "window_seconds": 3600, "warmup": True,
                "first_elapsed_seconds": 600, "last_elapsed_seconds": 1200,
                "discovered_delta": 120_000, "closed_delta": 127_200,
                "discovered_per_second": 200, "closed_per_second": 212,
                "snapshot_stale": True, "snapshot_age_seconds": 2, "snapshot_advanced": True}},
        "checkpoint": {"state": "saved", "generation": 4, "duration_seconds": 3.22,
                       "directory": "/campaigns/example/checkpoints/main", "bytes": 123_000_000}}


class TelemetryTests(unittest.TestCase):
    def test_dirty_closure_snapshot_must_have_a_scan_inside_window(self):
        status = sample_status()
        gap = status["derived"]["discovery_closure_net_1h"]
        gap.update(discovered_delta=1200, closed_delta=0, per_second=2,
                   discovered_per_second=2, closed_per_second=0)
        for age, blocked in ((599, False), (600, True), (5700, True)):
            with self.subTest(age=age):
                status["progress"]["descendant_closure"]["snapshot_age_seconds"] = age
                frame = TELEMETRY.normalize_status(status)
                ratio = frame["rates"]["discovery_per_recursive_closure_1h"]
                self.assertEqual(ratio["infinite"], not blocked)
                self.assertEqual(frame["rates"]["recursive_closure"]["per_second"],
                                 None if blocked else 0)
                self.assertEqual(frame["rates"]["pending_growth_per_completion_1h"], .22)
                self.assertEqual(frame["rates"]["discovery_minus_closure"]["discovered_delta"], 1200)
                self.assertEqual(frame["counts"]["total_closed"], 1_440_000)
                if blocked:
                    self.assertEqual(ratio["state"], "awaiting_closure_scan")
                    self.assertIn("awaiting_closure_scan", "\n".join(DASHBOARD.derived_lines(status)))
                json.dumps(frame, allow_nan=False)
        # No graph change since the scan: an old counter is still current.
        status["progress"]["descendant_closure"]["snapshot_stale"] = False
        frame = TELEMETRY.normalize_status(status)
        self.assertEqual(frame["rates"]["recursive_closure"]["per_second"], 0)
        self.assertTrue(frame["rates"]["discovery_per_recursive_closure_1h"]["infinite"])

    def test_old_saved_frames_hide_unmeasured_closure_rates_in_all_consumers(self):
        frame = TELEMETRY.normalize_status(sample_status())
        frame["closure_snapshot"].update(snapshot_age_seconds=5700, stale=True)
        frame["rates"]["recursive_closure"]["per_second"] = 0
        frame["rates"]["discovery_minus_closure"].update(
            discovered_delta=1200, closed_delta=0, per_second=2,
            discovered_per_second=2, closed_per_second=0)
        for width in (80, 100, 140):
            lines = DASHBOARD.render_table(frame, width, 32, True)
            ratio = next(line for line in lines if "Discovery/closure" in line)
            closure = next(line for line in lines if "Recursive closure" in line)
            self.assertIn("Discovery/closure unknown", ratio)
            self.assertNotIn("\x1b", ratio)
            self.assertIn("unknown", closure)
            self.assertIn("awaiting closure scan", closure)
            self.assertNotIn("0.000/s", closure)
        plain = "\n".join(DASHBOARD.plain_summary(frame))
        self.assertIn("Recursive closure unknown awaiting closure scan", plain)
        self.assertNotIn("∞", plain)
        with patch.object(PLOT, "records", return_value=iter([frame])):
            point = next(PLOT.observations("unused"))
        self.assertIsNone(point["closure"])
        self.assertIsNone(point["gap"])
        self.assertEqual(point["completion"], 1500)
        self.assertEqual(point["unresolved"], 5_060_000)

    def test_ratio_uses_paired_deltas_and_preserves_raw_rates(self):
        status = sample_status()
        frame = TELEMETRY.normalize_status(status)
        ratio = frame["rates"]["discovery_per_recursive_closure_1h"]
        self.assertAlmostEqual(ratio["value"], 120000 / 127200)
        self.assertFalse(ratio["infinite"])
        self.assertEqual(ratio["covered_seconds"], 600)
        self.assertTrue(ratio["warmup"])
        self.assertNotIn("discovery_closure_balance", frame["rates"])
        self.assertEqual(frame["rates"]["discovery_minus_closure"]["per_second"], -12)
        # Independent local completions may have a different sampled window.
        status["derived"]["window_wall_seconds"] = 500
        self.assertEqual(TELEMETRY.normalize_status(status)["rates"]["discovery_per_recursive_closure_1h"], ratio)

    def test_ratio_refuses_missing_reset_empty_and_mismatched_windows(self):
        original = sample_status()["derived"]["discovery_closure_net_1h"]
        for changes in ({"discovered_delta": None}, {"closed_delta": -1},
                        {"closed_delta": float("inf")}, {"per_second": float("nan")},
                        {"state": "counter_reset"}, {"state": "missing_current"},
                        {"state": "snapshot_reset"}, {"state": "invalid_counts"},
                        {"covered_seconds": 599}, {"first_elapsed_seconds": None},
                        {"last_elapsed_seconds": 600}, {"window_seconds": 500},
                        {"closed_per_second": 211},
                        {"discovered_delta": 0, "closed_delta": 0, "discovered_per_second": 0,
                         "closed_per_second": 0, "per_second": 0}):
            with self.subTest(changes=changes):
                ratio = TELEMETRY.discovery_per_recursive_closure_1h({**original, **changes})
                self.assertIsNone(ratio["value"])
                self.assertFalse(ratio["infinite"])
        for discovered, closed, expected in ((30, 10, 3), (10, 10, 1), (0, 10, 0), (10, 0, None)):
            gap = {**original, "discovered_delta": discovered, "closed_delta": closed,
                   "discovered_per_second": discovered / 600, "closed_per_second": closed / 600,
                   "per_second": (discovered - closed) / 600}
            ratio = TELEMETRY.discovery_per_recursive_closure_1h(gap)
            self.assertEqual(ratio["value"], expected)
            self.assertEqual(ratio["infinite"], closed == 0)
            json.dumps(ratio, allow_nan=False)
        status = sample_status()
        status["progress"]["descendant_closure"]["available"] = False
        ratio = TELEMETRY.normalize_status(status)["rates"]["discovery_per_recursive_closure_1h"]
        self.assertIsNone(ratio["value"])
        self.assertFalse(ratio["infinite"])
        # Explicitly unavailable current telemetry suppresses even an otherwise
        # infinite historical ratio, but preserves its aged snapshot diagnostic.
        status["heartbeat_age_seconds"] = 7
        status["progress"]["descendant_closure"]["snapshot_age_seconds"] = 99
        status["derived"]["discovery_closure_net_1h"].update(
            discovered_delta=1200, closed_delta=0, per_second=2,
            discovered_per_second=2, closed_per_second=0)
        lines = DASHBOARD.derived_lines(status)
        self.assertIn("Discovery/closure unknown", "\n".join(lines))
        self.assertIn("closure_unavailable", "\n".join(lines))
        self.assertNotIn("∞", "\n".join(lines))
        self.assertIn("Closure snapshot stale · age 00:00:09 · scan advanced", lines)
        ratio = TELEMETRY.normalize_status(status)["rates"]["discovery_per_recursive_closure_1h"]
        self.assertIsNone(ratio["value"])
        self.assertFalse(ratio["infinite"])

    def test_two_rates_preserve_distinct_meanings_and_raw_deltas(self):
        frame = TELEMETRY.normalize_status(sample_status())
        self.assertEqual(frame["rates"]["local_completion"]["per_second"], 1500)
        self.assertEqual(frame["rates"]["recursive_closure"]["per_second"], 212)
        self.assertEqual(frame["rates"]["recursive_closure"]["delta"], 127200)
        self.assertEqual(frame["rates"]["discovery_minus_closure"]["per_second"], -12)
        self.assertEqual(frame["rates"]["pending_growth_per_completion_1h"], .22)
        self.assertTrue(frame["closure_snapshot"]["stale"])
        self.assertTrue(frame["closure_snapshot"]["closed_counts_are_conservative_lower_bounds"])
        self.assertTrue(frame["closure_snapshot"]["unresolved_counts_are_conservative_upper_bounds"])
        self.assertFalse(frame["family_closure_claim"])

    def test_actual_heartbeat_window_additive_endpoints(self):
        metrics = METRICS.HeartbeatWindow()
        for elapsed, completed, discovered, closed in ((0, 5, 100, 20), (10, 25, 130, 60)):
            metrics.observe({"event": "heartbeat", "elapsed_seconds": elapsed,
                "progress": {"completed_nodes": completed, "queued_nodes": 10, "scheduled_nodes": discovered,
                    "descendant_closure": {"available": True, "total_domains": discovered,
                        "total_closed": closed, "snapshot_revision": elapsed, "snapshot_stale": True}}})
        status = sample_status()
        status["derived"] = metrics.derived()
        frame = TELEMETRY.normalize_status(status)
        self.assertEqual(frame["rates"]["local_completion"]["delta"], 20)
        self.assertEqual(frame["rates"]["local_completion"]["per_second"], 2)
        self.assertEqual(frame["rates"]["recursive_closure"]["delta"], 40)
        self.assertEqual(frame["rates"]["recursive_closure"]["per_second"], 4)
        self.assertEqual(frame["rates"]["discovery_minus_closure"]["per_second"], -1)
        self.assertEqual(frame["rates"]["discovery_per_recursive_closure_1h"]["value"], .75)

    def test_delayed_completion_samples_have_segment_aware_warmup(self):
        metrics = METRICS.HeartbeatWindow()
        def observe(elapsed, completed):
            metrics.observe({"event": "heartbeat", "elapsed_seconds": elapsed,
                             "progress": {"completed_nodes": completed, "queued_nodes": 10}})
        observe(600, 0)
        observe(601, 1)
        observe(3600, 3000)
        self.assertTrue(metrics.derived()["completions_warmup_1h"])
        self.assertEqual(metrics.derived()["window_wall_seconds"], 3000)
        observe(4200.5, 4000)
        self.assertFalse(metrics.derived()["completions_warmup_1h"])
        self.assertEqual(metrics.derived()["window_wall_seconds"], 3599.5)
        observe(4201, 0)
        self.assertTrue(metrics.derived()["completions_warmup_1h"])
        self.assertFalse(metrics.derived()["completions_window_valid_1h"])

    def test_missing_reset_and_unavailable_are_not_zero(self):
        frame = TELEMETRY.normalize_status({})
        self.assertIsNone(frame["rates"]["local_completion"]["per_second"])
        self.assertIsNone(frame["rates"]["recursive_closure"]["per_second"])
        for state in ("missing_current", "unavailable_current", "counter_reset", "invalid_counts"):
            status = sample_status()
            status["derived"]["discovery_closure_net_1h"]["state"] = state
            self.assertIsNone(TELEMETRY.normalize_status(status)["rates"]["recursive_closure"]["per_second"])
        status = sample_status()
        status["derived"]["completions_delta_1h"] = -1
        self.assertEqual(TELEMETRY.normalize_status(status)["rates"]["local_completion"]["state"], "counter_reset")
        status = sample_status()
        status["progress"]["descendant_closure"]["total_closed"] = 99_000_000
        frame = TELEMETRY.normalize_status(status)
        self.assertIsNone(frame["counts"]["total_closed"])
        self.assertFalse(frame["closure_snapshot"]["available"])

    def test_stream_is_bounded_machine_readable_and_failure_is_nonfatal(self):
        with tempfile.TemporaryDirectory() as temporary:
            stream = TELEMETRY.TelemetryStream(Path(temporary) / "telemetry.jsonl")
            for _ in range(3):
                stream.emit(sample_status())
            lines = stream.path.read_bytes().splitlines()
            self.assertEqual([json.loads(line)["sequence"] for line in lines], [0, 1, 2])
            self.assertTrue(all(len(line) < TELEMETRY.MAX_FRAME_BYTES for line in lines))
            self.assertIsNone(stream.error)
            failed = TELEMETRY.TelemetryStream(Path(temporary) / "absent" / "telemetry.jsonl")
            self.assertEqual(failed.emit(sample_status())["state"], "running")
            self.assertIsNotNone(failed.error)
            failed.emit(sample_status())
            self.assertEqual(failed.sequence, 2)
            original = TELEMETRY.normalize_status
            with patch.object(TELEMETRY, "normalize_status", side_effect=[ValueError("bad observation"), original({})]):
                self.assertEqual(failed.emit({})["observation_error"], "bad observation")

    def test_saved_frame_alone_retains_milestones_and_plain_message(self):
        status = sample_status()
        status["checkpoint_milestones"] = [{"event": "checkpoint_saved", "sequence": 1,
            "generation": 4, "saved_unix_time": 1790208000, "duration_seconds": 3.22,
            "state_path": "/run/checkpoint/state-4"}]
        frame = json.loads(json.dumps(TELEMETRY.normalize_status(status)))
        output = io.StringIO()
        presenter = DASHBOARD.Presenter(output)
        presenter.render_frame(frame, now=0)
        presenter.render_frame(frame, now=1)
        events = [json.loads(line) for line in output.getvalue().splitlines()]
        self.assertEqual([event["event"] for event in events], ["checkpoint", "campaign_status"])
        self.assertIn("/run/checkpoint/state-4", events[0]["message"])
        self.assertIn("pending +0.22 per completion", events[1]["message"])
        self.assertIn("Discovery/closure 0.943", events[1]["message"])
        # Epoch checkpoint milestones supply a directory, not a state_path.
        frame["checkpoint_milestones"][0]["state_path"] = ""
        frame["checkpoint_milestones"][0]["directory"] = "/run/epoch/checkpoints/main"
        output = io.StringIO()
        DASHBOARD.Presenter(output).render_frame(frame, now=0)
        self.assertIn("/run/epoch/checkpoints/main", json.loads(output.getvalue().splitlines()[0])["message"])

    def test_no_untrusted_ansi_or_nan_in_normalized_frame(self):
        status = sample_status()
        status["run_directory"] = "bad\x1b[31m\nname" * 2000
        status["derived"]["discovery_closure_net_1h"]["per_second"] = float("nan")
        frame = TELEMETRY.normalize_status(status)
        self.assertNotIn("\x1b", frame["run_directory"])
        self.assertLessEqual(len(frame["run_directory"]), 4096)
        json.dumps(frame, allow_nan=False)

    def test_unrepresentable_integer_is_unknown_not_a_presenter_failure(self):
        status = sample_status()
        status["resources"]["aggregate_rss_bytes"] = 10 ** 400
        status["derived"]["completions_per_hour_1h"] = 10 ** 400
        frame = TELEMETRY.normalize_status(status)
        self.assertIsNone(frame["resources"]["aggregate_rss_bytes"])
        self.assertIsNone(frame["rates"]["local_completion"]["per_second"])
        self.assertEqual(TELEMETRY.number(2 ** 64 - 1), 2 ** 64 - 1)
        DASHBOARD.render_table(frame, 99, 24, False)
        output = io.StringIO()
        DASHBOARD.Presenter(output).render_frame(frame, now=0)
        self.assertIn("Rate unknown per hour", json.loads(output.getvalue())["message"])


class DashboardTests(unittest.TestCase):
    def test_exact_indicator_thresholds_and_unknown_neutral(self):
        for kind, cases in {
            "cpu": ((0, "31"), (.4999, "31"), (.5, "33"), (.7499, "33"), (.75, "32"), (1, "32")),
            "pending": ((-1, "32"), (0, "32"), (1, "32"), (1.0001, "33"), (2, "33"), (2.0001, "31")),
            "closure": ((0, "32"), (1, "32"), (1.0001, "33"), (2, "33"), (2.0001, "31")),
        }.items():
            for value, expected in cases:
                self.assertEqual(DASHBOARD.indicator_color(value, kind), expected, (kind, value))
            for unknown in (None, float("nan"), float("inf")):
                self.assertIsNone(DASHBOARD.indicator_color(unknown, kind))

    def test_cpu_colours_at_real_widths_and_no_color(self):
        for width in (80, 100, 140):
            for busy, tint in ((24.9, "31"), (25, "33"), (37.5, "32"), (None, None)):
                status = sample_status()
                status["resources"]["native_busy_cores"] = busy
                frame = TELEMETRY.normalize_status(status)
                for color in (True, False):
                    lines = DASHBOARD.render_table(frame, width, 32, color)
                    self.assertTrue(all(DASHBOARD.cell_width(ANSI.sub("", line)) == width for line in lines))
                    cpu = next(line for line in lines if "Active CPU cores" in line)
                    if color and tint:
                        self.assertIn(f"\x1b[{tint}m", cpu)
                    else:
                        self.assertNotIn("\x1b", cpu)
                    if not color:
                        self.assertNotIn("\x1b", "\n".join(lines))
        class Tty(io.StringIO):
            def isatty(self):
                return True
        output = Tty()
        with patch.dict(os.environ, {"TERM": "xterm", "NO_COLOR": ""}, clear=True):
            presenter = DASHBOARD.Presenter(output)
            presenter.render(sample_status(), now=0)
        self.assertNotRegex(output.getvalue(), r"\x1b\[[0-9;]+m")

    def test_pending_and_ratio_colours_infinity_and_unknown_at_real_widths(self):
        for value, closed, tint in ((0, 10, "32"), (1, 10, "32"), (1.0001, 10, "33"),
                                    (2, 10, "33"), (2.0001, 10, "31"), (3, 0, "31"),
                                    (0, 0, None)):
            status = sample_status()
            status["derived"]["pending_growth_per_completion_1h"] = value
            gap = status["derived"]["discovery_closure_net_1h"]
            discovered = value * 10
            gap.update(discovered_delta=discovered, closed_delta=closed,
                       discovered_per_second=discovered / 600, closed_per_second=closed / 600,
                       per_second=(discovered - closed) / 600)
            frame = TELEMETRY.normalize_status(status)
            # Infinity remains valid strict JSON, including the non-TTY path.
            json.dumps(frame, allow_nan=False)
            expected = "∞" if closed == 0 and discovered else "unknown" if closed == 0 else f"{value:.3f}"
            for width in (80, 100, 140):
                for color in (True, False):
                    with self.subTest(value=value, closed=closed, width=width, color=color):
                        lines = DASHBOARD.render_table(frame, width, 32, color)
                        ratio_line = next(line for line in lines if "Discovery/closure" in line)
                        self.assertIn(f"Discovery/closure {expected}", ratio_line)
                        pending_line = next(line for line in lines if "pending " in line and "per completion" in line)
                        for line, expected_tint in ((ratio_line, tint), (pending_line, DASHBOARD.indicator_color(value, "pending"))):
                            if color and expected_tint:
                                self.assertIn(f"\x1b[{expected_tint}m", line)
                            else:
                                self.assertNotIn("\x1b", line)
            output = io.StringIO()
            DASHBOARD.Presenter(output).render_frame(frame, now=0)
            event = json.loads(output.getvalue())
            self.assertIn(f"Discovery/closure {expected}", event["message"])
            self.assertNotIn("\x1b", output.getvalue())
            self.assertNotIn("Infinity", output.getvalue())
            self.assertIn(f"Discovery/closure {expected}", "\n".join(DASHBOARD.derived_lines(status)))

    def test_saved_normalized_balance_is_never_reinterpreted_as_ratio(self):
        frame = TELEMETRY.normalize_status(sample_status())
        del frame["rates"]["discovery_per_recursive_closure_1h"]
        frame["rates"]["discovery_closure_balance"] = {"value": -.029, "state": "valid"}
        for render in (DASHBOARD.plain_summary, lambda value: DASHBOARD.render_table(value, 140, 32, False)):
            self.assertIn("Discovery/closure 0.943", "\n".join(render(frame)))
            frame["rates"]["discovery_minus_closure"]["state"] = "counter_reset"
            self.assertIn("Discovery/closure unknown", "\n".join(render(frame)))
            frame["rates"]["discovery_minus_closure"]["state"] = "valid"
        frame["closure_snapshot"]["available"] = False
        self.assertIn("Discovery/closure unknown", "\n".join(DASHBOARD.plain_summary(frame)))

    def test_aligned_responsive_colour_table(self):
        frame = TELEMETRY.normalize_status(sample_status())
        for width, height in ((40, 12), (79, 23), (99, 23), (139, 35)):
            for color in (False, True):
                with self.subTest(width=width, height=height, color=color):
                    lines = DASHBOARD.render_table(frame, width, height, color)
                    self.assertLessEqual(len(lines), height)
                    self.assertTrue(all(DASHBOARD.cell_width(ANSI.sub("", line)) == width for line in lines))
                    self.assertEqual(any("\x1b" in line for line in lines), color)
        lines = "\n".join(DASHBOARD.render_table(frame, 139, 35, False))
        for label in ("Local completions", "Recursive closure", "pending +0.22 per completion",
                      "Discovery/closure 0.943", "Closure snapshot stale", "scan advanced", "warm-up"):
            self.assertIn(label, lines)

    def test_real_width_rate_labels_and_warmup_do_not_clip(self):
        status = sample_status()
        status["derived"]["window_wall_seconds"] = 11
        status["derived"]["discovery_closure_net_1h"]["covered_seconds"] = 11
        frame = TELEMETRY.normalize_status(status)
        for width in (79, 99, 139):
            lines = DASHBOARD.render_table(frame, width, 31, True)
            text = "\n".join(lines)
            self.assertIn("\x1b[94m", text)
            self.assertNotIn("\x1b[34m", text)
            for label in ("Local completions", "Recursive closure"):
                line = next(line for line in lines if label in line)
                self.assertNotIn("…", line)
                self.assertIn("warm-up", line)
            self.assertIn("scan-batched", text)

    def test_tty_overwrites_resize_and_plain_is_json(self):
        class Tty(io.StringIO):
            def isatty(self):
                return True
        output = Tty()
        with patch.dict(os.environ, {"TERM": "xterm"}, clear=True):
            presenter = DASHBOARD.Presenter(output)
            with patch.object(DASHBOARD.shutil, "get_terminal_size", return_value=os.terminal_size((100, 24))):
                presenter.render(sample_status(), now=0)
            with patch.object(DASHBOARD.shutil, "get_terminal_size", return_value=os.terminal_size((80, 18))):
                presenter.render(sample_status(), now=2)
        self.assertIn("\x1b[23A\r\x1b[J", output.getvalue())
        self.assertEqual(presenter.drawn, 17)
        plain = io.StringIO()
        DASHBOARD.Presenter(plain).render(sample_status(), now=0)
        event = json.loads(plain.getvalue())
        self.assertEqual(event["event"], "campaign_status")
        self.assertEqual(event["telemetry"]["schema"], TELEMETRY.SCHEMA)
        self.assertNotIn("\x1b", plain.getvalue())

    def test_stale_unresolved_count_is_an_upper_bound(self):
        status = sample_status()
        text = "\n".join(DASHBOARD.render_table(TELEMETRY.normalize_status(status), 160, 35, False))
        self.assertIn("≥1,440,000 closed · ≤5,060,000 unresolved", text)
        status["progress"]["descendant_closure"]["snapshot_stale"] = False
        text = "\n".join(DASHBOARD.render_table(TELEMETRY.normalize_status(status), 160, 35, False))
        self.assertIn("1,440,000 closed · 5,060,000 unresolved", text)
        self.assertNotIn("≤", text)

    def test_plain_rate_does_not_render_a_counter_reset_as_negative_work(self):
        metrics = METRICS.HeartbeatWindow()
        for elapsed, completed in ((100, 100), (110, 5)):
            metrics.observe({"event": "heartbeat", "elapsed_seconds": elapsed,
                             "progress": {"completed_nodes": completed, "queued_nodes": 1}})
        frame = TELEMETRY.normalize_status({"derived": metrics.derived()})
        self.assertEqual(frame["rates"]["completions_per_hour_1h"], -34200)
        self.assertEqual(frame["rates"]["local_completion"]["state"], "counter_reset")
        self.assertEqual(DASHBOARD.plain_summary(frame)[1],
                         "Rate unknown per hour · pending unknown per completion")

    def test_unicode_and_stale_alarm_have_no_layout_escape(self):
        self.assertEqual(DASHBOARD.cell_width(DASHBOARD.fit("中文e\u0301\x1b\n", 8)), 8)
        status = sample_status()
        status["heartbeat_stale"] = True
        for height in (12, 24):
            text = "\n".join(DASHBOARD.render_table(TELEMETRY.normalize_status(status), 99, height, False))
            self.assertIn("STALE HEARTBEAT", text)

    def test_standalone_copied_monitor_imports_without_repository_search_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary)
            for name in ("campaign_monitor.py", "campaign_dashboard.py", "campaign_telemetry.py", "plot_campaign_rates.py"):
                shutil.copyfile(Path(__file__).with_name(name), destination / name)
            for command in ("campaign_monitor.py", "plot_campaign_rates.py"):
                result = subprocess.run([sys.executable, "-I", "-B", str(destination / command), "--help"],
                                        cwd=destination, capture_output=True, timeout=10)
                self.assertEqual(result.returncode, 0, result.stderr.decode())


class PlotTests(unittest.TestCase):
    def test_stale_graph_is_dashed_but_missing_heartbeat_is_a_gap(self):
        frame = TELEMETRY.normalize_status(sample_status())
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "telemetry.jsonl"
            missing = json.loads(json.dumps(frame))
            missing["heartbeat_stale"] = True
            missing["elapsed_seconds"] += 2
            path.write_text(json.dumps(frame) + "\n" + json.dumps(missing) + "\n")
            points, metadata = PLOT.load_points(path)
            self.assertEqual(points[0]["closure"], 212)
            self.assertEqual(points[0]["unresolved"], 5_060_000)
            self.assertEqual(points[0]["gap"], -12)
            self.assertIsNone(points[1]["closure"])
            self.assertIsNone(points[1]["completion"])
            self.assertIsNone(points[1]["unresolved"])
            self.assertIsNone(points[1]["gap"])
            svg = PLOT.make_svg(points, metadata, title="Test <&>")
            ET.fromstring(svg)
            self.assertIn("Test &lt;&amp;&gt;", svg)
            self.assertIn("graph-dirty closure snapshot", svg)
            self.assertIn("not proof", svg)
            self.assertIn("Seconds since plotted interval start", svg)
            self.assertIn(">2.00</text>", svg)
            self.assertIn("Unresolved domains (total)", svg)
            self.assertIn("Discovery − closure (domains / second)", svg)
            self.assertIn("RIGHT: raw net rate, domains/s", svg)
            self.assertIn("no interpolation", svg)
            self.assertNotIn("<path", svg)

    def test_empty_ratio_window_keeps_a_real_zero_raw_net_rate(self):
        frame = TELEMETRY.normalize_status(sample_status())
        gap = frame["rates"]["discovery_minus_closure"]
        gap.update(discovered_delta=0, closed_delta=0, per_second=0,
                   discovered_per_second=0, closed_per_second=0)
        with patch.object(PLOT, "records", return_value=iter([frame])):
            point = next(PLOT.observations("unused"))
        self.assertEqual(point["gap"], 0)
        self.assertEqual(point["unresolved"], 5_060_000)

    def test_infinite_ratio_keeps_a_real_positive_raw_net_rate(self):
        frame = TELEMETRY.normalize_status(sample_status())
        gap = frame["rates"]["discovery_minus_closure"]
        gap.update(discovered_delta=1200, closed_delta=0, per_second=2,
                   discovered_per_second=2, closed_per_second=0)
        # Plotting is derived from the raw window, not either cached ratio.
        frame["rates"]["discovery_closure_balance"] = {"value": 1}
        with patch.object(PLOT, "records", return_value=iter([frame])):
            point = next(PLOT.observations("unused"))
        self.assertEqual(point["gap"], 2)

    def test_plot_keeps_negative_net_and_gaps_on_mismatch_reset_and_counter_restart(self):
        frames = [TELEMETRY.normalize_status(sample_status()) for _ in range(4)]
        for index, frame in enumerate(frames):
            frame["elapsed_seconds"] += index * 10
            gap = frame["rates"]["discovery_minus_closure"]
            gap["first_elapsed_seconds"] += index * 10
            gap["last_elapsed_seconds"] += index * 10
        frames[1]["rates"]["discovery_minus_closure"]["covered_seconds"] = 1
        frames[2]["rates"]["discovery_minus_closure"]["state"] = "counter_reset"
        frames[3]["elapsed_seconds"] = 1
        with patch.object(PLOT, "records", return_value=iter(frames)):
            points = list(PLOT.observations("unused"))
        self.assertEqual(points[0]["gap"], -12)
        self.assertIsNone(points[1]["gap"])
        self.assertIsNone(points[2]["gap"])
        self.assertIsNone(points[3])

    def test_same_endpoint_expiring_closure_snapshot_breaks_plot_rate(self):
        first = TELEMETRY.normalize_status(sample_status())
        first["closure_snapshot"]["snapshot_age_seconds"] = 599
        second = json.loads(json.dumps(first))
        second["elapsed_seconds"] += 1
        second["closure_snapshot"]["snapshot_age_seconds"] = 600
        with patch.object(PLOT, "records", return_value=iter([first, second])):
            points = list(PLOT.observations("unused"))
        self.assertEqual(len(points), 2)
        self.assertEqual(points[0]["gap"], -12)
        self.assertIsNone(points[1]["gap"])
        self.assertIsNone(points[1]["closure"])
        self.assertEqual(points[1]["completion"], 1500)

    def test_repeated_poll_endpoint_does_not_create_a_measurement(self):
        frame = TELEMETRY.normalize_status(sample_status())
        duplicate = json.loads(json.dumps(frame))
        duplicate["elapsed_seconds"] += 2
        with patch.object(PLOT, "records", return_value=iter([frame, duplicate])):
            self.assertEqual(len(list(PLOT.observations("unused"))), 1)

    def test_bounded_reader_rejects_oversize_and_torn_tail(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "telemetry.jsonl"
            path.write_bytes(b"x" * (TELEMETRY.MAX_FRAME_BYTES + 20) + b"\ninvalid\n" +
                             json.dumps(TELEMETRY.normalize_status(sample_status())).encode() + b"\n{partial")
            records = list(PLOT.records(path))
            self.assertEqual(len(records), 3)
            self.assertEqual(records[:2], [None, None])

    def test_extrema_accumulator_memory_does_not_scale_with_bucket_length(self):
        def generated(*args, **kwargs):
            for index in range(30000):
                yield {"elapsed": index, "completion": index % 41, "closure": index % 23, "gap": 1,
                       "unresolved": index % 41,
                       "closure_stale": True, "snapshot_age_seconds": 1, "scan_advanced": False, "warmup": False}
        with patch.object(PLOT, "observations", side_effect=generated):
            tracemalloc.start()
            points, metadata = PLOT.load_points("unused", max_points=16)
            _, peak = tracemalloc.get_traced_memory()
            tracemalloc.stop()
        self.assertLessEqual(len(points), 16)
        self.assertEqual(metadata["records"], 30000)
        self.assertLess(peak, 1_000_000)
        self.assertEqual(max(point["unresolved"] for point in points), 40)

    def test_svg_cli_and_input_overwrite_refusal(self):
        with tempfile.TemporaryDirectory() as temporary:
            path, output = Path(temporary) / "telemetry.jsonl", Path(temporary) / "rates.svg"
            path.write_text(json.dumps(TELEMETRY.normalize_status(sample_status())) + "\n")
            self.assertEqual(PLOT.main([str(path), "--output", str(output)]), 0)
            ET.parse(output)
            with self.assertRaises(SystemExit), patch("sys.stderr", io.StringIO()):
                PLOT.main([str(output), "--output", str(output)])

    def test_svg_cli_rejects_hardlink_to_telemetry(self):
        with tempfile.TemporaryDirectory() as temporary:
            path, output = Path(temporary) / "telemetry.jsonl", Path(temporary) / "rates.svg"
            original = (json.dumps(TELEMETRY.normalize_status(sample_status())) + "\n").encode()
            path.write_bytes(original)
            os.link(path, output)
            with self.assertRaises(SystemExit), patch("sys.stderr", io.StringIO()):
                PLOT.main([str(path), "--output", str(output)])
            self.assertEqual(path.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
