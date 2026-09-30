"""The viewer observes snapshots; it never owns solver scheduling or signals."""
from contextlib import redirect_stderr
import io
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest
from unittest.mock import MagicMock, patch

import generation_monitor as monitor
from generation_guard import ROOT


class DashboardTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=ROOT / "TMP")
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name)
        self.resources = dict(cpus=str(min(os.sched_getaffinity(0))))

    def test_viewer_starts_only_after_snapshot_in_separate_session(self):
        child = MagicMock(pid=12345)
        child.poll.return_value = None
        child.wait.return_value = 0
        with patch.object(monitor.subprocess, "Popen", return_value=child) as spawn:
            with monitor.Dashboard("CLI", self.path, self.resources) as dashboard:
                dashboard.observe({"event":"before-start"})
                spawn.assert_not_called()
                (self.path / "snapshot.json").write_text("{}")
                dashboard.observe({"event":"started"})
                dashboard.observe({"event":"sample"})
                self.assertEqual(spawn.call_count, 1)
                self.assertTrue(spawn.call_args.kwargs["start_new_session"])
                self.assertEqual(spawn.call_args.args[0],
                    ["CLI","preparation-monitor","--snapshot",str(self.path / "snapshot.json")])
                self.assertEqual(spawn.call_args.kwargs["stdin"], subprocess.DEVNULL)
        self.assertTrue((self.path / "dashboard.json").is_file())

    def test_disabled_viewer_never_spawns(self):
        (self.path / "snapshot.json").write_text("{}")
        with patch.object(monitor.subprocess, "Popen") as spawn:
            with monitor.Dashboard("CLI", self.path, self.resources, False) as dashboard:
                dashboard.observe({})
            spawn.assert_not_called()

    def test_viewer_failure_does_not_raise_or_restart(self):
        (self.path / "snapshot.json").write_text("{}")
        with patch.object(monitor.subprocess, "Popen", side_effect=OSError("viewer absent")) as spawn, \
                redirect_stderr(io.StringIO()):
            with monitor.Dashboard("CLI", self.path, self.resources) as dashboard:
                dashboard.observe({})
                dashboard.observe({})
            self.assertEqual(spawn.call_count, 1)
            self.assertEqual(dashboard.receipt["state"], "unavailable")

    def test_cleanup_can_only_signal_its_viewer_group(self):
        (self.path / "snapshot.json").write_text("{}")
        child = MagicMock(pid=12345)
        child.poll.return_value = None
        child.wait.side_effect = [subprocess.TimeoutExpired("viewer",1),
                                 subprocess.TimeoutExpired("viewer",2), -signal.SIGKILL]
        with patch.object(monitor.subprocess, "Popen", return_value=child), \
                patch.object(monitor.os, "killpg") as kill:
            with monitor.Dashboard("CLI", self.path, self.resources) as dashboard:
                dashboard.observe({})
            self.assertEqual(kill.call_args_list[0].args, (12345, signal.SIGTERM))
            self.assertEqual(kill.call_args_list[1].args, (12345, signal.SIGKILL))
            self.assertEqual(kill.call_count, 2)

    def test_natural_viewer_exit_does_not_restart(self):
        (self.path / "snapshot.json").write_text("{}")
        child = MagicMock(pid=12345)
        child.poll.return_value = 1
        child.wait.return_value = 1
        with patch.object(monitor.subprocess, "Popen", return_value=child) as spawn, \
                patch.object(monitor.os, "killpg") as kill:
            with monitor.Dashboard("CLI", self.path, self.resources) as dashboard:
                dashboard.observe({})
                dashboard.observe({})
            self.assertEqual(spawn.call_count, 1)
            kill.assert_not_called()
            self.assertEqual(dashboard.receipt["exit_code"], 1)

    def test_interrupt_during_viewer_cleanup_drains_only_viewer_then_propagates(self):
        (self.path / "snapshot.json").write_text("{}")
        child = MagicMock(pid=12345)
        child.poll.return_value = None
        child.wait.side_effect = [KeyboardInterrupt(), -signal.SIGKILL]
        with patch.object(monitor.subprocess, "Popen", return_value=child), \
                patch.object(monitor.os, "killpg") as kill, redirect_stderr(io.StringIO()), \
                self.assertRaises(KeyboardInterrupt):
            with monitor.Dashboard("CLI", self.path, self.resources) as dashboard:
                dashboard.observe({})
        kill.assert_called_once_with(12345, signal.SIGKILL)


if __name__ == "__main__":
    unittest.main()
