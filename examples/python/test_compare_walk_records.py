"""Strict and multiset comparisons on synthetic walks; no native executable."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


COMPARE = module("compare_walk_records")
FIXTURE = module("test_audit_owner_domain_walk")


def walk(directory, policy="ordered", mutate=None, order=None):
    queries, top = FIXTURE.walk_document(policy, Path(directory) / "checkpoint")
    if mutate is not None:
        mutate(top)
    FIXTURE.write_walk(Path(directory) / "result.json", Path(directory) / "events.jsonl",
                       Path(directory) / "checkpoint", queries, top, order)
    return Path(directory) / "result.json"


class StrictComparisonTests(unittest.TestCase):
    def test_identical_and_timing_only_differences_pass(self):
        def slower(top):
            top["prepared_seconds"] = 9.0
            top["traversal_seconds"] = 8.0
            top["checkpoint"]["duration_seconds"] = 7.0
            top["checkpoint"]["generation"] = 5
            top["parallel"]["admission_preparation"]["preparation_wall_seconds"] = 3.0
            top["workers"] = 7
            for record in top["domains"]:
                record["seconds"] = 1.5

        with tempfile.TemporaryDirectory() as temporary:
            a = walk(Path(temporary) / "a")
            b = walk(Path(temporary) / "b", mutate=slower)
            report = COMPARE.strict_compare(a, b)
            self.assertEqual(report["verdict"], "PASS", report)
            self.assertEqual(report["differing_records"], 0)
            self.assertEqual(report["top_level_differences"], {})
            self.assertEqual(report["a"]["records"], 6)
            self.assertEqual(report["a"]["native_by_phase"], {"Apply": 4, "Route": 1})

    def test_counter_geometry_and_order_differences_fail_with_examples(self):
        def counter(top):
            top["domains"][3]["stats"]["native_operations"] = 11

        def geometry(top):
            top["domains"][4]["upper"] = [0, 3]

        def top_level(top):
            top["successors"] = 4

        with tempfile.TemporaryDirectory() as temporary:
            a = walk(Path(temporary) / "a")
            for name, mutate, order in (("counter", counter, None), ("geometry", geometry, None),
                                        ("top", top_level, None), ("order", None, [1, 0, 2, 3, 4, 5])):
                with self.subTest(name=name):
                    b = walk(Path(temporary) / name, mutate=mutate, order=order)
                    report = COMPARE.strict_compare(a, b)
                    self.assertEqual(report["verdict"], "FAIL")
                    if name == "top":
                        self.assertEqual(list(report["top_level_differences"]), ["successors"])
                        self.assertEqual(report["differing_records"], 0)
                    elif name == "order":
                        self.assertEqual(report["differing_records"], 2)
                        self.assertEqual(report["record_examples"][0]["id_a"], 0)
                        self.assertEqual(report["record_examples"][0]["id_b"], 1)
                    else:
                        self.assertEqual(report["differing_records"], 1)
                        self.assertEqual(report["record_examples"][0]["differing_keys"],
                                         ["stats"] if name == "counter" else ["upper"])
            short = walk(Path(temporary) / "short", mutate=lambda top: top["domains"].pop())
            report = COMPARE.strict_compare(a, short)
            self.assertEqual(report["verdict"], "FAIL")
            self.assertEqual((report["a"]["records"], report["b"]["records"]), (6, 5))
            report = COMPARE.strict_compare(a, walk(Path(temporary) / "ignored", mutate=top_level), ignore_top=["successors"])
            self.assertEqual(report["verdict"], "PASS")


class MultisetComparisonTests(unittest.TestCase):
    def test_reordered_records_are_equal_multisets_and_cross_policy_tolerance(self):
        with tempfile.TemporaryDirectory() as temporary:
            a = walk(Path(temporary) / "a", "ordered")
            b = walk(Path(temporary) / "b", "ready", order=[5, 4, 3, 2, 1, 0])
            report = COMPARE.multiset_compare(a, b)
            self.assertEqual(report["verdict"], "PASS", report)
            self.assertTrue(report["equal_multisets"])
            self.assertEqual(report["shapes_only_in_a"], 0)
            self.assertEqual(report["native_by_phase"]["Apply"], {"a": 4, "b": 4, "within_tolerance": True})

            def fewer_native(top):
                top["completed_nodes"] = 4

            c = walk(Path(temporary) / "c", mutate=fewer_native)
            self.assertEqual(COMPARE.multiset_compare(a, c)["verdict"], "FAIL")
            report = COMPARE.multiset_compare(a, c, native_tolerance=0.25)
            self.assertEqual(report["verdict"], "PASS")
            self.assertTrue(report["native_within_tolerance"])

            def delegated_instead(top):
                # The same logical domain discharged by delegation rather than natively.
                top["domains"][3].update(record_kind="delegated_not_inspected", local_inspection_finished=False,
                                         responsibility_status="discharged_by_representative")

            d = walk(Path(temporary) / "d", mutate=delegated_instead)
            report = COMPARE.multiset_compare(a, d, native_tolerance=0.25)
            self.assertEqual(report["verdict"], "PASS", report)
            self.assertEqual(report["shape"], "discharged")
            self.assertEqual(report["record_kinds"]["b"]["delegated_not_inspected"], 2)
            report = COMPARE.multiset_compare(a, d, native_tolerance=0.25, shape="kind")
            self.assertEqual(report["verdict"], "FAIL")
            self.assertEqual((report["shapes_only_in_a"], report["shapes_only_in_b"]), (1, 1))
            self.assertIn('"native_inspection"', report["examples_only_in_a"][0])
            self.assertIn('"delegated_not_inspected"', report["examples_only_in_b"][0])

            def failed_record(top):
                top["domains"][3]["error"] = "fixture failure"
                top["domains"][3]["local_inspection_finished"] = False

            e = walk(Path(temporary) / "e", mutate=failed_record)
            report = COMPARE.multiset_compare(a, e)
            self.assertEqual(report["verdict"], "FAIL")
            self.assertEqual((report["shapes_only_in_a"], report["shapes_only_in_b"]), (1, 1))
            self.assertIn('"discharged":false', report["examples_only_in_b"][0])
            self.assertEqual(COMPARE.multiset_compare(a, e, shape="geometry")["verdict"], "PASS")
            with self.assertRaises(ValueError):
                COMPARE.multiset_compare(a, e, shape="other")

    def test_cli_modes_and_exit_status(self):
        with tempfile.TemporaryDirectory() as temporary:
            a = walk(Path(temporary) / "a")
            b = walk(Path(temporary) / "b", order=[1, 0, 2, 3, 4, 5])
            output = Path(temporary) / "comparison.json"
            result = subprocess.run([sys.executable, "-B", COMPARE.__file__, "--mode", "multiset", str(a), str(b),
                                     "--output", str(output)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(output.read_text())["verdict"], "PASS")
            result = subprocess.run([sys.executable, "-B", COMPARE.__file__, "--mode", "strict", str(a), str(b)],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(json.loads(result.stdout)["verdict"], "FAIL")
            result = subprocess.run([sys.executable, "-B", COMPARE.__file__, "--mode", "multiset", str(a), str(b),
                                     "--native-tolerance", "1.5"], capture_output=True, text=True)
            self.assertEqual(result.returncode, 2)
            result = subprocess.run([sys.executable, "-B", COMPARE.__file__, "--mode", "multiset", "--shape", "kind",
                                     str(a), str(b)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)["shape"], "kind")


if __name__ == "__main__":
    unittest.main()
