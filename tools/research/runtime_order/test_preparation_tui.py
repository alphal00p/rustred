"""Real detached-terminal regressions; no solver, algebra or campaign writes.

RUSTRED_PREPARATION_VIEWER=/path/to/rustred python -m unittest test_preparation_tui
The viewer must render with stdin=/dev/null, stdout piped, and only a write-only
stderr TTY. No test responds to terminal cursor-position queries.
"""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]
BINARY = os.environ.get("RUSTRED_PREPARATION_VIEWER")


def fixture():
    jobs = []
    for index in range(14):
        jobs.append(dict(id=f"topology-{index+1:016x}", state="running" if index < 4 else "pending",
            metadata=dict(parent=index+1, root="111111111111111000000",
                selected_sectors=["1"*n + "0"*(21-n) for n in range(1,13)]),
            cpus=[], workers=8, observed_cores=1.0 if index < 4 else None,
            sampled_rss_bytes=800_000_000 if index < 4 else None))
    return dict(schema="rustred.preparation-progress.v1", run_id="visual-control", attempt_id="test",
        snapshot_seq=1, phase="generation", state="running", timestamp_unix_seconds=time.time(),
        elapsed_seconds=14, resources=dict(jobs=4, workers_per_job=8, total_workers=32, slots=[],
            memory=dict(effective_hard_memory_bytes=600_000_000_000,
                        effective_soft_memory_bytes=570_000_000_000, host_memory_reserve_bytes=0)),
        jobs=jobs, aggregate=dict(sampled_rss_bytes=3_200_000_000,
            sampled_peak_tree_rss_bytes=3_200_000_000, observed_cores=4.0,
            host_available_bytes=450_000_000_000, active_jobs=4, pending_jobs=10),
        stop_reason=None, family_closure_claim=False)


def completed_native_fixture():
    return dict(schema="rustred.family-generation-progress.v1", pid=9, process_start_ticks=17,
        invocation_id="pty-final", revision=1, state="output_written", elapsed_seconds=20,
        last_event_elapsed_seconds=20, counts=dict(sectors_total=18, generated=1, reused=17,
            checkpointed_new=1, rules_generated=1781, finite_residuals_generated=31,
            observed_rule_hits=1781, failures=0, overflow=False), active_jobs=[],
        details_truncated=False, last_failure=None,
        checkpoint=dict(resume_granularity="completed-sector", in_sector_resume=False),
        family_closure_claim=False)


@unittest.skipUnless(BINARY and os.name == "posix", "set RUSTRED_PREPARATION_VIEWER to run native PTY tests")
class PreparationTuiTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=ROOT / "TMP")
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "snapshot.json"
        self.data = fixture()
        self.publish()

    def publish(self):
        temporary = self.path.with_suffix(".part")
        temporary.write_text(json.dumps(self.data))
        os.replace(temporary, self.path)

    def launch(self, no_color=False, width=160, height=32):
        master, slave = pty.openpty()
        self.addCleanup(os.close, master)
        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
        # Match `2>/dev/tty`: the output fd itself is not readable either.
        output = os.open(os.ttyname(slave), os.O_WRONLY | os.O_NOCTTY)
        environment = dict(os.environ, TERM="xterm-256color", SYMBOLICA_HIDE_BANNER="1")
        environment.pop("NO_COLOR", None)
        if no_color:
            environment["NO_COLOR"] = "1"
        try:
            child = subprocess.Popen([BINARY, "preparation-monitor", "--snapshot", str(self.path)],
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=output,
                start_new_session=True, env=environment)
        finally:
            os.close(slave)
            os.close(output)
        def cleanup():
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)
            child.stdout.close()
        self.addCleanup(cleanup)
        return child, master

    def read_until(self, master, marker, timeout=5):
        output = b""
        deadline = time.monotonic()+timeout
        while marker not in output and time.monotonic() < deadline:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                output += chunk
        self.assertIn(marker, output, output.decode(errors="replace"))
        return output

    def test_detached_write_only_terminal_renders_color_without_cursor_query(self):
        child, master = self.launch()
        text = self.read_until(master, b"Read-only;")
        # Ratatui skips blank cells using cursor moves, so words separated by
        # spaces need not be contiguous in the ANSI transport.
        for marker in (b"\x1b[?1049h", b"PARENTS", b"Host", b"available", b"reserved",
                       b"P01", b"P14", b"Rules+", b"\x1b[38;"):
            self.assertIn(marker, text)
        self.assertNotIn(b"\x1b[6n", text)
        self.assertNotIn(b"unavailable", text)
        child.send_signal(signal.SIGTERM)
        self.read_until(master, b"\x1b[?1049l")
        self.assertEqual(child.wait(timeout=5), 0)

    def test_no_color_still_renders_table_and_resize_is_supported(self):
        child, master = self.launch(no_color=True)
        text = self.read_until(master, b"Read-only;")
        self.assertIn(b"PARENTS", text)
        self.assertNotIn(b"\x1b[38;", text)
        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 90, 0, 0))
        text = self.read_until(master, b"Read-only;")
        self.assertIn(b"CPU/budget", text)
        self.assertIn(b"RSS", text)
        self.assertNotIn(b"\x1b[6n", text)
        self.assertIsNone(child.poll())

    def test_completed_parent_recovers_final_counts_with_an_old_running_producer(self):
        self.data["run_id"] = str(self.path.parent.parent)
        self.data["attempt_id"] = self.path.parent.name
        self.data["resources"]["shared_pool"] = True
        job = self.data["jobs"][0]
        job.update(state="completed", preparation_state="completed", shared_process=True,
            pid=9, pid_start=17,
            native_progress=dict(status="stale", age_seconds=20000, snapshot=None))
        job["metadata"]["selected_sectors"] = [f"{sector:018b}" for sector in range(1, 19)]
        native = self.path.parent / "native-final.json"
        native.write_text(json.dumps(completed_native_fixture()))
        batch = self.path.parent / "batch.json"
        batch.write_text(json.dumps(dict(schema="rustred.family-candidates-batch.v1",
            jobs=[dict(id=job["id"],arguments=["--progress-json",str(native)])])))
        (self.path.parent / "request.json").write_text(json.dumps(dict(started_unix=1,
            jobs=[dict(id="shared-worker-pool",command=["rustred","family-candidates-batch",
                                                       "--config",str(batch)])])))
        self.publish()
        original = self.path.read_bytes()
        child, master = self.launch()
        text = self.read_until(master, b"Read-only;")
        self.assertIn(b"final counters", text)
        self.assertIn(b"18/18", text)
        self.assertNotIn(b"completed; native stale", text)
        self.assertEqual(self.path.read_bytes(), original)
        self.assertIsNone(child.poll())

    def test_completed_parent_never_displays_nonfinal_native_counts(self):
        for observation in ("fresh", "historical", "final"):
            for state in ("running", "failed", "stopped"):
                with self.subTest(observation=observation, state=state):
                    native = completed_native_fixture()
                    native["state"] = state
                    self.data["jobs"][0].update(state="completed", pid=9, pid_start=17,
                        native_progress=dict(status=observation, age_seconds=0, snapshot=native))
                    self.publish()
                    result = subprocess.run([BINARY, "preparation-monitor", "--snapshot",
                        str(self.path), "--once"], capture_output=True, text=True, check=True)
                    self.assertNotIn("final counters", result.stdout)
                    self.assertNotIn("sectors 18/18", result.stdout)
                    self.assertNotIn("new rules 1781", result.stdout)

    def test_terminal_snapshot_restores_screen_and_prints_final_summary(self):
        child, master = self.launch()
        self.read_until(master, b"Read-only;")
        self.data["state"] = "stopped"
        self.data["stop_reason"] = "operator_signal_2"
        self.publish()
        text = self.read_until(master, b"Read-only; campaign signals still stop work.")
        self.assertIn(b"\x1b[?1049l", text)
        primary = text.split(b"\x1b[?1049l", 1)[1]
        if "╯".encode() not in primary:
            text += self.read_until(master, "╯".encode())
            primary = text.split(b"\x1b[?1049l", 1)[1]
        for marker in ("╭".encode(), "╰".encode(), b"PARENTS", b"Parent", b"operator_signal_2"):
            self.assertIn(marker, primary)
        self.assertNotIn(b"RustRed preparation |", primary)
        self.assertNotIn(b"\x1b[6n", primary)
        self.assertEqual(child.wait(timeout=5), 0)

    def test_viewer_interrupt_does_not_change_snapshot(self):
        before = self.path.read_bytes()
        child, master = self.launch()
        self.read_until(master, b"Read-only;")
        child.send_signal(signal.SIGINT)
        self.read_until(master, b"\x1b[?1049l")
        self.assertEqual(child.wait(timeout=5), 0)
        self.assertEqual(self.path.read_bytes(), before)

    def test_shared_snapshot_labels_global_pool_and_parent_sector_activity(self):
        self.data['resources'].update(shared_pool=True,workers_per_job=0,max_active_sectors=2)
        self.data['aggregate']['pool']=dict(running=2,waiting_for_worker=0,admitted=2,completed=0,peak_running=2)
        for job in self.data['jobs']:
            job.update(shared_process=True,workers=None,observed_cores=None,sampled_rss_bytes=None)
        self.data['jobs'][0]['preparation_state']='preparing'
        self.publish()
        child,master=self.launch()
        output=self.read_until(master,b'Read-only;')
        for marker in [b'Active',b'shared',b'cap',b'preparation']:
            self.assertIn(marker,output)
        self.assertNotIn(b'CPU/budget',output)
        self.assertNotIn(b'unavailable',output)
        self.assertIsNone(child.poll())

    def test_piped_output_stays_plain_and_json_is_unchanged(self):
        self.data["state"] = "completed"
        self.publish()
        command = [BINARY, "preparation-monitor", "--snapshot", str(self.path)]
        result = subprocess.run(command, capture_output=True, check=True, timeout=5)
        self.assertNotIn(b"\x1b[", result.stderr)
        self.assertIn(b"Parents", result.stderr)
        result = subprocess.run(command+["--json"], capture_output=True, check=True, timeout=5)
        decoded = json.loads(result.stdout)
        for original, roundtrip in zip(self.data["jobs"], decoded["jobs"], strict=True):
            for key, value in original.items():
                self.assertEqual(roundtrip[key], value)


if __name__ == "__main__":
    unittest.main()
