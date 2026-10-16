"""Transient finite-campaign startup status; never persisted as telemetry.

Stop and join the reporter before launching native work: the process guard uses
preexec_fn, which must not inherit this thread or its locks across fork.
"""
from __future__ import annotations

import os
import sys
import threading
import time


class StartupProgress:
    def __init__(self, enabled=True, *, stream=None, tty_interval=1.0, log_interval=30.0):
        self.enabled = enabled
        self.stream = sys.stderr if stream is None else stream
        self.tty = self.stream.isatty()
        self.color = self.tty and "NO_COLOR" not in os.environ
        self.interval = tty_interval if self.tty else log_interval
        self._lock = threading.Lock()
        self._stop = threading.Event()
        self._thread = None
        self._phase = None
        self._started = None
        self._width = 0

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.stop()

    def _write(self, text):
        try:
            self.stream.write(text)
            self.stream.flush()
        except (OSError, ValueError):
            # An unavailable progress stream cannot invalidate campaign work.
            self.enabled = False
            self._stop.set()

    def _render(self):
        elapsed = max(0, int(time.monotonic() - self._started))
        suffix = f" ({elapsed}s elapsed)"
        label = "Finite startup: " + self._phase
        if self.tty:
            try:
                columns = os.get_terminal_size(self.stream.fileno()).columns
            except (AttributeError, OSError, ValueError):
                columns = 100
            width = max(1, columns - 1)
            available = max(0, width - len(suffix))
            if len(label) > available:
                label = label[:max(0, available - 3)] + "..."[:available]
            line = (label + suffix)[-width:]
            line = line.ljust(min(self._width, width))
            self._write("\r" + ("\033[1;36m" + line + "\033[0m" if self.color else line))
            self._width = len(line)
        else:
            self._write(label + suffix + "\n")

    def _heartbeat(self):
        while not self._stop.wait(self.interval):
            with self._lock:
                if not self._stop.is_set():
                    self._render()

    def phase(self, label):
        if not self.enabled:
            return
        with self._lock:
            self._phase, self._started = label, time.monotonic()
            self._render()
            if self._thread is None and self.enabled:
                self._stop.clear()
                self._thread = threading.Thread(target=self._heartbeat,
                                                name="finite-startup-progress", daemon=True)
                self._thread.start()

    def stop(self):
        thread = self._thread
        if thread is None:
            return
        self._stop.set()
        thread.join()
        self._thread = None
        with self._lock:
            if self.tty and self.enabled:
                self._render()
                self._write("\n")
            self._width = 0
