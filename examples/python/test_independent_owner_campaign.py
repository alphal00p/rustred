"""No native launch: contract checks for the thin independent-campaign adapter."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "independent_owner_campaign", Path(__file__).with_name("independent_owner_campaign.py"))
MOD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MOD)


class Steering(unittest.TestCase):
    def run_main(self, args):
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return MOD.main(args)

    def test_preparation_is_generic_and_does_not_launch(self):
        with tempfile.TemporaryDirectory() as root, patch.object(MOD.os, "execv") as execute:
            base = Path(root)
            config = base / "input.json"
            self.assertEqual(self.run_main([
                "--directory", str(base / "run"), "--manifest", str(base / "selection.json"),
                "--queries", str(base / "queries.json"), "--owner-base", str(base),
                "--config-output", str(config), "--shards", "2", "--jobs", "2",
                "--workers-per-job", "3", "--total-workers", "6", "--cpus", "50,51,52,53,54,55",
                "--route-joint-source-support-pruning",
            ]), 0)
            document = json.loads(config.read_text())
            self.assertEqual(document["jobs"], 2)
            self.assertEqual(document["cpus"], list(range(50, 56)))
            self.assertIn("--route-joint-source-support-pruning", document["native_options"])
            self.assertFalse((base / "run").exists())
            execute.assert_not_called()

    def test_resume_is_exec_not_python_supervision(self):
        with patch.object(MOD.os, "execv") as execute:
            self.run_main(["--directory", "TMP/never-launched", "--resume", "--start"])
            command = execute.call_args.args[1]
            self.assertEqual(command[1:3], ["campaign", "shards"])
            self.assertEqual(command[-1], "--resume")
            self.assertNotIn("--config", command)

    def test_default_queues_individual_owners_and_has_no_500gb_ceiling(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            config = base / "input.json"
            self.run_main([
                "--directory", str(base / "run"), "--manifest", "selection.json",
                "--queries", "queries.json", "--owner-base", ".",
                "--config-output", str(config), "--max-memory-bytes", "700000000000",
                "--cpus", ",".join(map(str, range(50))),
            ])
            document = json.loads(config.read_text())
            self.assertNotIn("shards", document)
            self.assertEqual((document["jobs"], document["workers_per_job"]), (10, 5))
            self.assertEqual(document["max_memory_bytes"], 700_000_000_000)

    def test_resume_cannot_replace_inputs(self):
        with self.assertRaises(SystemExit):
            self.run_main(["--directory", "TMP/no-run", "--resume", "--queries", "changed.json"])

    def test_resume_cannot_silently_ignore_policy(self):
        for option in (["--jobs", "2"], ["--max-memory-bytes", "700000000000"],
                       ["--route-joint-source-support-pruning"]):
            with self.subTest(option=option), self.assertRaises(SystemExit):
                self.run_main(["--directory", "TMP/no-run", "--resume", *option])

    def test_worker_oversubscription_rejected_before_writing(self):
        with self.assertRaises(SystemExit):
            self.run_main(["--directory", "TMP/no-run", "--manifest", "m", "--queries", "q",
                           "--owner-base", ".", "--total-workers", "2"])


if __name__ == "__main__":
    unittest.main()
