"""Differential controls against the existing exhaustive labelled censuses."""
from collections import Counter
import copy
import json
from pathlib import Path
import tempfile
import unittest

from audit import audit_contractions
from unique_contractions import enumerate_classes

DIRECTORY = Path(__file__).resolve().parent


class UniqueContractionTests(unittest.TestCase):
    def test_matches_every_class_in_four_five_and_six_loop_exhaustive_censuses(self):
        for directory, expected_count in [(DIRECTORY/'controls/four_loop', 16),
                                          (DIRECTORY/'controls/five_loop', 67),
                                          (DIRECTORY, 435)]:
            with self.subTest(directory=directory):
                manifest = json.loads((directory/'manifest.json').read_text())
                expected = json.loads((directory/'contraction_census.json').read_text())
                result = enumerate_classes(manifest)
                self.assertEqual(result['total_classes'], expected_count)
                self.assertEqual(result['classes_by_kind'], expected['classes_by_kind'])
                self.assertEqual(result['classes_by_edge_count'], expected['classes_by_edge_count'])
                self.assertEqual({r['canonical_matroid'] for r in result['representatives']},
                                 {r['canonical_matroid'] for r in expected['representatives']})

    def test_physical_coordinates_need_not_be_contiguous_or_in_input_order(self):
        directory = DIRECTORY/'controls/four_loop'
        manifest = json.loads((directory/'manifest.json').read_text())
        expected = enumerate_classes(manifest)
        changed = copy.deepcopy(manifest)
        for parent in changed['entries']:
            size = len(parent['momenta'])
            parent['momenta'].reverse()
            parent['physical_propagators'] = [size-1-i for i in reversed(parent['physical_propagators'])]
        result = enumerate_classes(changed)
        self.assertEqual({r['canonical_matroid'] for r in result['representatives']},
                         {r['canonical_matroid'] for r in expected['representatives']})

    def test_independent_adjacency_audit_detects_omitted_descendant(self):
        directory = DIRECTORY/'controls/four_loop'
        manifest = json.loads((directory/'manifest.json').read_text())
        result = enumerate_classes(manifest)
        graphs = {g['id']: g for g in json.loads((directory/'cubic_graphs.json').read_text())['graphs']}
        representatives = {g['id']: g for g in json.loads((directory/'classified_census.json').read_text())['representatives']}
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)/'contraction_census.json'
            path.write_text(json.dumps(result))
            audit_contractions(path.parent, 4, graphs, representatives)
            result['representatives'].pop()
            values = result['representatives']
            result['total_classes'] = len(values)
            result['counts']['expanded_classes'] = len(values)
            result['classes_by_kind'] = dict(Counter(r['classification'] for r in values))
            result['classes_by_edge_count'] = {kind: dict(Counter(str(r['physical_edge_count'])
                for r in values if r['classification'] == kind)) for kind in ('nonfactorized', 'factorized')}
            path.write_text(json.dumps(result))
            with self.assertRaisesRegex(ValueError, 'omits a next-edge contraction'):
                audit_contractions(path.parent, 4, graphs, representatives)


if __name__ == '__main__':
    unittest.main()
