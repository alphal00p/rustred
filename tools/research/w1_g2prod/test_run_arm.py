"""Command-template validation only; these tests never launch native work."""
import json
import io
import os
from contextlib import redirect_stdout, redirect_stderr
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import run_arm


class CommandTemplateTests(unittest.TestCase):
    def test_explicit_template_keeps_inputs_and_rewrites_only_runtime_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "command.json"
            argv = ["old-binary", "walk-owner-domains", "--manifest", "/fixture/v6/selection.json",
                    "--queries", "/fixture/v6/queries.json", "--owner-base", "/fixture/v6/owners",
                    "--checkpoint", "/old/checkpoint", "--output", "/old/result.json",
                    "--events", "/old/events.jsonl", "--stop-file", "/old/stop.json"]
            path.write_text(json.dumps(argv))
            before = path.read_bytes()
            loaded = run_arm.command_template(path)
            out = Path(directory) / "fresh"
            rewritten = run_arm.run_control.rewrite(loaded, "new-binary", out, "ordered", 6,
                                                    None, ["--g2-residual-anchors", "union"], False)
            self.assertEqual(path.read_bytes(), before)
            self.assertEqual(loaded, argv)
            for flag in ("--manifest", "--queries", "--owner-base"):
                self.assertEqual(rewritten[rewritten.index(flag) + 1], argv[argv.index(flag) + 1])
            for flag, name in (("--checkpoint", "checkpoint"), ("--output", "result.json"),
                               ("--events", "events.jsonl"), ("--stop-file", "stop-request.json")):
                self.assertEqual(rewritten[rewritten.index(flag) + 1], str(out / name))
            self.assertEqual(run_arm.configured_workers(rewritten), 6)

    def test_worker_metadata_uses_explicit_template_budget(self):
        self.assertEqual(run_arm.configured_workers(["binary", "--workers", "16"]), 16)
        for argv in (["binary"], ["--workers", "auto"], ["--workers", "0"], ["--workers"]):
            self.assertIsNone(run_arm.configured_workers(argv))

    def test_rejects_non_argv_templates(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "command.json"
            for value in (None, [], {}, ["binary", 1], "binary"):
                with self.subTest(value=value):
                    path.write_text(json.dumps(value))
                    with self.assertRaises(ValueError):
                        run_arm.command_template(path)

    def test_cli_guard_refuses_low_headroom_and_flag_off_adds_no_native_flag(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            template = base / "command.json"
            template.write_text(json.dumps(["binary", "owner-domain-match", "--manifest", "/v6/manifest",
                "--queries", "/v6/queries", "--owner-base", "/v6/owners", "--workers", "1"]))
            cpu = next(cpu for cpu in os.sched_getaffinity(0) if not 128 <= cpu <= 227)
            args = ["runner", "--binary", "never-launched", "--family", "fg", "--label", "guard",
                    "--command", str(template), "--cpus", str(cpu), "--out-root", str(base / "out"),
                    "--heavy-lock", str(base / "lock"), "--minimum-start-headroom-gib", "250",
                    "--minimum-headroom-gib", "150", "--env", "SYMBOLICA_LICENSE=not-a-real-license"]
            with patch("sys.argv", args), patch.object(run_arm, "mem_available", return_value=100 * run_arm.GIB), \
                    patch.object(run_arm.subprocess, "Popen") as launch, \
                    patch.object(run_arm.os, "sched_setaffinity") as affinity, redirect_stdout(io.StringIO()):
                self.assertEqual(run_arm.main(), 1)
            launch.assert_not_called()
            affinity.assert_called_once_with(0, [cpu])
            output = base / "out" / "guard" / "fg"
            metrics = json.loads((output / "metrics.json").read_text())
            self.assertEqual(metrics["stop_reason"], "insufficient_start_headroom")
            self.assertTrue(metrics["censored"])
            self.assertFalse(metrics["resource_guard"]["child_started"])
            self.assertEqual(metrics["env"]["SYMBOLICA_LICENSE"], "<redacted>")
            argv = json.loads((output / "command.json").read_text())
            self.assertNotIn("--g2-residual-anchors", argv)
            self.assertEqual(argv[argv.index("--owner-base") + 1], "/v6/owners")
            contaminated = json.loads(template.read_text()) + ["--g2-residual-anchors", "union"]
            template.write_text(json.dumps(contaminated))
            with patch("sys.argv", args), redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as raised:
                run_arm.main()
            self.assertEqual(raised.exception.code, 2)


class BoundedRootMetricTests(unittest.TestCase):
    def extract(self, text, head=None, tail=None):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "result.json"
            path.write_text(text)
            with patch.object(run_arm, "EXTRACT_HEAD_BYTES", head or run_arm.EXTRACT_HEAD_BYTES), \
                    patch.object(run_arm, "EXTRACT_TAIL_BYTES", tail or run_arm.EXTRACT_TAIL_BYTES):
                return run_arm.extract(path)

    def test_all_root_metrics_ignore_nested_same_name_fields(self):
        values = {key: index + 10 for index, key in enumerate(run_arm.ROOT_METRICS)}
        values.update(status="locally_resolved", events=500, successors=400,
                      g2_residual_anchors={"mode": "union"}, g2_index_telemetry={"entry_visits": 20},
                      delegation={"native_discharged": 12, "delegated_publications": 3, "transferred_obligations": 3})
        doc = {"domains": [{"stats": {key: "wrong nested value" for key in values}}], **values,
               "descendant_closure": {"available": True, "initial_closed": 2, "initial_total": 2,
                                      "total_closed": 15, "total_domains": 15, "unresolved_domains": 0, "dependency_edges": 20},
               "parallel": {"slot_busy_seconds": [1.25, 2.5], "slot_backpressure_seconds": [0, 0]}}
        found = self.extract(json.dumps(doc, indent=2))
        for key, value in values.items():
            self.assertEqual(found[key], value)
        self.assertEqual(found["initial_closed"], 2)
        self.assertEqual(found["native_discharged"], 12)
        self.assertEqual(found["slot_busy_seconds_sum"], 3.75)
        self.assertEqual(found["inspection_slots"], 2)
        self.assertEqual(self.extract(json.dumps({"domains": [{"stats": values}]}, indent=2)), {})

    def test_head_tail_windows_and_all_reads_are_bounded(self):
        text = json.dumps({"events": 50, "padding": "x" * 4096, "status": "locally_resolved"}, indent=2)
        calls = []
        class Reads(io.BytesIO):
            def read(self, size=-1):
                calls.append(size)
                return super().read(size)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "result.json"
            path.write_text(text)
            with patch.object(Path, "open", return_value=Reads(text.encode())), \
                    patch.object(run_arm, "EXTRACT_HEAD_BYTES", 128), \
                    patch.object(run_arm, "EXTRACT_TAIL_BYTES", 128):
                self.assertEqual(run_arm.extract(path), {"events": 50, "status": "locally_resolved"})
        self.assertEqual(calls, [128, 128])

    def test_clipped_scalar_and_partial_objects_remain_unknown(self):
        text = json.dumps({"events": 1234567890, "padding": "x" * 4096, "status": "locally_resolved"}, indent=2)
        found = self.extract(text, head=text.index("1234567890") + 4, tail=80)
        self.assertNotIn("events", found)
        self.assertEqual(found["status"], "locally_resolved")
        partial = json.dumps({"parallel": {"slot_busy_seconds": [999], "padding": "x" * 4096},
                              "status": "locally_resolved"}, indent=2)
        found = self.extract(partial, head=128, tail=80)
        self.assertNotIn("slot_busy_seconds_sum", found)
        self.assertEqual(found["status"], "locally_resolved")

    def test_tail_clipped_indentation_never_promotes_nested_field(self):
        text = json.dumps({"padding": "x" * 1024, "domains": [{"events": 999}],
                           "status": "locally_resolved"}, indent=2)
        offset = text.index('"events"') - 2
        found = self.extract(text, head=64, tail=len(text.encode()) - offset)
        self.assertNotIn("events", found)
        self.assertEqual(found["status"], "locally_resolved")

    def test_malformed_or_conflicting_root_fields_are_unknown(self):
        self.assertEqual(self.extract('{\n  "events": 1,\n  "events": 2,\n  "status": "broken'), {})


if __name__ == "__main__":
    unittest.main()
