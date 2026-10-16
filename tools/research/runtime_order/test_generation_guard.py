"""Owned stop-file behavior with short real children, never a native campaign."""
import importlib.util
from contextlib import redirect_stdout
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

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

    def test_terminal_passthrough_requires_opt_in_and_preserves_non_tty_logging(self):
        for requested, terminal, inherited in [(False, True, False), (True, False, False), (True, True, True)]:
            with self.subTest(requested=requested, terminal=terminal):
                directory = self.folder / f"mode-{requested}-{terminal}"
                child = Mock(pid=999999999)
                child.poll.side_effect = [None, 0, 0]
                child.wait.return_value = 0
                output = io.StringIO()
                with patch.object(GUARD.SUPERVISOR, "RamGuard", ObserveGuard), \
                     patch.object(GUARD.subprocess, "Popen", return_value=child) as popen, \
                     patch.object(GUARD, "group_running", return_value=False), \
                     patch.object(GUARD.os, "isatty", return_value=terminal), \
                     redirect_stdout(output):
                    options = {"native_tty_progress":True} if requested else {}
                    result = GUARD.run(["mock-native"], directory, self.resources,
                        poll_seconds=0, **options)
                self.assertEqual(popen.call_args.kwargs["stderr"] is None, inherited)
                self.assertEqual("Generation/admission" in output.getvalue(), not inherited)
                request = json.loads((directory / "request.json").read_text())
                self.assertEqual(request["native_stderr"], "inherited_tty" if inherited else "captured_file")
                self.assertTrue(result["owned_group_drained"])
                self.assertTrue((directory / "resources.jsonl").read_text())
                self.assertTrue((directory / "stderr").is_file())

    @unittest.skipUnless(os.name == "posix", "PTY requires POSIX")
    def test_opted_in_child_sees_real_tty_without_guard_output_competing(self):
        import pty
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        self.addCleanup(os.close, slave)
        directory = self.folder / "pty-guard"
        # A tiny Python child stands in for the CLI: no Rust build or campaign.
        # The outer process gets a real stderr PTY, so both isatty decisions
        # exercise actual descriptors rather than a mocked terminal predicate.
        runner = """import importlib.util,json,sys
from unittest.mock import patch
spec=importlib.util.spec_from_file_location('test_guard',sys.argv[1])
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
code=\"import os,sys,time;print('NATIVE_TTY='+str(os.isatty(2)),file=sys.stderr,flush=True);time.sleep(.1)\"
with patch.object(m.GUARD.SUPERVISOR,'RamGuard',m.ObserveGuard):
 m.GUARD.run([sys.executable,'-B','-c',code],sys.argv[2],json.loads(sys.argv[3]),poll_seconds=.02,native_tty_progress=True)
"""
        completed = subprocess.run([sys.executable, "-B", "-c", runner, __file__, str(directory),
            json.dumps(self.resources)], stdout=subprocess.PIPE, stderr=slave, timeout=15)
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(completed.stdout, b"")
        os.set_blocking(master, False)
        displayed = os.read(master, 65536)
        self.assertIn(b"NATIVE_TTY=True", displayed)
        self.assertNotIn(b"Generation/admission", displayed)
        self.assertEqual((directory / "stderr").read_bytes(), b"")
        self.assertTrue((directory / "resources.jsonl").read_text())
        self.assertTrue(json.loads((directory / "result.json").read_text())["owned_group_drained"])


if __name__ == "__main__":
    unittest.main()
