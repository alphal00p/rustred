"""Optional actual-CLI PTY regression; uses an existing binary, never builds Rust.

RUSTRED_NATIVE_TTY_TEST_BINARY=/absolute/path/to/rustred python -B -m unittest \
    discover -s tools/research/runtime_order -p test_generation_guard_native_tty.py -v

The tiny generated one-loop fixture and every native output stay in workspace TMP.
A cursor-position reply exercises Ratatui initialization, beyond isatty detection.
"""
import errno
import codecs
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import pty
import re
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unittest


ROOT = Path(__file__).resolve().parents[3]
GUARD_PATH = Path(__file__).with_name("generation_guard.py")
BINARY = os.environ.get("RUSTRED_NATIVE_TTY_TEST_BINARY")
RUNNER = """import importlib.util,json,subprocess,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location('native_tty_guard',sys.argv[1])
guard=importlib.util.module_from_spec(spec);spec.loader.exec_module(guard)
request=json.loads(Path(sys.argv[2]).read_text())
if request['broken_stderr_only']:
 original=subprocess.Popen
 def broken(command,**options):
  options['stdin']=subprocess.DEVNULL
  with (Path(request['directory'])/'stdout').open('ab') as captured:
   options['stdout']=captured
   return original(command,**options)
 subprocess.Popen=broken
try:
 guard.run(request['command'],request['directory'],request['resources'],
  poll_seconds=.02,grace_seconds=1,
  cooperative_stop_file=request['stop_file'],native_tty_progress=True)
except RuntimeError:
 pass
"""


def write_json(path, value):
    path.write_text(json.dumps(value) + "\n")


def policy():
    # Small explicit grants suffice for this one-row fixture. No production
    # source, namespace, or operational policy is opened or amended.
    mib = 1024 * 1024
    return {
        "workers": 1,
        "windows": {"max_seed_points": 1, "max_seed_bytes": mib,
                    "max_positive_layers": 64, "max_partition_checks": 1000},
        "census": {"max_unique_selections": 100, "max_identity_payload_bytes": mib},
        "export": {"max_records": 1000, "max_live_payload_bytes": 16 * mib},
        "storage": {
            "raw": {"max_page_records": 256, "max_page_bytes": mib,
                    "max_runs": 128, "max_pages": 128, "max_directory_bytes": mib,
                    "max_stored_references": 10000, "max_merge_runs": 128,
                    "max_merge_cached_pages": 128, "max_merge_payload_bytes": 16 * mib,
                    "max_merge_read_bytes": 64 * mib, "max_decode_transient_bytes": 128 * mib},
            "max_arity": 1, "max_delta_records": 1000, "max_delta_payload_bytes": 16 * mib,
            "max_page_records": 256, "max_page_bytes": mib, "max_cached_pages": 128,
            "max_cached_page_bytes": 16 * mib, "max_directory_bytes": mib,
            "max_manifest_bytes": mib, "max_state_bytes": mib, "max_scope_rows": 16,
            "max_census_records": 100, "max_runs": 128, "max_pages": 128,
            "max_physical_files": 1000, "max_disk_bytes": 64 * mib,
            "max_lookup_pages": 128, "max_lookup_bytes": 16 * mib,
            "max_transient_bytes": 128 * mib},
        "max_total_rows": 16, "max_source_bytes": mib,
        "control": {"max_bytes": 16 * mib, "max_files": 1000}}


@unittest.skipUnless(os.name == "posix" and BINARY,
                     "set RUSTRED_NATIVE_TTY_TEST_BINARY to an existing native CLI")
class NativeInlineTtyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = Path(BINARY).resolve(strict=True)
        cls.temporary = tempfile.TemporaryDirectory(dir=ROOT / "TMP", prefix="native-tty-regression-")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.root = Path(cls.temporary.name)
        cls.fixture = cls.root / "fixture"
        cls.fixture.mkdir()
        spec = importlib.util.spec_from_file_location("tty_test_guard", GUARD_PATH)
        cls.guard = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.guard)
        cls.environment = cls.guard.native_environment()
        cls.environment["TERM"] = "xterm-256color"
        cls.environment.pop("NO_COLOR", None)
        cls.cpu = min(os.sched_getaffinity(0))
        source = cls.fixture / "source.toml"
        source.write_text('''schema="rustred.project.toml.v1"
[family]
name="native_inline_tty_test"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
''')
        cls.native("family-candidates", "--input", source, "--output", cls.fixture / "owner.rrbin",
                   "--max-numerator-rank", "0", "--report-output", cls.fixture / "generation.toml")
        cls.native("candidate-inspect", "--input", cls.fixture / "owner.rrbin",
                   "--output", cls.fixture / "inspection.json")
        inspection = json.loads((cls.fixture / "inspection.json").read_text())
        write_json(cls.fixture / "selection.json", {
            "family_fingerprint": inspection["family_fingerprint"],
            "owners": [{"path": "owner.rrbin", "bytes": (cls.fixture / "owner.rrbin").stat().st_size,
                        "mask": "1"}], "initial_frontier_routes": []})
        write_json(cls.fixture / "queries.json", {
            "schema": "rustred.owner-domain-queries.json.v2",
            "queries": [{"id": "q", "owner": "1", "lower": [0], "upper": [2],
                         "max_numerator_rank": 0, "power_bounds": {"max_positive_power": 3,
                         "min_power_difference": 1, "max_power_difference": 3}}],
            "query_roles": {"required": ["q"], "auxiliary": []}})
        write_json(cls.fixture / "source.json", ["rustred", "owner-domain-match",
            "--manifest", "selection.json", "--queries", "queries.json", "--owner-base", ".",
            "--follow-successors", "--publication-policy", "epoch", "--transfer-unreserved-lookahead", "1",
            "--checkpoint", "unused-checkpoint", "--output", "unused-output.json"])
        write_json(cls.fixture / "policy.json", policy())

    @classmethod
    def native(cls, *arguments):
        completed = subprocess.run([str(cls.binary), *map(str, arguments)],
            stdin=subprocess.DEVNULL, capture_output=True, env=cls.environment, timeout=20,
            preexec_fn=lambda: os.sched_setaffinity(0, {cls.cpu}))
        if completed.returncode:
            raise AssertionError(completed.stderr.decode(errors="replace"))

    def run_guard(self, name, *, broken=False, cancelled=False, write_only_stderr=False):
        directory = self.root / name
        directory.mkdir()
        inputs = directory / "inputs"
        shutil.copytree(self.fixture, inputs)
        stop = directory / "stop.json"
        if cancelled:
            write_json(stop, {"reason": "test_preexisting_stop"})
        command = [str(self.binary), "finite-demand", "run", "--directory", str(directory / "finite"),
                   "--source-command", str(inputs / "source.json"),
                   "--policy", str(inputs / "policy.json"), "--output", str(directory / "native.json"),
                   "--events", str(directory / "events.jsonl"), "--stop-file", str(stop)]
        write_json(directory / "request.json", {
            "command": command, "directory": str(directory / "guard"), "stop_file": str(stop),
            "broken_stderr_only": broken,
            "resources": {"cpus": str(self.cpu), "workers": 1, "max_memory_bytes": 600_000_000_000,
                          "memory_policy": "process-limit",
                          "host_memory_reserve_bytes": 0, "ram_guard_margin_percent": 5,
                          "swap_growth_stop_bytes_per_second": 0}})
        master, slave = pty.openpty()
        outer = None
        display = bytearray()
        replies = 0
        terminal_stream = None
        decoder = codecs.getincrementaldecoder("utf-8")(errors="replace")
        if os.environ.get("RUSTRED_NATIVE_TTY_EMULATOR") == "1":
            # Optional visual-audit mode: a real emulator reports the current
            # cursor rather than the fixed origin used by the dependency-free
            # handshake smoke test. This matters for inline viewport resizing.
            import pyte
            class ReplyingScreen(pyte.Screen):
                def write_process_input(self, text):
                    os.write(master, text.encode())
            terminal_stream = pyte.Stream(ReplyingScreen(160, 32))
        try:
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 160, 0, 0))
            original_termios = termios.tcgetattr(slave)
            stderr = os.open(os.ttyname(slave), os.O_WRONLY | os.O_NOCTTY) if write_only_stderr else slave
            try:
                outer = subprocess.Popen([sys.executable, "-B", "-c", RUNNER,
                    str(GUARD_PATH), str(directory / "request.json")], stdin=subprocess.DEVNULL,
                    stdout=subprocess.PIPE, stderr=stderr, start_new_session=True, env=self.environment)
            finally:
                if write_only_stderr:
                    os.close(stderr)
            deadline = time.monotonic() + 20
            # A PTY is not a terminal emulator: actively answer the actual
            # ESC[6n request that Ratatui/crossterm writes through stdout.
            while True:
                ready, _, _ = select.select([master], [], [], .05)
                if ready:
                    try:
                        block = os.read(master, 65536)
                    except OSError as error:
                        if error.errno != errno.EIO:
                            raise
                        block = b""
                    display.extend(block)
                    count = display.count(b"\x1b[6n")
                    if terminal_stream is not None:
                        terminal_stream.feed(decoder.decode(block))
                        replies = count
                    else:
                        while replies < count:
                            os.write(master, b"\x1b[1;1R")
                            replies += 1
                elif outer.poll() is not None:
                    break
                if time.monotonic() >= deadline:
                    self.fail("native TTY guard exceeded 20s: " + bytes(display[-2000:]).decode(errors="replace"))
            stdout, _ = outer.communicate(timeout=2)
            self.assertEqual(outer.returncode, 0, bytes(display).decode(errors="replace"))
            self.assertEqual(stdout, b"")
            self.assertEqual(termios.tcgetattr(slave), original_termios)
        finally:
            # Cleanup only the guard and native groups created by this test.
            child = directory / "guard/child.json"
            if child.is_file():
                group = json.loads(child.read_text())["process_group"]
                if self.guard.group_running(group):
                    os.killpg(group, signal.SIGKILL)
            if outer is not None and outer.poll() is None:
                outer.kill()
                outer.wait(timeout=3)
            os.close(master)
            os.close(slave)
        receipt = json.loads((directory / "guard/result.json").read_text())
        self.assertTrue(receipt["owned_group_drained"])
        self.assertFalse(self.guard.group_running(json.loads((directory / "guard/child.json").read_text())["process_group"]))
        self.assertTrue((directory / "guard/resources.jsonl").read_text())
        native = json.loads((directory / "native.json").read_text())
        self.assertEqual(native["schema"], "rustred.finite-demand-run.v1")
        self.assertEqual(native["complete"], not cancelled, native)
        self.assertTrue((directory / "events.jsonl").read_text())
        self.assertNotIn(b"Generation/admission", display)
        self.assertEqual((directory / "guard/stderr").read_bytes(), b"")
        # Ratatui positions each changed word/cell instead of emitting a plain
        # text line. Preserve separators while removing control sequences.
        text = b" ".join(re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b" ", display).split())
        if evidence := os.environ.get("RUSTRED_NATIVE_TTY_EVIDENCE"):
            evidence = Path(evidence).resolve()
            if not evidence.is_relative_to(ROOT / "TMP"):
                raise ValueError("native TTY test evidence must remain in workspace TMP")
            evidence.mkdir(parents=True, exist_ok=True)
            (evidence / (name + ".ansi")).write_bytes(display)
        self.last_display = bytes(display)
        return text, replies, receipt

    def test_real_inline_dashboard_requires_all_native_terminal_descriptors(self):
        before, replies, before_receipt = self.run_guard("stderr-only", broken=True)
        self.assertEqual(before_receipt["exit_code"], 0)
        self.assertEqual(replies, 0)
        self.assertNotIn(b"FINITE DEMAND", before)
        after, replies, after_receipt = self.run_guard("terminal-descriptors")
        self.assertEqual(after_receipt["exit_code"], 0)
        self.assertGreater(replies, 0)
        self.assertIn(b"FINITE DEMAND", after)
        self.assertIn("╭".encode(), after)
        self.assertIn(b"required rows closed", after)
        self.assertIn(b"Host available", after)
        self.assertIn(b"Process RAM cap", after)
        self.assertIn(b"600.00 GB", after)
        self.assertIn(b"Input occurrences", after)
        self.assertIn(b"Chase elapsed", after)
        self.assertIn(b"Convergence", after)
        # Inline viewport is21 rows in this32-row terminal. Cleanup must land
        # at the viewport bottom, not scroll the whole screen's last row.
        self.assertNotIn(b"\x1b[32;1H", self.last_display)
        self.assertIn(b"\x1b[21;1H", self.last_display)

    def test_native_cancellation_keeps_display_and_drains_group(self):
        display, replies, receipt = self.run_guard("cancelled", cancelled=True)
        self.assertGreater(replies, 0)
        self.assertIn(b"FINITE DEMAND", display)
        self.assertNotEqual(receipt["exit_code"], 0)

    def test_write_only_stderr_is_reopened_for_native_terminal_input(self):
        display, replies, receipt = self.run_guard("write-only-stderr", write_only_stderr=True)
        self.assertGreater(replies, 0)
        self.assertIn(b"FINITE DEMAND", display)
        self.assertEqual(receipt["exit_code"], 0)


if __name__ == "__main__":
    unittest.main()
