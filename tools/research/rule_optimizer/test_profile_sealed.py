"""Small synthetic files only; no native or production checkpoint reads."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import profile_records as P
import profile_sealed as S


class SealedProfileTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.cp = self.root / "checkpoints/main"
        self.cp.mkdir(parents=True)
        (self.root / "inputs").mkdir()
        (self.root / "run").mkdir()
        self.write(self.root / "inputs/input-receipt.json", {"queries_sha256": "fixture"})
        self.write(self.root / "active-run.json", {"run_directory": str(self.root / "run"),
                                                   "executable_sha256": "fixture"})
        self.write(self.root / "run/status.json", {"progress": {"native_status": "running"}})
        # Live inventory must never consume result.json.
        (self.root / "run/result.json").write_text("not JSON and must not be read")
        segments = []
        for gen in (1, 2, 3):
            path = self.cp / f"records-{gen:020}.bin"
            with path.open("xb") as output:
                output.truncate(16 * S.MIB)
            segments.append(dict(generation=gen, file=path.name, first=(gen - 1) * 10,
                                 count=10, bytes=path.stat().st_size, blake3=[0] * 32))
        self.meta = self.cp / "epoch-00000000000000000003-meta.part"
        self.registry = self.cp / "epoch-00000000000000000003-record-segments.part"
        self.write(self.meta, {"record_schema": 1})
        self.write(self.registry, segments)
        manifest = dict(format="RUSTRED-WALK-CP6", schema=3, generation=3, arity=15, files=[
            dict(key="meta", file=self.meta.name, bytes=self.meta.stat().st_size, count=1),
            dict(key="record-segments", file=self.registry.name,
                 bytes=self.registry.stat().st_size, count=3)])
        self.write(self.cp / "latest.json", {"manifest": manifest})

    @staticmethod
    def write(path, data):
        path.write_text(json.dumps(data))

    def plan(self, generations="1,2"):
        return P.make_plan([str(self.root) + ":" + generations], [], True)

    def test_live_requires_explicit_opt_in(self):
        (self.root / "run/result.json").unlink()
        with self.assertRaisesRegex(ValueError, "stopped"):
            P.make_plan([str(self.root) + ":1,2"], [])
        self.write(self.root / "run/status.json", {"progress": {"native_status": "stopped"}})
        old = P.make_plan([str(self.root) + ":1,2"], [])
        self.assertNotIn("live_sealed_only", old)
        P.validate_plan(old)

    def test_codec_dynamic_import_from_outside_tool_directory(self):
        path = str(Path(P.__file__).resolve())
        code = ("import importlib.util; "
                f"s=importlib.util.spec_from_file_location('standalone_codec', {path!r}); "
                "m=importlib.util.module_from_spec(s); s.loader.exec_module(m); "
                "assert m.MAGIC == b'ERB1'")
        completed = subprocess.run([sys.executable, "-I", "-B", "-c", code], cwd=self.root,
                                   text=True, capture_output=True)
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_two_old_segments_exact_8mib_and_snapshot(self):
        plan = self.plan()
        P.validate_plan(plan)
        self.assertEqual(plan["total_requested_bytes"], 8 * S.MIB)
        self.assertFalse(plan["full_checkpoint_authenticated"])
        self.assertNotIn("result", plan["sources"][0])
        self.assertEqual(len(S.read_windows(plan)), 8)

    def test_latest_may_advance_without_reselection(self):
        plan = self.plan()
        self.write(self.cp / "latest.json", {"a_new_generation": "not consumed"})
        self.assertEqual(len(S.read_windows(plan)), 8)
        self.assertEqual(plan["sources"][0]["latest_manifest"]["manifest"]["generation"], 3)

    def test_latest_sealed_active_and_duplicate_generations_rejected(self):
        for generations in ("2,3", "1,1", "1,2,3", "1"):
            with self.subTest(generations=generations), self.assertRaises(ValueError):
                self.plan(generations)
        # An unlisted tail cannot be selected, regardless of its filename.
        (self.cp / "records-00000000000000000004.bin").write_bytes(b"tail")
        with self.assertRaises(StopIteration):
            self.plan("1,4")

    def test_registry_generation_count_and_snapshot_tampering_rejected(self):
        plan = self.plan()
        for mutate in (lambda s: s["record_segments"][0].update(first=1),
                       lambda s: s["latest_manifest"]["manifest"].update(generation=4),
                       lambda s: s["sealed_snapshots"]["registry"].update(text="[]")):
            bad = copy.deepcopy(plan)
            mutate(bad["sources"][0])
            with self.assertRaises(ValueError):
                P.validate_plan(bad)

    def test_pinned_registry_or_run_changes_rejected(self):
        plan = self.plan()
        self.write(self.registry, [])
        with self.assertRaisesRegex(ValueError, "pinned committed metadata"):
            S.read_windows(plan)

    def test_symlink_and_nonregular_rejected(self):
        path = self.cp / "records-00000000000000000001.bin"
        path.unlink()
        path.symlink_to(self.cp / "records-00000000000000000002.bin")
        with self.assertRaisesRegex(ValueError, "regular non-symlink"):
            self.plan()
        path.unlink()
        path.mkdir()
        with self.assertRaisesRegex(ValueError, "regular non-symlink"):
            self.plan()

    def test_same_size_mtime_replacement_rejected_by_inode(self):
        plan = self.plan()
        window = plan["windows"][0]
        path = Path(window["path"])
        replacement = path.with_suffix(".new")
        with replacement.open("xb") as output:
            output.truncate(window["stat_size"])
        os.utime(replacement, ns=(window["stat_mtime_ns"], window["stat_mtime_ns"]))
        replacement.replace(path)
        with self.assertRaisesRegex(ValueError, "identity changed"):
            S.read_windows(plan)

    def test_same_size_write_with_restored_mtime_rejected_by_ctime(self):
        plan = self.plan()
        window = plan["windows"][0]
        path = Path(window["path"])
        with path.open("r+b") as output:
            output.write(b"changed")
        os.utime(path, ns=(window["stat_mtime_ns"], window["stat_mtime_ns"]))
        with self.assertRaisesRegex(ValueError, "identity changed"):
            S.read_windows(plan)

    def test_mid_read_mutation_rejected(self):
        plan = self.plan()
        path = Path(plan["windows"][0]["path"])
        original = os.pread
        def changing(fd, count, offset):
            block = original(fd, count, offset)
            with path.open("ab") as output:
                output.write(b"mutation")
            return block
        with patch.object(S.os, "pread", side_effect=changing):
            with self.assertRaises(ValueError):
                S.read_ranges(path, [(0, 32)], plan["windows"][0]["stat_identity"])

    def test_plan_cannot_move_or_grow_windows(self):
        plan = self.plan()
        for mutate in (lambda p: p["windows"][0].update(start=0),
                       lambda p: p.update(total_requested_bytes=9 * S.MIB),
                       lambda p: p["windows"][0].update(path=str(self.root / "outside"))):
            bad = copy.deepcopy(plan)
            mutate(bad)
            with self.assertRaises(ValueError):
                P.validate_plan(bad)

    def test_metadata_read_is_bounded(self):
        with self.meta.open("wb") as output:
            output.truncate(S.MIB + 1)
        with self.assertRaisesRegex(ValueError, "1MiB"):
            self.plan()

    def test_segment_size_must_match_committed_registry(self):
        path = self.cp / "records-00000000000000000001.bin"
        with path.open("ab") as output:
            output.write(b"tail")
        with self.assertRaisesRegex(ValueError, "committed segment size"):
            self.plan()


if __name__ == "__main__":
    unittest.main()
