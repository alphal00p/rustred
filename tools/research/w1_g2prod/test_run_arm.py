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


if __name__ == "__main__":
    unittest.main()
