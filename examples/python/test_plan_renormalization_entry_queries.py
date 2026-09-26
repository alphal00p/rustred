"""Planner and match-summary tests: offline input planning only; no solver runs.

The five-loop regression uses the committed classification fixture; the full
skeleton enumeration (about 90 s) runs only with RUSTRED_SLOW_TESTS=1.
"""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import sys
import tempfile
import unittest


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


PLAN = module("plan_renormalization_entry_queries")
SUMMARY = module("summarize_owner_domain_match")
HERE = Path(__file__).resolve().parent
FIXTURES = HERE / "fixtures"
INPUTS = HERE.parent / "input"
FIVE_LOOP = {
    "manifest": FIXTURES / "tide_five_loop_owner_selection.json",
    "momenta": INPUTS / "tide_five_loop_manifest.json",
    "witnesses": INPUTS / "tide_five_loop_parent_vertices.json",
    "classification": FIXTURES / "tide_five_loop_skeleton_classification.json",
}
NON_ENTRY = ["000011001001011", "001000100101111", "010111011000001", "011001110110100",
             "011011111101001", "101010000110001", "110010101101011", "111000100011101"]


def dump(path, document):
    path.write_text(json.dumps(document, indent=1) + "\n")
    return path


def l2_family(root, masks=("111", "110", "100"), factorized=("110",)):
    """Sunrise family: momenta k1, k2, k1-k2; parent vertices (1,-2,-3), (-1,2,3)."""
    momenta = {"loop_count": 2, "coordinate_count": 3,
               "momenta": [{"index_one_based": 1, "momentum": "k1"}, {"index_one_based": 2, "momentum": "k2"},
                           {"index_one_based": 3, "momentum": "k1-k2"}],
               "representatives": [{"sector_id": int(m, 2), "factorized": m in factorized} for m in masks]}
    witness = {"schema": PLAN.WITNESS_SCHEMA, "loop_count": 2, "coordinate_count": 3,
               "parents": [{"sector_id": 7, "sector_bits": "111", "vertices": [[1, -2, -3], [-1, 2, 3]]}]}
    selection = {"owners": [{"mask": m, "representative": int(m, 2), "parent": 7, "ordinal": i, "published_sector": int(m, 2)}
                            for i, m in enumerate(masks)]}
    return {"momenta": dump(root / "momenta.json", momenta), "witnesses": dump(root / "witnesses.json", witness),
            "manifest": dump(root / "selection.json", selection)}


K4_MOMENTA = {1: (1, 0, 0), 2: (0, 1, 0), 3: (0, 0, 1), 4: (1, 0, -1), 5: (0, 1, 1), 6: (1, 1, 0)}
K4_WITNESS = {"schema": PLAN.WITNESS_SCHEMA, "loop_count": 3, "coordinate_count": 6,
              "parents": [{"sector_id": 63, "sector_bits": "111111",
                           "vertices": [[1, 2, -6], [-1, 3, 4], [-2, -3, 5], [-4, -5, 6]]}]}


def run_planner(files, output, *extra, loops=2):
    argv = ["--loops", str(loops), "--manifest", str(files["manifest"]), "--momenta", str(files["momenta"]),
            "--parent-witnesses", str(files["witnesses"]), "--output-directory", str(output), "--quiet", *extra]
    stderr, stdout = io.StringIO(), io.StringIO()
    with contextlib.redirect_stderr(stderr), contextlib.redirect_stdout(stdout):
        code = PLAN.main(argv)
    return code, stderr.getvalue() + stdout.getvalue()


def load(path):
    return json.loads(Path(path).read_text())


def fake_executable(root, mode):
    """Shell wrapper around a tiny closed-form plan generator (mode: correct, wrong, fail)."""
    helper = root / "fake_plan.py"
    helper.write_text(
        "import json, math, sys\n"
        "spec = json.load(sys.stdin)\n"
        "mode, log = sys.argv[1], sys.argv[2]\n"
        "open(log, 'a').write(json.dumps(spec, sort_keys=True) + '\\n')\n"
        "if mode == 'fail':\n    sys.exit(1)\n"
        "def count(mask, b):\n"
        "    t = mask.count('1'); m = len(mask) - t; total = 0\n"
        "    lo = max(t, b['min_power_difference'] or 0)\n"
        "    for a in range(lo, b['max_positive_power'] + 1):\n"
        "        r0 = 0 if b['max_power_difference'] is None else max(0, a - b['max_power_difference'])\n"
        "        r1 = b['max_numerator_rank'] if b['min_power_difference'] is None else min(b['max_numerator_rank'], a - b['min_power_difference'])\n"
        "        if m == 0: r1 = min(r1, 0)\n"
        "        if r1 < r0: continue\n"
        "        total += (1 if t == 0 else math.comb(a - 1, t - 1)) * sum(1 if m == 0 else math.comb(r + m - 1, m - 1) for r in range(r0, r1 + 1))\n"
        "    return total\n"
        "sectors = [{'sector': s, 'target_count': str(count(s, spec['budget']) + (mode == 'wrong'))} for s in spec['sectors']]\n"
        "plan = {'schema': 'rustred.entry-domain-plan.json.v1', 'operation': 'entry_domain_plan', 'counts_exact': True,\n"
        "        'arity': len(spec['sectors'][0]), 'budget': spec['budget'], 'sectors': sectors,\n"
        "        'max_positive_layers_per_sector': spec['max_positive_layers_per_sector'],\n"
        "        'total_target_count': str(sum(int(s['target_count']) for s in sectors)), 'preview': {'emitted': 0, 'targets': []}}\n"
        "print(json.dumps(plan, sort_keys=True))\n")
    script = root / f"fake-rustred-{mode}"
    script.write_text(f'#!/bin/sh\ntest "$1" = entry-domain-plan || exit 3\n'
                      f'exec "{sys.executable}" "{helper}" {mode} "{root}/specs-{mode}.jsonl"\n')
    script.chmod(0o755)
    return script


class ClassificationTests(unittest.TestCase):
    def test_l2_family_classes_and_queries(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root)
            code, text = run_planner(files, root / "out")
            self.assertEqual(code, 0, text)
            classification = load(root / "out" / "skeleton-classification.json")["owners"]
            self.assertEqual(classification["111"]["V4min"], 0)
            self.assertEqual(classification["111"]["skeleton_counts_by_V4"], {"0": 1})
            self.assertEqual(classification["110"]["V4min"], 1)  # figure-eight realization
            self.assertEqual(classification["110"]["skeleton_counts_by_V4"], {"1": 1})
            self.assertFalse(classification["100"]["entry_capable"])
            receipt = load(root / "out" / "entry-plan-receipt.json")
            self.assertEqual([r["class"] for r in receipt["owners"]], ["connected", "factorized", "non_entry"])
            self.assertEqual(receipt["summary"]["class_counts"], {"connected": 1, "factorized": 1, "non_entry": 1})
            ids = [q["id"] for q in load(root / "out" / "queries.json")["queries"]]
            self.assertEqual(ids, ["owner-anchor-r9-anone-111", "phys-d10-a19-r9-111", "phys-d9-a17-r8-111",
                                   "owner-anchor-r5-anone-110", "nested-d3p-a8-r5-110",
                                   "owner-anchor-r9-anone-100", "conv-d10-a19-r9-100", "conv-d9-a17-r8-100"])
            nested = [q for q in load(root / "out" / "queries.json")["queries"] if q["id"].startswith("nested")][0]
            self.assertEqual(nested["upper"], [6, 6, 5])
            self.assertEqual(nested["power_bounds"], {"max_positive_power": 8, "min_power_difference": 3,
                                                      "max_power_difference": None})
            self.assertIs(receipt["family_closure_claim"], False)
            self.assertIs(receipt["descendant_clipping"], False)
            self.assertEqual(receipt["summary"]["count_source"], "closed_form_only")

    def test_l2_omit_and_box_modes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root)
            code, text = run_planner(files, root / "omit", "--factorized-roots", "omit", "--non-entry-roots", "omit")
            self.assertEqual(code, 0, text)
            receipt = load(root / "omit" / "entry-plan-receipt.json")
            self.assertEqual([len(r["roots"]) for r in receipt["owners"]], [2, 0, 0])
            self.assertEqual([r["helper"] for r in receipt["owners"]][1:], [None, None])
            self.assertEqual(receipt["summary"]["query_count"], 3)
            code, text = run_planner(files, root / "box", "--factorized-roots", "box")
            self.assertEqual(code, 0, text)
            ids = [q["id"] for q in load(root / "box" / "queries.json")["queries"] if q["owner"] == "110"]
            self.assertEqual(ids, ["owner-anchor-r8-anone-110", "fact-d10-a18-r8-110", "fact-d9-a16-r7-110"])
            code, text = run_planner(files, root / "none", "--factorized-roots", "omit", "--non-entry-roots", "omit",
                                     "--difference-set", "10")
            self.assertEqual(code, 0, text)
            self.assertEqual(load(root / "none" / "entry-plan-receipt.json")["summary"]["query_count"], 2)

    def test_k4_family_hand_cases(self):
        parents = PLAN.validate_witnesses(K4_WITNESS, K4_MOMENTA, 3)
        masks = ["111111", "011111", "011101", "001111", "001110"]
        owners, enumeration = PLAN.classify_owners(masks, parents, {}, 3, 3, 4)
        expected = {"111111": (0, {"0": 1}),          # K4 itself
                    "011111": (0, {"0": 1, "1": 1}),  # doubled C4 cosimplifies here; quartic skeleton too
                    "011101": (2, {"2": 1}),          # four parallel lines
                    "001111": (1, {"1": 1}),          # loop plus triple line
                    "001110": (2, {"2": 1})}          # bouquet of three loops
        for mask, (excess, counts) in expected.items():
            self.assertTrue(owners[mask]["entry_capable"], mask)
            self.assertEqual((owners[mask]["V4min"], owners[mask]["skeleton_counts_by_V4"]), (excess, counts), mask)
        self.assertEqual(enumeration["distinct_skeletons_by_V4"], {"0": 2, "1": 2, "2": 2})
        self.assertEqual(enumeration["unmatched_reduced_skeletons_by_V4_and_t"], {})
        with self.assertRaisesRegex(PLAN.PlanError, "share a connected realization form"):
            PLAN.classify_owners(masks + ["110110"], parents, {}, 3, 3, 4)  # another four-line banana

    def test_canonical_form_is_isomorphism_invariant(self):
        square = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)]
        relabelled = [(3, 2), (2, 0), (0, 1), (1, 3), (3, 0)]
        self.assertEqual(PLAN.canonical_form(square), PLAN.canonical_form(relabelled))
        square_with_loop = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 0)]
        self.assertNotEqual(PLAN.canonical_form(square), PLAN.canonical_form(square_with_loop))
        self.assertEqual(PLAN.canonical_form([(0, 0), (0, 0)]), ((2,),))


class WitnessValidationTests(unittest.TestCase):
    def momenta(self):
        return {1: (1, 0), 2: (0, 1), 3: (1, -1)}

    def witness(self, vertices, **overrides):
        document = {"schema": PLAN.WITNESS_SCHEMA, "loop_count": 2, "coordinate_count": 3,
                    "parents": [{"sector_id": 7, "sector_bits": "111", "vertices": vertices}]}
        document.update(overrides)
        return document

    def test_valid_witness(self):
        parents = PLAN.validate_witnesses(self.witness([[1, -2, -3], [-1, 2, 3]]), self.momenta(), 2)
        self.assertEqual(list(parents), [7])

    def test_flipped_sign_is_rejected(self):
        with self.assertRaisesRegex(PLAN.PlanError, "nonzero momentum sum"):
            PLAN.validate_witnesses(self.witness([[1, 2, -3], [-1, -2, 3]]), self.momenta(), 2)

    def test_missing_slot_is_rejected(self):
        # zero-sum vertices, but slot 1 occurs three times
        with self.assertRaisesRegex(PLAN.PlanError, "exactly twice"):
            PLAN.validate_witnesses(self.witness([[1, -2, -3], [-1, 2, 3], [1, -1]]), self.momenta(), 2)
        with self.assertRaisesRegex(PLAN.PlanError, "differ from sector bits"):
            PLAN.validate_witnesses(self.witness([[1, -1], [2, -2]]), self.momenta(), 2)

    def test_wrong_loop_count_is_rejected(self):
        with self.assertRaisesRegex(PLAN.PlanError, "loop_count"):
            PLAN.validate_witnesses(self.witness([[1, -2, -3], [-1, 2, 3]], loop_count=3), self.momenta(), 2)
        document = self.witness([[1, -1, 3, -3], [2, -2]])
        document["parents"][0]["sector_bits"] = "111"
        with self.assertRaisesRegex(PLAN.PlanError, "disconnected"):
            PLAN.validate_witnesses(document, self.momenta(), 2)

    def test_sector_bits_must_match_sector_id(self):
        document = self.witness([[1, -2, -3], [-1, 2, 3]])
        document["parents"][0]["sector_bits"] = "110"
        with self.assertRaisesRegex(PLAN.PlanError, "sector_bits"):
            PLAN.validate_witnesses(document, self.momenta(), 2)

    def test_committed_five_loop_witness_validates(self):
        momenta = PLAN.load_momenta(load(FIVE_LOOP["momenta"]), 5)
        parents = PLAN.validate_witnesses(load(FIVE_LOOP["witnesses"]), momenta, 5)
        self.assertEqual(sorted(parents), [30527, 30699, 31740, 32745])
        document = load(FIVE_LOOP["witnesses"])
        for parent in document["parents"]:
            self.assertEqual(parent["sector_bits"], format(parent["sector_id"], "015b"))


class BoundTests(unittest.TestCase):
    def test_five_loop_tables(self):
        for excess in range(5):
            self.assertEqual(PLAN.connected_bounds(5, 10, excess, 0),
                             {"A_max": 16 - excess, "R_max": 6 - excess, "D_min": 10, "D_max": 10})
            self.assertEqual(PLAN.connected_bounds(5, 9, excess, 0),
                             {"A_max": 14 - excess, "R_max": 5 - excess, "D_min": 9, "D_max": 9})
            self.assertEqual(PLAN.nested_bounds(5, excess, 0),
                             {"A_max": 24 - excess, "R_max": 15 - excess, "D_min": 9, "D_max": None})

    def test_four_loop_nested_control_envelope(self):
        self.assertEqual(PLAN.nested_bounds(4, 0, 0), {"A_max": 19, "R_max": 12, "D_min": 7, "D_max": None})

    def test_gauge_parameter_shift(self):
        base, shifted = PLAN.connected_bounds(5, 10, 1, 0), PLAN.connected_bounds(5, 10, 1, 1)
        self.assertEqual((shifted["A_max"], shifted["R_max"]), (base["A_max"] + 1, base["R_max"] + 1))
        nested = PLAN.nested_bounds(5, 0, 2)
        self.assertEqual((nested["A_max"], nested["R_max"], nested["D_min"]), (26, 17, 9))

    def test_closed_form_matches_known_control(self):
        self.assertEqual(PLAN.closed_form_count(9, 6, {"A_max": 11, "R_max": 2, "D_min": 9, "D_max": None}), 1324)
        self.assertEqual(PLAN.closed_form_count(9, 6, {"A_max": 5, "R_max": 2, "D_min": 7, "D_max": None}), 0)
        self.assertEqual(PLAN.closed_form_count(3, 0, {"A_max": 4, "R_max": 3, "D_min": None, "D_max": None}), 4)

    def test_negative_upper_refused(self):
        options = {"loops": 2, "gauge_parameter_powers": 0, "difference_set": [1], "factorized_roots": "nested",
                   "non_entry_roots": "widest"}
        with self.assertRaisesRegex(PLAN.PlanError, "negative coordinate upper"):
            PLAN.root_rows("111", PLAN.CLASS_CONNECTED, 0, options)

    def test_gauge_options_via_cli(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root, masks=("111",), factorized=())
            code, text = run_planner(files, root / "xi", "--gauge", "linear-xi", "--gauge-parameter-powers", "1")
            self.assertEqual(code, 0, text)
            ids = [q["id"] for q in load(root / "xi" / "queries.json")["queries"]]
            self.assertEqual(ids, ["owner-anchor-r10-anone-111", "phys-d10-a20-r10-111", "phys-d9-a18-r9-111"])
            self.assertEqual(run_planner(files, root / "bad1", "--gauge-parameter-powers", "1")[0], 2)
            self.assertEqual(run_planner(files, root / "bad2", "--gauge", "linear-xi")[0], 2)
            code, text = run_planner(files, root / "bad3", "--difference-set", "1")
            self.assertEqual(code, 2)
            self.assertIn("negative coordinate upper", text)
            self.assertFalse((root / "bad3").exists())


ROOT_ID = re.compile(r"^(phys|conv|fact)-d\d+-a\d+-r\d+-[01]+$|^nested-d\d+p-a\d+-r\d+-[01]+$")
HELPER_ID = re.compile(r"^owner-anchor-r\d+-a(none|\d+)-[01]+$")


def contains(container, candidate):
    """Domain::contains as in walking/queue.rs (owner, rank, powers, lower, upper)."""
    if container["owner"] != candidate["owner"]:
        return False
    r1, r2 = container["max_numerator_rank"], candidate["max_numerator_rank"]
    if r1 is not None and (r2 is None or r2 > r1):
        return False
    p1, p2 = container["power_bounds"], candidate["power_bounds"]
    for key, later in (("max_positive_power", False), ("min_power_difference", True), ("max_power_difference", False)):
        if p1[key] is not None and (p2[key] is None or (p2[key] < p1[key] if later else p2[key] > p1[key])):
            return False
    return (all(a <= b for a, b in zip(container["lower"], candidate["lower"]))
            and all(a is None or (b is not None and b <= a) for a, b in zip(container["upper"], candidate["upper"])))


class DocumentTests(unittest.TestCase):
    def test_exact_v2_shape_ids_order_and_containment(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root)
            code, text = run_planner(files, root / "out", "--helper-positive-power-owners", "111")
            self.assertEqual(code, 0, text)
            document = load(root / "out" / "queries.json")
            self.assertEqual(set(document), {"schema", "queries"})
            self.assertEqual(document["schema"], "rustred.owner-domain-queries.json.v2")
            helper = None
            seen_owner_blocks = []
            for row in document["queries"]:
                self.assertEqual(set(row), {"id", "owner", "lower", "upper", "max_numerator_rank", "power_bounds"})
                self.assertEqual(set(row["power_bounds"]), {"max_positive_power", "min_power_difference", "max_power_difference"})
                self.assertLessEqual(len(row["id"].encode()), 128)
                if row["id"].startswith("owner-anchor-"):
                    self.assertRegex(row["id"], HELPER_ID)
                    self.assertEqual(row["upper"], [None] * 3)
                    helper = row
                    seen_owner_blocks.append(row["owner"])
                    previous_rank = None
                else:
                    self.assertRegex(row["id"], ROOT_ID)
                    self.assertEqual(row["owner"], helper["owner"])
                    self.assertTrue(contains(helper, row), row["id"])
                    if previous_rank is not None:
                        self.assertLessEqual(row["max_numerator_rank"], previous_rank)
                    previous_rank = row["max_numerator_rank"]
            self.assertEqual(seen_owner_blocks, ["111", "110", "100"])
            bounded = [q for q in document["queries"] if q["id"] == "owner-anchor-r9-a19-111"]
            self.assertEqual(bounded[0]["power_bounds"]["max_positive_power"], 19)
            receipt = load(root / "out" / "entry-plan-receipt.json")
            self.assertEqual(receipt["physics"]["helper_positive_power_owners"], ["111"])
            self.assertEqual(receipt["summary"]["queries_bytes"], (root / "out" / "queries.json").stat().st_size)

    def test_helper_positive_power_owners_from_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root)
            summary = dump(root / "matching-summary.json", {"helper_positive_power_owners": ["110"]})
            code, text = run_planner(files, root / "out", "--helper-positive-power-owners-from", str(summary))
            self.assertEqual(code, 0, text)
            ids = [q["id"] for q in load(root / "out" / "queries.json")["queries"] if q["owner"] == "110"]
            self.assertEqual(ids[0], "owner-anchor-r5-a8-110")
            code, text = run_planner(files, root / "bad", "--helper-positive-power-owners", "101")
            self.assertEqual(code, 2)
            self.assertIn("not selected owners", text)

    def test_two_runs_are_byte_identical(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root)
            fake = fake_executable(root, "correct")
            for name in ("first", "second"):
                code, text = run_planner(files, root / name, "--executable", str(fake))
                self.assertEqual(code, 0, text)
            names = ["queries.json", "entry-plan-receipt.json", "skeleton-classification.json"]
            names += sorted(p.relative_to(root / "first").as_posix() for p in (root / "first" / "entry-plans").iterdir())
            self.assertGreater(len(names), 3)
            for name in names:
                self.assertEqual((root / "first" / name).read_bytes(), (root / "second" / name).read_bytes(), name)
            self.assertNotIn(str(root / "first"), (root / "first" / "entry-plan-receipt.json").read_text())

    def test_output_directory_must_not_exist(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root)
            (root / "taken").mkdir()
            code, text = run_planner(files, root / "taken")
            self.assertEqual(code, 2)
            self.assertIn("already exists", text)


class ExecutableTests(unittest.TestCase):
    def test_fake_executable_groups_and_receipt_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root)
            fake = fake_executable(root, "correct")
            code, text = run_planner(files, root / "out", "--executable", str(fake))
            self.assertEqual(code, 0, text)
            receipt = load(root / "out" / "entry-plan-receipt.json")
            groups = receipt["summary"]["budget_groups"]
            self.assertEqual([g["name"] for g in groups],
                             ["a19-r9-dmin10-dmax10", "a17-r8-dmin9-dmax9", "a8-r5-dmin3-dmaxnone"])
            self.assertEqual([g["sectors"] for g in groups], [["111", "100"], ["111", "100"], ["110"]])
            specs = [json.loads(line) for line in (root / "specs-correct.jsonl").read_text().splitlines()]
            self.assertEqual([s["sectors"] for s in specs], [["111", "100"], ["111", "100"], ["110"]])
            self.assertEqual(specs[0]["budget"], {"max_positive_power": 19, "max_numerator_rank": 9,
                                                  "min_power_difference": 10, "max_power_difference": 10})
            self.assertEqual(specs[0]["schema"], "rustred.entry-domain.json.v1")
            for owner in receipt["owners"]:
                for root_row in owner["roots"]:
                    self.assertEqual(root_row["target_count"], str(root_row["closed_form_count"]))
            self.assertEqual(receipt["summary"]["count_source"], "rust")
            total = sum(int(g["total_target_count"]) for g in groups)
            self.assertEqual(receipt["summary"]["total_target_count"], str(total))
            for group in groups:
                plan = load(root / "out" / group["plan_path"])
                self.assertEqual(plan["total_target_count"], group["total_target_count"])
                self.assertEqual([s["sector"] for s in plan["sectors"]], group["sectors"])
            self.assertEqual(receipt["planner"]["executable"]["path"], str(fake))

    def test_canned_shell_plan_single_group(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root, masks=("111",), factorized=())
            canned = dump(root / "canned.json", {
                "schema": "rustred.entry-domain-plan.json.v1", "counts_exact": True, "total_target_count": "36",
                "sectors": [{"sector": "111", "target_count": "36"}]})
            script = root / "fake-rustred-canned"
            script.write_text(f'#!/bin/sh\ncat "{canned}"\n')
            script.chmod(0o755)
            code, text = run_planner(files, root / "out", "--executable", str(script), "--difference-set", "10")
            self.assertEqual(code, 0, text)
            receipt = load(root / "out" / "entry-plan-receipt.json")
            self.assertEqual(receipt["summary"]["total_target_count"], "36")
            self.assertEqual(receipt["owners"][0]["roots"][0]["target_count"], "36")

    def test_wrong_or_failing_executable_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root)
            code, text = run_planner(files, root / "wrong", "--executable", str(fake_executable(root, "wrong")))
            self.assertEqual(code, 2)
            self.assertIn("!= closed form", text)
            self.assertFalse((root / "wrong").exists())
            code, text = run_planner(files, root / "fail", "--executable", str(fake_executable(root, "fail")))
            self.assertEqual(code, 2)
            self.assertIn("entry-domain-plan failed", text)


class ClassificationReuseTests(unittest.TestCase):
    def test_reuse_and_fixture_equality(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = l2_family(root)
            code, text = run_planner(files, root / "computed")
            self.assertEqual(code, 0, text)
            classification = root / "computed" / "skeleton-classification.json"
            code, text = run_planner(files, root / "reused", "--classification", str(classification))
            self.assertEqual(code, 0, text)
            self.assertEqual((root / "reused" / "queries.json").read_bytes(), (root / "computed" / "queries.json").read_bytes())
            self.assertEqual(load(root / "reused" / "entry-plan-receipt.json")["classification_source"]["kind"], "reused")
            code, text = run_planner(files, root / "equal", "--classification-fixture", str(classification))
            self.assertEqual(code, 0, text)
            self.assertEqual(load(root / "equal" / "entry-plan-receipt.json")["classification_source"]["kind"],
                             "computed_equal_to_fixture")
            document = load(classification)
            document["owners"]["110"]["V4min"] = 0
            broken = dump(root / "broken.json", document)
            code, text = run_planner(files, root / "unequal", "--classification-fixture", str(broken))
            self.assertEqual(code, 2)
            self.assertIn("differs from fixture", text)
            document = load(classification)
            document["inputs"]["momenta_sha256"] = "0" * 64
            stale = dump(root / "stale.json", document)
            code, text = run_planner(files, root / "stale", "--classification", str(stale))
            self.assertEqual(code, 2)
            self.assertIn("different loops, degrees or inputs", text)


class FiveLoopTests(unittest.TestCase):
    def test_fixture_matches_the_expected_split(self):
        fixture = load(FIVE_LOOP["classification"])
        self.assertEqual(fixture["schema"], "rustred.vacuum-skeleton-classification.json.v1")
        self.assertEqual(fixture["enumeration"]["labelled_leaves_by_V4"],
                         {"0": 1256395, "1": 100525, "2": 9236, "3": 1004, "4": 138})
        self.assertEqual(fixture["enumeration"]["distinct_skeletons_by_V4"], {"0": 16, "1": 38, "2": 64, "3": 38, "4": 10})
        self.assertEqual(fixture["enumeration"]["owner_canonical_forms"], 101)
        self.assertEqual(fixture["enumeration"]["unmatched_reduced_skeletons_by_V4_and_t"], {})
        owners = fixture["owners"]
        self.assertEqual(len(owners), 67)
        self.assertEqual(sorted(m for m, o in owners.items() if not o["entry_capable"]), NON_ENTRY)
        self.assertEqual(fixture["inputs"], {"momenta_sha256": PLAN.sha256_file(FIVE_LOOP["momenta"]),
                                             "parent_witnesses_sha256": PLAN.sha256_file(FIVE_LOOP["witnesses"])})

    def test_fast_plan_from_fixture(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            code, text = run_planner(FIVE_LOOP, root / "out", "--gauge", "feynman", "--difference-set", "9,10",
                                     "--classification", str(FIVE_LOOP["classification"]), loops=5)
            self.assertEqual(code, 0, text)
            receipt = load(root / "out" / "entry-plan-receipt.json")
            summary = receipt["summary"]
            self.assertEqual(summary["class_counts"], {"connected": 41, "factorized": 18, "non_entry": 8})
            self.assertEqual(summary["V4min_histogram_by_class"],
                             {"connected": {"0": 16, "1": 5, "2": 13, "3": 4, "4": 3},
                              "factorized": {"1": 7, "2": 4, "3": 4, "4": 3}})
            self.assertEqual((summary["query_count"], summary["helper_count"], summary["root_count"]), (183, 67, 116))
            self.assertEqual(sorted(o["owner"] for o in receipt["owners"] if o["class"] == "non_entry"), NON_ENTRY)
            rows = load(root / "out" / "queries.json")["queries"]
            by_id = {q["id"]: q for q in rows}
            # twelve-line connected owner, V4min 0: A <= 16 - 12 dots on active axes, R <= 6 on inactive axes
            self.assertEqual(by_id["phys-d10-a16-r6-111111111101001"]["upper"],
                             [4 if b == "1" else 6 for b in "111111111101001"])
            self.assertEqual(by_id["phys-d9-a14-r5-111111111101001"]["power_bounds"],
                             {"max_positive_power": 14, "min_power_difference": 9, "max_power_difference": 9})
            # non-entry owner: widest connected box (V4 = 0)
            self.assertEqual(by_id["conv-d10-a16-r6-000011001001011"]["upper"], [10 if b == "1" else 6 for b in "000011001001011"])
            # factorized owner with V4min 1: nested full-jet root A <= 23, R <= 14, D >= 9 and helper rank 14
            self.assertEqual([q["id"] for q in rows if q["owner"] == "011111001101001"],
                             ["owner-anchor-r14-anone-011111001101001", "nested-d9p-a23-r14-011111001101001"])
            # the 1,324-tuple control owner (connected, V4min 3): A <= 13 / R <= 3 at D = 10, A <= 11 / R <= 2 at D = 9
            self.assertEqual([q["id"] for q in rows if q["owner"] == "011101110111000"],
                             ["owner-anchor-r3-anone-011101110111000", "phys-d10-a13-r3-011101110111000",
                              "phys-d9-a11-r2-011101110111000"])
            self.assertEqual(rows[0]["id"], "owner-anchor-r6-anone-000011001001011")  # selection owner order
            self.assertEqual(sum(1 for q in rows if q["id"].startswith("nested-d9p-")), 18)
            self.assertEqual(receipt["classification_source"]["kind"], "reused")
            self.assertEqual(summary["total_target_count"], "271990954170")

    @unittest.skipUnless(os.environ.get("RUSTRED_SLOW_TESTS") == "1", "full skeleton enumeration takes about 90 s")
    def test_full_enumeration_equals_fixture(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            code, text = run_planner(FIVE_LOOP, root / "out", "--classification-fixture", str(FIVE_LOOP["classification"]),
                                     loops=5)
            self.assertEqual(code, 0, text)
            self.assertEqual(load(root / "out" / "entry-plan-receipt.json")["classification_source"]["kind"],
                             "computed_equal_to_fixture")


class SummaryTests(unittest.TestCase):
    def result(self, records, **top):
        document = {"schema": "rustred.owner-domain-match.json.v2", "status": "locally_applicable",
                    "classification_complete": True, "all_queries_locally_applicable": True, "error": None,
                    "error_kind": None, "error_query_id": None, "query_count": len(records),
                    "completed_queries": len(records), "processed_queries": len(records), "retained_pieces": 0,
                    "counts": {k: 0 for k in SUMMARY.KINDS}, "queries": records, "family_closure_claim": False}
        document.update(top)
        return document

    def record(self, query_id, owner, kinds, complete=True, **extra):
        pieces = [{"lower": [0], "upper": [None], "max_numerator_rank": 1,
                   "power_bounds": {"max_positive_power": None, "min_power_difference": None, "max_power_difference": None},
                   "disposition": {"kind": kind}} for kind in kinds]
        row = {"id": query_id, "owner": owner, "classification_complete": complete, "error": None, "error_kind": None,
               "summary_limit": False, "stats": {"rules": 1}, "pieces": pieces}
        row.update(extra)
        return row

    def test_counts_and_helper_flagging(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            records = [self.record("owner-anchor-r6-anone-110", "110", ["selected_rule", "unresolved"], complete=False),
                       self.record("phys-d10-a16-r6-110", "110", ["selected_rule", "terminal"]),
                       self.record("owner-anchor-r5-anone-101", "101", ["exact_zero_sector"]),
                       self.record("phys-d9-a14-r5-101", "101", ["exact_gap", "invalid_source_condition"])]
            counts = {"selected_rule": 2, "terminal": 1, "exact_zero_sector": 1, "exact_gap": 1, "unresolved": 1,
                      "invalid_source_condition": 1}
            result = dump(root / "result.json", self.result(records, counts=counts, classification_complete=False))
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                code = SUMMARY.main(["--result", str(result), "--output", str(root / "summary.json")])
            self.assertEqual(code, 0)
            summary = load(root / "summary.json")
            self.assertEqual(summary["piece_counts"], counts)
            self.assertTrue(summary["counts_agree_with_report"])
            self.assertEqual(summary["helper_positive_power_owners"], ["110"])
            self.assertEqual(summary["queries_with_gaps"], ["phys-d9-a14-r5-101"])
            self.assertEqual(summary["queries_with_unresolved_pieces"], ["owner-anchor-r6-anone-110"])
            self.assertEqual(summary["incomplete_queries"], ["owner-anchor-r6-anone-110"])
            self.assertEqual([q["piece_count"] for q in summary["queries"]], [2, 2, 1, 2])
            self.assertEqual(summary["result_sha256"], PLAN.sha256_file(result))
            self.assertIs(summary["family_closure_claim"], False)
            with contextlib.redirect_stderr(io.StringIO()), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(SUMMARY.main(["--result", str(result), "--output", str(root / "summary.json")]), 2)
                self.assertEqual(SUMMARY.main(["--result", str(result), "--output", str(root / "summary.json"), "--force"]), 0)

    def test_streaming_across_chunk_boundaries(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            padding = "x" * (3 * 1024 * 1024 + 17)
            records = [self.record(f"owner-anchor-r{i}-anone-1{i % 2}", f"1{i % 2}", ["selected_rule"], error=None, note=padding)
                       for i in range(3)]
            records[1]["id"] = "phys-d3-a4-r1-11"
            result = dump(root / "result.json", self.result(records, counts={"selected_rule": 3, "terminal": 0,
                          "exact_zero_sector": 0, "exact_gap": 0, "unresolved": 0, "invalid_source_condition": 0}))
            summary = SUMMARY.summarize(result)
            self.assertEqual(summary["streamed_query_count"], 3)
            self.assertEqual(summary["piece_counts"]["selected_rule"], 3)
            self.assertEqual(summary["helper_positive_power_owners"], [])

    def test_rejects_wrong_schema_and_unknown_kind(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bad = dump(root / "bad.json", self.result([], schema="other"))
            with self.assertRaisesRegex(ValueError, "schema"):
                SUMMARY.summarize(bad)
            odd = dump(root / "odd.json", self.result([self.record("q", "1", ["mystery"])]))
            with self.assertRaisesRegex(ValueError, "unknown disposition kind"):
                SUMMARY.summarize(odd)
            (root / "trailing.json").write_text(json.dumps(self.result([])) + " {}")
            with self.assertRaisesRegex(ValueError, "trailing"):
                SUMMARY.summarize(root / "trailing.json")


if __name__ == "__main__":
    unittest.main()
