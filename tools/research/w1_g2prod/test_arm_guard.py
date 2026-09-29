"""Owned Python mock processes only; no native solver or shared heavy lock."""
import fcntl
import itertools
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import run_arm


def wait_for(path, process, timeout=5):
    deadline = time.monotonic() + timeout
    while not path.exists() and process.poll() is None and time.monotonic() < deadline:
        time.sleep(0.02)
    if not path.exists():
        raise AssertionError(f"owned mock did not publish {path.name}")


def driver(base, command, grace=0.8):
    code = f"""
import json,sys
from pathlib import Path
sys.path.insert(0, {str(Path(run_arm.__file__).parent)!r})
import run_arm
base = Path({str(base)!r})
g = run_arm.ArmGuard(base/'stop', [base/'lock'], grace={grace})
result = 125
with g:
    (base/'waiting').touch()
    if g.acquire():
        child = g.launch({command!r})
        (base/'child.json').write_text(json.dumps({{'pid': child.pid}}))
        result = g.wait()
(base/'result.json').write_text(json.dumps(dict(g.receipt(), exit_code=result, killed=g.killed)))
"""
    return subprocess.Popen([sys.executable, "-B", "-c", code],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)


class ArmGuardTests(unittest.TestCase):
    def test_unreadable_process_is_not_false_group_drain(self):
        path = Mock()
        path.name = "123"
        path.__truediv__ = Mock(return_value=path)
        for failure in (PermissionError("unreadable"), ValueError("malformed")):
            path.read_text.side_effect = failure
            with patch.object(Path, "iterdir", return_value=iter([path])), \
                    patch.object(run_arm, "group_exists", return_value=True) as exists:
                self.assertTrue(run_arm.group_running(999))
                exists.assert_called_once_with(999)
        path.read_text.side_effect = FileNotFoundError()
        with patch.object(Path, "iterdir", return_value=iter([path])), \
                patch.object(run_arm, "group_exists") as exists:
            self.assertFalse(run_arm.group_running(999))
            exists.assert_not_called()

    def test_partial_recorder_start_failure_stops_and_joins_first_thread(self):
        stop = threading.Event()
        first = threading.Thread(target=lambda: stop.wait(5))
        broken = Mock()
        broken.start.side_effect = RuntimeError("start failed")
        guard = Mock()
        with self.assertRaisesRegex(RuntimeError, "start failed"):
            run_arm.wait_with_recorders(guard, stop, [first, broken])
        self.assertTrue(stop.is_set())
        self.assertFalse(first.is_alive())
        guard.wait.assert_not_called()

    def test_cpu_deltas_and_single_child_rss_are_separate_metrics(self):
        before = SimpleNamespace(ru_utime=3.0, ru_stime=1.0, ru_maxrss=50)
        after = SimpleNamespace(ru_utime=7.5, ru_stime=2.5, ru_maxrss=100)
        usage = run_arm.waited_child_usage(before, after)
        self.assertEqual(usage["child_user_seconds"], 4.5)
        self.assertEqual(usage["child_system_seconds"], 1.5)
        self.assertEqual(usage["maximum_single_waited_child_rss_bytes"], 102400)
        self.assertIn("not concurrent aggregate", usage["single_child_rss_scope"])

    def test_low_start_and_failed_memory_read_never_start_child(self):
        for memory in (lambda: 1, lambda: (_ for _ in ()).throw(OSError("missing"))):
            with self.subTest(memory=memory), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                with patch.object(run_arm, "mem_available", side_effect=memory):
                    with run_arm.ArmGuard(base / "stop", [base / "lock"], 250, 150) as guard:
                        self.assertFalse(guard.acquire())
                        self.assertIsNone(guard.launch([sys.executable, "-c", "raise Exception('not launched')"]))
                    self.assertFalse(guard.receipt()["child_started"])
                    self.assertIsNotNone(guard.reason)

    def test_runtime_low_memory_cooperatively_stops_even_if_child_returns_zero(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            code = ("import time; from pathlib import Path; "
                    f"p=Path({str(base / 'stop')!r}); "
                    "\nwhile not p.exists(): time.sleep(.01)\n")
            with patch.object(run_arm, "mem_available", side_effect=itertools.chain([300], itertools.repeat(100))):
                with run_arm.ArmGuard(base / "stop", [base / "lock"], 250, 150, grace=1) as guard:
                    self.assertTrue(guard.acquire())
                    guard.launch([sys.executable, "-c", code])
                    self.assertEqual(guard.wait(), 0)
            self.assertEqual(guard.reason, "host_headroom_below_minimum")
            self.assertFalse(guard.killed)
            self.assertEqual(guard.receipt()["minimum_mem_available_bytes"], 100)

    def test_memory_monitor_failure_during_run_is_a_stop(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            code = f"import time; from pathlib import Path\nwhile not Path({str(base/'stop')!r}).exists(): time.sleep(.01)"
            with patch.object(run_arm, "mem_available", side_effect=itertools.chain([300], itertools.repeat(OSError("lost")))):
                with run_arm.ArmGuard(base / "stop", minimum_start=250, minimum_run=150, grace=1) as guard:
                    self.assertTrue(guard.acquire())
                    guard.launch([sys.executable, "-c", code])
                    self.assertEqual(guard.wait(), 0)
            self.assertEqual(guard.reason, "memory_monitor_unavailable")

    def test_cancel_during_lock_wait_never_launches_later(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            command = [sys.executable, "-c", f"from pathlib import Path; Path({str(base/'bad')!r}).touch()"]
            with (base / "lock").open("a") as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                proc = driver(base, command)
                try:
                    wait_for(base / "waiting", proc)
                    proc.send_signal(signal.SIGTERM)
                    stdout, stderr = proc.communicate(timeout=5)
                    self.assertEqual(proc.returncode, 0, (stdout, stderr))
                    result = json.loads((base / "result.json").read_text())
                    self.assertEqual(result["stop_reason"], "operator_signal_15")
                    self.assertFalse(result["child_started"])
                    self.assertFalse((base / "bad").exists())
                finally:
                    if proc.poll() is None:
                        proc.kill()
                        proc.wait()

    def test_exited_launcher_keeps_lock_until_term_resistant_new_session_group_drains(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            child = ("import signal,time; from pathlib import Path; "
                     "signal.signal(signal.SIGTERM,signal.SIG_IGN); "
                     f"Path({str(base/'ready')!r}).touch(); time.sleep(30)")
            leader = (f"import subprocess,sys,time; subprocess.Popen([sys.executable,'-c',{child!r}]); "
                      "time.sleep(.2)")
            proc = driver(base, [sys.executable, "-c", leader])
            group = None
            try:
                wait_for(base / "ready", proc)
                group = json.loads((base / "child.json").read_text())["pid"]
                self.assertNotEqual(group, os.getpgrp(), "solver must have escaped outer process group")
                time.sleep(.4)
                self.assertTrue(run_arm.group_running(group))
                self.assertTrue(set(run_arm.descendants(group)) - {group},
                                "reparented native children remain in resource accounting")
                proc.send_signal(signal.SIGTERM)
                time.sleep(.2)
                self.assertIsNone(proc.poll())
                with (base / "lock").open("a") as lock:
                    with self.assertRaises(BlockingIOError):
                        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                stdout, stderr = proc.communicate(timeout=5)
                self.assertEqual(proc.returncode, 0, (stdout, stderr))
                self.assertFalse(run_arm.group_running(group))
                result = json.loads((base / "result.json").read_text())
                self.assertEqual(result["stop_reason"], "operator_signal_15")
                self.assertEqual(result["exit_code"], 0)  # still censored: launcher exited first
                self.assertTrue(result["killed"])
                self.assertGreater(result["owned_group_drain_seconds"], .5)
                with (base / "lock").open("a") as lock:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            finally:
                if group is not None:
                    try:
                        os.killpg(group, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                if proc.poll() is None:
                    proc.kill()
                    proc.wait()


if __name__ == "__main__":
    unittest.main()
