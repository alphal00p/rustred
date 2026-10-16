"""Startup feedback remains transient and relinquishes threads before launch."""
from contextlib import redirect_stdout
import io
import os
import re
import threading
import time
import unittest
from unittest.mock import patch

import finite_startup_progress as STARTUP


class NotifyingStream(io.StringIO):
    def __init__(self, tty=False):
        super().__init__()
        self.tty = tty
        self.updated = threading.Event()

    def isatty(self):
        return self.tty

    def write(self, value):
        result = super().write(value)
        self.updated.set()
        return result


class StartupProgressTests(unittest.TestCase):
    @staticmethod
    def visible(stream):
        return re.sub(r"\x1b\[[0-9;]*m", "", stream.getvalue())

    def test_non_tty_heartbeat_shows_elapsed_and_stops_without_stdout(self):
        stream, stdout = NotifyingStream(), io.StringIO()
        now = [100.0]
        with redirect_stdout(stdout), patch.object(STARTUP.time, "monotonic", side_effect=lambda: now[0]):
            with STARTUP.StartupProgress(stream=stream, log_interval=.02) as progress:
                progress.phase("checking executable")
                self.assertEqual(stream.getvalue(), "Finite startup: checking executable (0s elapsed)\n")
                stream.updated.clear()
                now[0] = 103.0
                self.assertTrue(stream.updated.wait(1), "background heartbeat did not arrive")
                thread = progress._thread
            self.assertFalse(thread.is_alive())
            stopped = stream.getvalue()
            time.sleep(.05)
            self.assertEqual(stream.getvalue(), stopped)
        self.assertIn("checking executable (3s elapsed)", stopped)
        self.assertNotIn("\r", stopped)
        self.assertEqual(stdout.getvalue(), "")

    def test_tty_reuses_one_line_and_ends_it_on_stop(self):
        stream = NotifyingStream(tty=True)
        with STARTUP.StartupProgress(stream=stream, tty_interval=60) as progress:
            progress.phase("validating committed input sources")
            progress.phase("ready")
            self.assertNotIn("\n", stream.getvalue())
        self.assertEqual(stream.getvalue().count("\n"), 1)
        self.assertIn("\rFinite startup: ready (0s elapsed)", self.visible(stream))
        self.assertTrue(stream.getvalue().endswith("\n"))

    def test_exceptions_and_keyboard_interrupt_join_reporter(self):
        for error in (ValueError("failed"), KeyboardInterrupt()):
            with self.subTest(error=type(error).__name__):
                with self.assertRaises(type(error)):
                    with STARTUP.StartupProgress(stream=io.StringIO(), log_interval=60) as progress:
                        progress.phase("validating sources")
                        thread = progress._thread
                        raise error
                self.assertFalse(thread.is_alive())

    def test_tty_uses_stderr_width_and_does_not_wrap_after_resize(self):
        stream = NotifyingStream(tty=True)
        with redirect_stdout(io.StringIO()), patch.object(stream, "fileno", return_value=42), \
             patch.object(STARTUP.os, "get_terminal_size", return_value=os.terminal_size((80, 24))) as size:
            with STARTUP.StartupProgress(stream=stream, tty_interval=60) as progress:
                progress.phase("validating committed input sources")
                self.assertLessEqual(len(self.visible(stream).split("\r")[-1]), 79)
                size.return_value = os.terminal_size((40, 24))
                progress.phase("checking executable")
                self.assertLessEqual(len(self.visible(stream).split("\r")[-1]), 39)
            size.assert_called_with(42)

    def test_startup_color_respects_no_color(self):
        for disabled in (False, True):
            stream = NotifyingStream(tty=True)
            with patch.dict(STARTUP.os.environ, {"NO_COLOR": "1"} if disabled else {}, clear=True):
                with STARTUP.StartupProgress(stream=stream, tty_interval=60) as progress:
                    progress.phase("preparing")
            self.assertEqual("\033[1;36m" in stream.getvalue(), not disabled)

    def test_reporter_restarts_only_when_next_phase_begins(self):
        with STARTUP.StartupProgress(stream=io.StringIO(), log_interval=60) as progress:
            progress.phase("before solve")
            before = progress._thread
            progress.stop()
            self.assertFalse(before.is_alive())
            self.assertIsNone(progress._thread)
            progress.phase("after solve")
            after = progress._thread
            self.assertIsNot(before, after)
            self.assertTrue(after.is_alive())
        self.assertFalse(after.is_alive())

    def test_disabled_reporter_and_closed_stream_never_start_threads(self):
        stream = io.StringIO()
        with STARTUP.StartupProgress(enabled=False, stream=stream) as progress:
            progress.phase("dry run")
            self.assertIsNone(progress._thread)
        self.assertEqual(stream.getvalue(), "")
        with STARTUP.StartupProgress(stream=stream) as progress:
            stream.close()
            progress.phase("unavailable stderr")
            self.assertIsNone(progress._thread)


if __name__ == "__main__":
    unittest.main()
