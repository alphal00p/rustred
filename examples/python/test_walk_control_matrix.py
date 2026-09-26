"""Control-matrix harness tests with a fake executable; no native walk or license."""
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


MATRIX = module("walk_control_matrix")
FIXTURE = module("test_audit_owner_domain_walk")
FIXTURE_PATH = Path(__file__).with_name("test_audit_owner_domain_walk.py")

FAKE_EXECUTABLE = f"""#!{sys.executable}
import importlib.util, json, os, sys
from pathlib import Path
assert sys.argv[1] == "owner-domain-match" and os.environ["RAYON_NUM_THREADS"] == "1"
spec = importlib.util.spec_from_file_location("fixture", {str(FIXTURE_PATH)!r})
fixture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture)
arg = lambda name: sys.argv[sys.argv.index(name) + 1]
queries, top = fixture.walk_document(arg("--publication-policy"), Path(arg("--checkpoint")), int(arg("--workers")))
assert json.loads(Path(arg("--queries")).read_text()) == queries
fixture.write_walk(Path(arg("--output")), Path(arg("--events")), Path(arg("--checkpoint")), queries, top)
sys.stdout.write("fake walk complete\\n")
raise SystemExit(int(os.environ.get("FAKE_EXIT", "0")))
"""


def fixture_inputs(root):
    executable = root / "fake-rustred"
    executable.write_text(FAKE_EXECUTABLE)
    executable.chmod(0o700)
    owner = root / "owner.rrbin"
    owner.write_bytes(b"fixture only")
    manifest = root / "selection.json"
    manifest.write_text(json.dumps({"owners": [{"mask": "10", "path": "owner.rrbin", "bytes": 12},
                                               {"mask": "01", "path": "owner.rrbin", "bytes": 12}]}))
    queries = root / "queries.json"
    queries.write_text(json.dumps(FIXTURE.queries_document(), indent=1) + "\n")
    return executable, manifest, queries


def matrix_document(root, cpus, worker_count, **overrides):
    case = {"name": "fg-ordered", "executable": "fake-rustred", "manifest": "selection.json",
            "queries": "queries.json", "owner_base": ".", "workers": worker_count, "cpus": cpus,
            "publication_policy": "ordered", "inspection_workers": None, "native_options": [],
            "max_memory_bytes": 50_000_000_000, "checkpoint_interval_seconds": 3600}
    case.update(overrides)
    return {"schema": MATRIX.MATRIX_SCHEMA, "cases": [case]}


class MatrixValidationTests(unittest.TestCase):
    def test_matrix_validation_rejects_bad_cases_before_running(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture_inputs(root)
            affinity = sorted(os.sched_getaffinity(0))
            cpu = affinity[-1]
            good = matrix_document(root, str(cpu), 1)
            path = root / "matrix.json"
            path.write_text(json.dumps(good))
            loaded = MATRIX.load_matrix(path)
            self.assertEqual(loaded["cases"][0]["cpu_set"], {cpu})
            self.assertEqual(loaded["cases"][0]["transfer_unreserved_lookahead"], 256)
            bad = [("schema", {"schema": "other", "cases": good["cases"]}, "schema"),
                   ("no cases", {"schema": MATRIX.MATRIX_SCHEMA, "cases": []}, "at least one"),
                   ("name", matrix_document(root, str(cpu), 1, name="bad name"), "name"),
                   ("workers", matrix_document(root, str(cpu), 1, workers=2), "exactly workers"),
                   ("cap", matrix_document(root, "0-256", 1, workers=257), "workers must be"),
                   ("policy", matrix_document(root, str(cpu), 1, publication_policy="owner-batched"), "publication_policy"),
                   ("inspectors", matrix_document(root, str(cpu), 1, inspection_workers=2), "inspection_workers"),
                   ("options", matrix_document(root, str(cpu), 1, native_options="--x"), "native_options"),
                   ("memory", matrix_document(root, str(cpu), 1, max_memory_bytes=0), "positive"),
                   ("executable", matrix_document(root, str(cpu), 1, executable="missing"), "not a file"),
                   ("affinity", matrix_document(root, str(max(affinity) + 1), 1), "outside this process's affinity")]
            for name, document, fragment in bad:
                with self.subTest(name=name):
                    path.write_text(json.dumps(document))
                    with self.assertRaisesRegex(ValueError, fragment):
                        MATRIX.load_matrix(path)
            duplicate = {"schema": MATRIX.MATRIX_SCHEMA, "cases": good["cases"] * 2}
            path.write_text(json.dumps(duplicate))
            with self.assertRaisesRegex(ValueError, "unique"):
                MATRIX.load_matrix(path)
            path.write_text(json.dumps(good))
            with self.assertRaisesRegex(ValueError, "affinity"):
                MATRIX.load_matrix(path, affinity={cpu + 1})

    def test_result_scalar_scan_uses_head_and_tail_for_huge_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            queries, top = FIXTURE.walk_document("ordered", Path(temporary) / "checkpoint")
            path = Path(temporary) / "result.json"
            path.write_text(json.dumps(top, indent=2, sort_keys=True))
            full = MATRIX.scan_result_scalars(path)
            self.assertTrue(full["complete_parse"])
            self.assertEqual((full["completed_nodes"], full["scheduled_nodes"], full["traversal_seconds"]), (5, 6, 0.2))
            scanned = MATRIX.scan_result_scalars(path, full_parse_limit=0)
            self.assertFalse(scanned["complete_parse"])
            for key in ("completed_nodes", "scheduled_nodes", "traversal_seconds", "status", "schema",
                        "max_scheduled_finite_rank", "routed_domains", "events"):
                self.assertEqual(scanned[key], full[key], key)
            self.assertNotIn("checkpoint", scanned)
            self.assertEqual(MATRIX.scan_result_scalars(Path(temporary) / "missing.json"), {"present": False})


class MatrixRunTests(unittest.TestCase):
    def test_fake_matrix_produces_case_summaries_results_and_receipt(self):
        affinity = sorted(os.sched_getaffinity(0))
        if len(affinity) < 2 or shutil.which("taskset") is None or shutil.which("nice") is None:
            self.skipTest("needs two permitted CPUs plus taskset and nice")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture_inputs(root)
            cpus = f"{affinity[-2]},{affinity[-1]}"
            document = matrix_document(root, cpus, 2)
            document["cases"].append(dict(document["cases"][0], name="fg-ready", publication_policy="ready",
                                          inspection_workers=1, native_options=["--sample-seconds", "0.1"]))
            document["cases"][0]["native_options"] = ["--sample-seconds", "0.1"]
            path = root / "matrix.json"
            path.write_text(json.dumps(document))
            output = root / "out"
            with patch("sys.stdout", io.StringIO()) as stdout:
                self.assertEqual(MATRIX.main([str(path), "--output", str(output), "--audit", "--nice", "5"]), 0)
            self.assertIn("[fg-ordered] exit 0", stdout.getvalue())
            for name, policy in (("fg-ordered", "ordered"), ("fg-ready", "ready")):
                with self.subTest(name=name):
                    case_dir = output / name
                    command = json.loads((case_dir / "command.json").read_text())
                    self.assertEqual((Path(command[0]).name, command[1], command[2]), ("nice", "-n", "5"))
                    self.assertEqual((Path(command[3]).name, command[4], command[5]), ("taskset", "-c", cpus))
                    self.assertEqual(command[command.index("--publication-policy") + 1], policy)
                    self.assertEqual(command[command.index("--max-queries") + 1], "2")
                    self.assertIn("--reuse-initial-d-bands", command)
                    self.assertEqual("--inspection-workers" in command, policy == "ready")
                    self.assertEqual(command[command.index("--sample-seconds") + 1], "0.1")
                    summary = json.loads((case_dir / "summary.json").read_text())
                    self.assertEqual(summary["exit_status"], 0)
                    self.assertEqual(summary["supervisor"]["exit_status"], 0)
                    self.assertEqual(summary["supervisor"]["state"], "completed")
                    self.assertGreater(summary["whole_command"]["wall_seconds"], 0)
                    self.assertGreater(summary["whole_command"]["max_rss_kib"], 0)
                    self.assertIsNotNone(summary["whole_command"]["cpu_seconds"])
                    self.assertEqual(summary["result"]["traversal_seconds"], 0.2)
                    self.assertEqual(summary["result"]["prepared_seconds"], 0.1)
                    self.assertEqual(summary["native_inspections"], 5)
                    self.assertEqual(summary["native_by_phase"], {"Apply": 4, "Route": 1})
                    self.assertEqual(summary["aliases"], 1)
                    self.assertEqual(summary["result"]["events"], 9)
                    self.assertEqual(summary["result"]["max_scheduled_finite_rank"], 2)
                    self.assertEqual(summary["events"]["checkpoint_seconds"], 0.5)
                    self.assertEqual(summary["events"]["last_checkpoint"]["generation"], 2)
                    self.assertGreater(summary["peak_sampled_rss_bytes"], 0)
                    self.assertEqual(summary["audit"]["verdict"], "PASS", summary["audit"])
                    self.assertEqual(summary["inputs"]["executable_sha256"], MATRIX.digest(root / "fake-rustred"))
                    self.assertTrue((case_dir / "run" / "supervisor-result.json").is_file())
                    self.assertTrue((case_dir / "audit.json").is_file())
                    request = json.loads((case_dir / "run" / "request.json").read_text())
                    self.assertEqual(request["cpus"], [affinity[-2], affinity[-1]])
            results = (output / "RESULTS.md").read_text()
            self.assertIn("| fg-ordered | ordered | 2 |", results)
            self.assertIn("| fg-ready | ready | 2 |", results)
            self.assertIn("4 / 1", results)
            self.assertNotIn("ETA", results)
            receipt = json.loads((output / "matrix-receipt.json").read_text())
            self.assertEqual(receipt["schema"], MATRIX.RECEIPT_SCHEMA)
            self.assertEqual([case["name"] for case in receipt["cases"]], ["fg-ordered", "fg-ready"])
            self.assertEqual(receipt["cases"][0]["executable_sha256"], MATRIX.digest(root / "fake-rustred"))
            self.assertEqual(receipt["cases"][0]["queries_sha256"], MATRIX.digest(root / "queries.json"))
            self.assertEqual(receipt["matrix_sha256"], MATRIX.digest(path))
            self.assertEqual(receipt["cases"][0]["status"], "completed")
            with patch("sys.stdout", io.StringIO()), patch("sys.stderr", io.StringIO()):
                with self.assertRaises(SystemExit):
                    MATRIX.main([str(path), "--output", str(output)])
                self.assertEqual(MATRIX.main([str(path), "--output", str(output), "--skip-existing"]), 0)
            receipt = json.loads((output / "matrix-receipt.json").read_text())
            self.assertEqual({case["status"] for case in receipt["cases"]}, {"skipped_existing"})

    def test_dry_run_writes_commands_only_and_failed_case_is_reported(self):
        affinity = sorted(os.sched_getaffinity(0))
        if shutil.which("taskset") is None or shutil.which("nice") is None:
            self.skipTest("needs taskset and nice")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture_inputs(root)
            path = root / "matrix.json"
            path.write_text(json.dumps(matrix_document(root, str(affinity[-1]), 1, native_options=["--sample-seconds", "0.1"])))
            output = root / "dry"
            with patch("sys.stdout", io.StringIO()) as stdout:
                self.assertEqual(MATRIX.main([str(path), "--output", str(output), "--dry-run"]), 0)
            self.assertIn("--publication-policy ordered", stdout.getvalue())
            self.assertTrue((output / "fg-ordered" / "command.json").is_file())
            self.assertFalse((output / "fg-ordered" / "run").exists())
            self.assertFalse((output / "RESULTS.md").exists())
            self.assertEqual(json.loads((output / "matrix-receipt.json").read_text())["cases"][0]["status"], "dry_run")
            failing = root / "failing"
            with patch.dict(os.environ, {"FAKE_EXIT": "3"}), patch("sys.stdout", io.StringIO()):
                self.assertEqual(MATRIX.main([str(path), "--output", str(failing), "--case", "fg-ordered"]), 1)
            summary = json.loads((failing / "fg-ordered" / "summary.json").read_text())
            self.assertEqual(summary["exit_status"], 3)
            self.assertEqual(summary["supervisor"]["exit_status"], 3)
            self.assertEqual(summary["supervisor"]["state"], "failed")
            self.assertIn("| fg-ordered | ordered | 1 |", (failing / "RESULTS.md").read_text())
            self.assertEqual(json.loads((failing / "matrix-receipt.json").read_text())["cases"][0]["status"], "exit_3")
            with patch("sys.stderr", io.StringIO()) as stderr, self.assertRaises(SystemExit):
                MATRIX.main([str(path), "--output", str(root / "unknown"), "--case", "nope"])
            self.assertIn("unknown cases", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
