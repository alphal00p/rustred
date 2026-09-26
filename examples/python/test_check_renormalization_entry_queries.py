"""Checker tests: mutated bounds, order, counts and digests must each fail.

The checker is exercised on planner output produced with a fake
`entry-domain-plan` executable; no native solver runs.
"""
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


CHECK = module("check_renormalization_entry_queries")
PLANNER_TESTS = module("test_plan_renormalization_entry_queries")


def load(path):
    return json.loads(Path(path).read_text())


def dump(path, document):
    Path(path).write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")


def run_checker(out, *extra):
    stdout = io.StringIO()
    with contextlib.redirect_stdout(stdout):
        code = CHECK.main(["--queries", str(out / "queries.json"), "--receipt", str(out / "entry-plan-receipt.json"), *extra])
    return code, json.loads(stdout.getvalue())


def plan(root, *extra, executable=True):
    files = PLANNER_TESTS.l2_family(root)
    args = list(extra)
    if executable:
        args += ["--executable", str(PLANNER_TESTS.fake_executable(root, "correct"))]
    code, text = PLANNER_TESTS.run_planner(files, root / "out", *args)
    assert code == 0, text
    return root / "out"


def rewrite_queries(out, mutate):
    """Apply a mutation to queries.json and refresh the receipt's digest and size."""
    document = load(out / "queries.json")
    mutate(document)
    data = (json.dumps(document, sort_keys=True, indent=2) + "\n").encode()
    (out / "queries.json").write_bytes(data)
    receipt = load(out / "entry-plan-receipt.json")
    receipt["summary"]["queries_sha256"] = hashlib.sha256(data).hexdigest()
    receipt["summary"]["queries_bytes"] = len(data)
    dump(out / "entry-plan-receipt.json", receipt)


class CheckerTests(unittest.TestCase):
    def test_passes_on_planner_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))
            code, report = run_checker(out)
            self.assertEqual((code, report["status"], report["failures"]), (0, "pass", []), report)
            self.assertTrue(report["rust_counts_verified"])
            self.assertEqual(report["control_count"], 1324)
            self.assertEqual(report["probes"]["disagreements"], 0)
            self.assertEqual((report["helper_count"], report["root_count"], report["query_count"]), (3, 5, 8))

    def test_closed_form_only_requires_flag(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp), executable=False)
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertTrue(any("lacks Rust target counts" in f for f in report["failures"]))
            code, report = run_checker(out, "--allow-closed-form-only")
            self.assertEqual((code, report["status"]), (0, "pass"), report)
            self.assertFalse(report["rust_counts_verified"])

    def test_mutated_bound_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))

            def loosen(document):
                row = [q for q in document["queries"] if q["id"] == "phys-d10-a19-r9-111"][0]
                row["upper"][0] += 1

            rewrite_queries(out, loosen)
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertTrue(any("phys-d10-a19-r9-111: root row" in f for f in report["failures"]), report["failures"])

    def test_reordered_helper_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))

            def swap(document):
                rows = document["queries"]
                rows[0], rows[1] = rows[1], rows[0]

            rewrite_queries(out, swap)
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertTrue(any("111: query ids/order" in f for f in report["failures"]), report["failures"])
            self.assertTrue(any("exactly one helper, first" in f for f in report["failures"]), report["failures"])

    def test_wrong_count_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))
            receipt = load(out / "entry-plan-receipt.json")
            root = receipt["owners"][0]["roots"][0]
            root["target_count"] = str(int(root["target_count"]) + 1)
            dump(out / "entry-plan-receipt.json", receipt)
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertTrue(any("Rust count" in f for f in report["failures"]), report["failures"])

    def test_stale_sha_and_size_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))
            (out / "queries.json").write_bytes((out / "queries.json").read_bytes() + b"\n")
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertIn("queries sha256 differs from the receipt", report["failures"])
            self.assertIn("queries byte size differs from the receipt", report["failures"])

    def test_unknown_field_and_bad_shape_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))
            rewrite_queries(out, lambda document: document["queries"][0].__setitem__("note", "extra"))
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertTrue(any("row field set" in f for f in report["failures"]), report["failures"])
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))
            rewrite_queries(out, lambda document: document["queries"][1].__setitem__("id", "x" * 129))
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertTrue(any("id length" in f for f in report["failures"]), report["failures"])

    def test_missing_helper_and_extra_root_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))
            rewrite_queries(out, lambda document: document["queries"].pop(0))
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertTrue(any("111: query ids/order" in f for f in report["failures"]), report["failures"])
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))

            def duplicate_root(document):
                extra = dict(document["queries"][2])
                extra["id"] = "phys-d8-a15-r7-111"
                document["queries"].insert(3, extra)

            rewrite_queries(out, duplicate_root)
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertTrue(any("summary query_count" in f for f in report["failures"]), report["failures"])

    def test_receipt_physics_changes_are_detected(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))
            receipt = load(out / "entry-plan-receipt.json")
            receipt["physics"]["gauge_parameter_powers"] = 1
            dump(out / "entry-plan-receipt.json", receipt)
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertTrue(any("receipt root ids/order" in f for f in report["failures"]), report["failures"])
        with tempfile.TemporaryDirectory() as tmp:
            out = plan(Path(tmp))
            receipt = load(out / "entry-plan-receipt.json")
            receipt["family_closure_claim"] = True
            dump(out / "entry-plan-receipt.json", receipt)
            code, report = run_checker(out)
            self.assertEqual(code, 1)
            self.assertIn("receipt must keep family_closure_claim false", report["failures"])


class PredicateTests(unittest.TestCase):
    def domain(self, owner="110", rank=None, power=None, d_min=None, d_max=None, upper=None):
        return {"owner": owner, "lower": [0] * len(owner), "upper": upper or [None] * len(owner),
                "max_numerator_rank": rank,
                "power_bounds": {"max_positive_power": power, "min_power_difference": d_min, "max_power_difference": d_max}}

    def test_contains_follows_domain_contains(self):
        helper = self.domain(rank=6)
        root = self.domain(rank=6, power=16, d_min=10, d_max=10, upper=[4, 4, 6])
        self.assertTrue(CHECK.contains(helper, root))
        self.assertFalse(CHECK.contains(root, helper))
        self.assertFalse(CHECK.contains(self.domain(rank=5), root))
        self.assertFalse(CHECK.contains(helper, self.domain(rank=None)))
        self.assertTrue(CHECK.contains(self.domain(rank=None), self.domain(rank=None)))
        self.assertFalse(CHECK.contains(self.domain(power=16), self.domain(power=None)))
        self.assertTrue(CHECK.contains(self.domain(power=16), self.domain(power=16)))
        self.assertFalse(CHECK.contains(self.domain(d_min=9), self.domain(d_min=8)))
        self.assertTrue(CHECK.contains(self.domain(d_min=9), self.domain(d_min=10)))
        self.assertFalse(CHECK.contains(self.domain(d_max=10), self.domain(d_max=11)))
        self.assertFalse(CHECK.contains(self.domain(owner="101"), root))
        self.assertFalse(CHECK.contains(self.domain(upper=[3, None, None]), self.domain(upper=[4, None, None])))
        self.assertTrue(CHECK.contains(self.domain(upper=[4, None, None]), self.domain(upper=[4, 0, 0])))

    def test_closed_form_control(self):
        self.assertEqual(CHECK.closed_form_count(9, 6, 11, 2, 9, None), 1324)
        self.assertEqual(CHECK.closed_form_count(3, 0, 19, 9, 10, 10), 36)
        self.assertEqual(CHECK.closed_form_count(2, 1, 8, 5, 3, None), 112)

    def test_probes_detect_a_loose_box(self):
        import random
        mask = "110"
        query = self.domain(owner=mask, rank=9, power=19, d_min=10, d_max=10, upper=[17, 17, 9])
        disagreements, inside = CHECK.probe_root(random.Random(3), query, mask, 19, 9, 10, 10, 400)
        self.assertEqual(disagreements, 0)
        self.assertGreater(inside, 0)
        tight = dict(query, upper=[5, 17, 9])
        disagreements, _ = CHECK.probe_root(random.Random(3), tight, mask, 19, 9, 10, 10, 400)
        self.assertGreater(disagreements, 0)

    def test_expected_bounds_tables(self):
        self.assertEqual(CHECK.expected_bounds("connected", 0, 5, 0, [10, 9], "nested", "widest"),
                         [("phys", 16, 6, 10, 10), ("phys", 14, 5, 9, 9)])
        self.assertEqual(CHECK.expected_bounds("factorized", 2, 5, 0, [10, 9], "nested", "widest"),
                         [("nested", 22, 13, 9, None)])
        self.assertEqual(CHECK.expected_bounds("factorized", 2, 5, 0, [10, 9], "omit", "widest"), [])
        self.assertEqual(CHECK.expected_bounds("non_entry", 0, 5, 1, [10], "nested", "widest"), [("conv", 17, 7, 10, 10)])
        self.assertEqual(CHECK.expected_bounds("factorized", 0, 4, 0, [7], "nested", "omit"), [("nested", 19, 12, 7, None)])


class FiveLoopCheckerTests(unittest.TestCase):
    def test_fast_five_loop_plan_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            code, text = PLANNER_TESTS.run_planner(PLANNER_TESTS.FIVE_LOOP, root / "out", "--classification",
                                                   str(PLANNER_TESTS.FIVE_LOOP["classification"]), loops=5)
            self.assertEqual(code, 0, text)
            code, report = run_checker(root / "out", "--allow-closed-form-only", "--probes", "40")
            self.assertEqual((code, report["status"]), (0, "pass"), report)
            self.assertEqual(report["class_counts"], {"connected": 41, "factorized": 18, "non_entry": 8})
            self.assertEqual(report["closed_form_total"], "271990954170")


if __name__ == "__main__":
    unittest.main()
