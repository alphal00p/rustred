"""Command-template validation only; these tests never launch native work."""
import json
import tempfile
import unittest
from pathlib import Path

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


if __name__ == "__main__":
    unittest.main()
