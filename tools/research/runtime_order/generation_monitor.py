"""Optional read-only native dashboard; presentation never controls generation."""
import json
import os
from pathlib import Path
import signal
import subprocess
import sys

from generation_guard import native_environment, SUPERVISOR


class Dashboard:
    def __init__(self, executable, directory, resources, enabled=True):
        self.command = [str(executable), "preparation-monitor", "--snapshot", str(Path(directory) / "snapshot.json")]
        self.directory = Path(directory)
        self.resources, self.enabled = resources, enabled
        self.process = None
        self.attempted = False
        self.receipt = dict(enabled=enabled, command=self.command, read_only=True,
                            solver_control=False, state="not_started")

    def _record(self):
        try:
            (self.directory / "dashboard.json").write_text(json.dumps(self.receipt, indent=2) + "\n")
        except OSError:
            pass  # Presentation receipts cannot stop generation either.

    def _failed(self, error):
        self.receipt.update(state="unavailable", diagnostic=f"{type(error).__name__}: {error}"[:1024])
        self._record()
        try:
            print(f"Preparation dashboard unavailable ({error}); generation continues. "
                  f"Read {self.directory / 'snapshot.json'} or events.jsonl.", file=sys.stderr, flush=True)
        except OSError:
            pass

    def __enter__(self):
        return self

    def observe(self, event):
        if not self.enabled:
            return
        try:
            if not self.attempted:
                # The producer writes its snapshot before invoking observers.
                # A missing initial snapshot is an observer error, not authority
                # to alter the generation queue or retry native work.
                if not (self.directory / "snapshot.json").is_file():
                    return
                self.attempted = True
                cpus = SUPERVISOR.parse_cpu_set(self.resources["cpus"])
                self.process = subprocess.Popen(self.command, stdin=subprocess.DEVNULL,
                    env=native_environment(), start_new_session=True,
                    preexec_fn=lambda: os.sched_setaffinity(0, cpus))
                self.receipt.update(state="running", pid=self.process.pid, process_group=self.process.pid)
                self._record()
            if self.process is not None:
                code = self.process.poll()
                if code is not None:
                    self.receipt.update(state="exited", exit_code=code)
                    self._record()
                    # No auto-restart: users may intentionally close this
                    # read-only viewer while generation continues.
        except Exception as error:
            self._failed(error)

    def __exit__(self, exception_type, exception, traceback):
        if self.process is None:
            return False
        try:
            # Normally the terminal snapshot makes the viewer finish itself.
            # These bounds affect only viewer cleanup, never a solver deadline.
            try:
                code = self.process.wait(timeout=1)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(self.process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    code = self.process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(self.process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    code = self.process.wait()
            self.receipt.update(state="exited", exit_code=code)
            self._record()
        except BaseException as error:
            # A second terminal interrupt during viewer shutdown must not
            # leave the read-only child behind. Only this viewer's PG is ours
            # here; native generation has its separate supervisor lifecycle.
            try:
                if self.process.poll() is None:
                    try:
                        os.killpg(self.process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    self.process.wait()
            except Exception as cleanup:
                self.receipt["cleanup_diagnostic"] = str(cleanup)[:1024]
            self._failed(error)
            if not isinstance(error, Exception):
                raise
        return False
