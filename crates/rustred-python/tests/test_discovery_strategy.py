"""Pure descriptor tests; importing native extension is deliberately unnecessary."""

import importlib.util
import json
from pathlib import Path
import unittest

_path = Path(__file__).parents[1] / "python/rustred/discovery.py"
_spec = importlib.util.spec_from_file_location("discovery_descriptor", _path)
_module = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_module)
discovery_strategy = _module.discovery_strategy


class DiscoveryStrategyTests(unittest.TestCase):
    def test_default_and_named_native_features(self):
        self.assertEqual(json.loads(discovery_strategy()), {
            "version": 1, "sectors": {"kind": "active-first"},
            "rows": {"kind": "input-order"}})
        for name in ("terms", "coefficient-monomials"):
            value = json.loads(discovery_strategy(rows=name, descending=True))
            self.assertEqual(value["rows"], {"kind": "features", "priorities": [
                {"feature": {"kind": name}, "descending": True}]})

    def test_weighted_runtime_parameters_are_data(self):
        weights = [1, 0, 4]
        value = json.loads(discovery_strategy(rows="absolute-shifts",
            sectors="weighted-support", weights=weights))
        self.assertEqual(value["sectors"]["weights"], weights)
        self.assertEqual(value["rows"]["priorities"][0]["feature"]["weights"], weights)
        self.assertEqual(weights, [1, 0, 4])

    def test_invalid_names_and_ambiguous_parameters_fail(self):
        for kwargs in [{"rows": "arbitrary-pivot"}, {"sectors": "cpu-timing"},
                       {"weights": [1]}, {"descending": 1}]:
            with self.subTest(kwargs=kwargs), self.assertRaises((TypeError, ValueError)):
                discovery_strategy(**kwargs)
        for weights in (None, [], [0], [True], [-1], [1.5], [1_000_001]):
            with self.subTest(weights=weights), self.assertRaises(ValueError):
                discovery_strategy(rows="positive-shifts", weights=weights)


if __name__ == "__main__":
    unittest.main()
