"""Owned stop-file behavior with short real children, never a native campaign."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("tested_generation_guard", Path(__file__).with_name("generation_guard.py"))
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class StopGuard:
    def __init__(self, *args, **kwargs):
        pass

    def policy(self):
        return {"test_only": True}

    def observe(self, *args):
        return {"hard": None, "cooperative": "test_cooperative_stop"}


class ObserveGuard(StopGuard):
    def observe(self, *args):
        return {"hard": None, "cooperative": None}


class NativeStopFileTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir=GUARD.ROOT / "TMP", prefix="guard-stop-test-")
        self.addCleanup(self.temporary.cleanup)
        self.folder = Path(self.temporary.name)
        cpu = min(os.sched_getaffinity(0))
        self.resources = {"cpus": str(cpu), "workers": 1, "max_memory_bytes": 1_000_000_000,
                          "host_memory_reserve_bytes": 1_000_000_000, "ram_guard_margin_percent": 5,
                          "swap_growth_stop_bytes_per_second": 0}

    def test_stop_file_allows_child_to_acknowledge_without_sigterm_handler(self):
        stop = self.folder / "stop.json"
        ack = self.folder / "checkpoint-ack.txt"
        code = "import pathlib,time,sys; p=pathlib.Path(sys.argv[1]);\nwhile not p.exists(): time.sleep(.01)\npathlib.Path(sys.argv[2]).write_text('durable fake prefix')\n"
        with patch.object(GUARD.SUPERVISOR, "RamGuard", StopGuard):
            with self.assertRaises(RuntimeError):
                GUARD.run([sys.executable, "-B", "-c", code, str(stop), str(ack)],
                          self.folder / "guard", self.resources, poll_seconds=.02,
                          grace_seconds=2, cooperative_stop_file=stop)
        self.assertEqual(ack.read_text(), "durable fake prefix")
        result = json.loads((self.folder / "guard/result.json").read_text())
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(result["stop_reason"], "test_cooperative_stop")
        self.assertTrue(result["owned_group_drained"])
        self.assertEqual(json.loads(stop.read_text())["reason"], "test_cooperative_stop")
        self.assertIsNone(result["completed_sector_checkpoint_only"])
        self.assertIsNone(result["in_sector_resume"])
        self.assertTrue(result["checkpoint_evidence_required"])
        self.assertFalse(result["checkpoint_durability_claim"])
        self.assertEqual(result["stop_mechanism"], "native_stop_file_then_owned_group_cleanup")

    def test_unresponsive_child_is_killed_only_after_owned_stop_grace(self):
        stop = self.folder / "stop.json"
        with patch.object(GUARD.SUPERVISOR, "RamGuard", StopGuard):
            with self.assertRaises(RuntimeError):
                GUARD.run([sys.executable, "-B", "-c", "import time;time.sleep(30)"],
                          self.folder / "guard", self.resources, poll_seconds=.02,
                          grace_seconds=.2, cooperative_stop_file=stop)
        result = json.loads((self.folder / "guard/result.json").read_text())
        self.assertEqual(result["exit_code"], -signal.SIGKILL)
        self.assertGreaterEqual(result["elapsed_seconds"], .2)
        self.assertTrue(result["owned_group_drained"])
        self.assertTrue(stop.is_file())

    def test_operator_or_deadline_signal_requests_the_native_stop_file_first(self):
        stop = self.folder / "stop.json"
        ack = self.folder / "checkpoint-ack.txt"
        # The child signals only its own guard parent after that guard has
        # installed its handlers. This is the same handler used by pilot timers.
        code = "import os,signal,pathlib,time,sys; p=pathlib.Path(sys.argv[1]);os.kill(os.getppid(),signal.SIGTERM)\nwhile not p.exists(): time.sleep(.01)\npathlib.Path(sys.argv[2]).write_text('saved after signal')\n"
        with patch.object(GUARD.SUPERVISOR, "RamGuard", ObserveGuard):
            with self.assertRaises(RuntimeError):
                GUARD.run([sys.executable, "-B", "-c", code, str(stop), str(ack)],
                          self.folder / "guard", self.resources, poll_seconds=.02,
                          grace_seconds=2, cooperative_stop_file=stop)
        self.assertEqual(ack.read_text(), "saved after signal")
        result = json.loads((self.folder / "guard/result.json").read_text())
        self.assertEqual(result["exit_code"], 0)
        self.assertEqual(result["stop_reason"], f"operator_signal_{signal.SIGTERM}")
        self.assertTrue(result["owned_group_drained"])

    def test_default_generation_stop_keeps_sigterm_behavior(self):
        with patch.object(GUARD.SUPERVISOR, "RamGuard", StopGuard):
            with self.assertRaises(RuntimeError):
                GUARD.run([sys.executable, "-B", "-c", "import time;time.sleep(30)"],
                          self.folder / "guard", self.resources, poll_seconds=.02,
                          grace_seconds=2)
        result = json.loads((self.folder / "guard/result.json").read_text())
        self.assertEqual(result["exit_code"], -signal.SIGTERM)
        self.assertTrue(result["owned_group_drained"])
        self.assertTrue(result["completed_sector_checkpoint_only"])
        self.assertFalse(result["in_sector_resume"])
        self.assertNotIn("checkpoint_evidence_required", result)

    def test_unrelated_stop_path_is_rejected_before_child_launch(self):
        with self.assertRaisesRegex(ValueError, "local to this owned job"):
            GUARD.run([sys.executable, "-B", "-c", "raise RuntimeError('must not start')"],
                      self.folder / "guard", self.resources,
                      cooperative_stop_file=GUARD.ROOT / "campaigns/never-write-this-stop")
        result = json.loads((self.folder / "guard/result.json").read_text())
        self.assertFalse(result["child_started"])
        self.assertFalse((GUARD.ROOT / "campaigns/never-write-this-stop").exists())


if __name__ == "__main__":
    unittest.main()
