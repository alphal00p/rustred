"""Offline input and steering checks; no seven-loop IBPs are generated."""
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from seven_loops.topology.prepare import commands
from six_loop_study.generate import manifest_entries


class SevenLoopStudyTests(unittest.TestCase):
    def test_default_preparation_deduplicates_contraction_work(self):
        steps = commands(ROOT / "seven_loops/topology")
        self.assertEqual([label for label, _ in steps], [
            "enumeration", "classification", "exact parent cover",
            "native family export", "full contraction census", "selected-sector export", "independent witness audit",
        ])
        self.assertEqual(steps[0][1][steps[0][1].index("--loops") + 1], "7")
        self.assertIn("unique_contractions.py", steps[-3][1][2])

    def test_labelled_subset_diagnostic_is_explicit_and_reexports(self):
        steps = commands(ROOT / "seven_loops/topology", labelled_subsets=True)
        self.assertEqual([label for label, _ in steps][-3:], [
            "full contraction census", "selected-sector export", "independent witness audit",
        ])
        self.assertTrue(steps[-3][1][2].endswith("/contractions.py"))

    def test_all_parent_inputs_are_portable_complete_vacuum_bases(self):
        path = ROOT / "seven_loops/topology/manifest.json"
        manifest, entries = manifest_entries(path)
        self.assertEqual(manifest["loop_count"], 7)
        self.assertEqual(manifest["coordinate_count"], 28)
        self.assertEqual(len(entries), manifest["parent_count"])
        self.assertEqual(len(entries), 57)
        for entry in entries:
            self.assertEqual(entry["loop_count"], 7)
            self.assertEqual(entry["arity"], 28)
            self.assertEqual(len(entry["physical_propagators"]), 18)
            self.assertEqual(len(entry["auxiliary_slots"]), 10)
            self.assertEqual(len(entry["momenta"]), 28)
            self.assertTrue(all(len(row) == 7 for row in entry["momenta"]))
            self.assertFalse(Path(entry["input"]).is_absolute())
            self.assertFalse(Path(entry["dot"]).is_absolute())
        self.assertFalse(manifest["ibp_generation_performed"])
        self.assertFalse(manifest["closure_claim"])

    def test_every_cubic_quotient_has_an_exact_parent_witness(self):
        directory = ROOT / "seven_loops/topology"
        raw = json.loads((directory / "cubic_graphs.json").read_text())
        census = json.loads((directory / "classified_census.json").read_text())
        cover = json.loads((directory / "maximal_parent_cover.json").read_text())
        self.assertTrue(raw["generation"]["completed"])
        self.assertEqual(raw["graph_count"], 2592)
        self.assertEqual(census["classification_counts"], {"factorized": 2227, "nonfactorized": 365})
        self.assertEqual(census["representative_count"], 359)
        self.assertTrue(cover["complete"])
        self.assertEqual(cover["covered"], cover["required"])
        self.assertEqual(set(cover["witnesses"]), {entry["id"] for entry in census["representatives"]})
        for witness in cover["witnesses"].values():
            self.assertEqual(abs(witness["routing"]["determinant"]), 1)

    def test_selected_generation_jobs_cover_the_full_contraction_census_once(self):
        directory = ROOT / "seven_loops/topology"
        manifest = json.loads((directory / "manifest.json").read_text())
        census = json.loads((directory / "contraction_census.json").read_text())
        self.assertTrue(census["complete"])
        self.assertEqual(census["total_classes"], 4401)
        self.assertEqual(census["classes_by_kind"], {"nonfactorized": 3885, "factorized": 516})
        self.assertEqual(manifest["selected_sector_count"], 4401)
        ids = [record["id"] for entry in manifest["entries"] for record in entry["selected_topology_classes"]]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertEqual(set(ids), {r["id"] for r in census["representatives"]})


if __name__ == "__main__":
    unittest.main()
