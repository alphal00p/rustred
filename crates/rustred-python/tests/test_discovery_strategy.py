"""Pure descriptor tests; importing native extension is deliberately unnecessary."""

import importlib.util
import json
import copy
from pathlib import Path
import unittest

_path = Path(__file__).parents[1] / "python/rustred/discovery.py"
_spec = importlib.util.spec_from_file_location("discovery_descriptor", _path)
_module = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_module)
discovery_strategy = _module.discovery_strategy
rule_portfolio = _module.rule_portfolio


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

    def test_portfolio_is_opt_in_and_preserves_inputs_and_default_bytes(self):
        self.assertEqual(discovery_strategy(),
            '{"rows":{"kind":"input-order"},"sectors":{"kind":"active-first"},"version":1}')
        recipe = rule_portfolio(alternatives=["terms", "coefficient-monomials"],
            quality=["rhs-terms", {"feature": "source-rows", "descending": True}],
            max_depth=0, max_rows=64, max_exact_trace_rows=32, max_exact_trace_terms=512,
            trigger={"kind": "any-at-least", "thresholds": [{"feature": "rhs-terms", "minimum": 3}]})
        before = copy.deepcopy(recipe)
        descriptor = json.loads(discovery_strategy(rows="terms", rule_selection=recipe))
        self.assertEqual(descriptor["version"], 2)
        self.assertEqual(descriptor["rule_selection"], recipe)
        self.assertEqual(recipe, before)
        self.assertEqual(descriptor["rule_selection"]["alternatives"][0]["priorities"][0]["feature"]["kind"], "terms")
        descriptor["rule_selection"]["trigger"]["thresholds"][0]["minimum"] = 5
        self.assertEqual(recipe, before)

    def test_portfolio_rejects_ambiguous_policy_parameters(self):
        base = dict(alternatives=["input-order"], quality=["rhs-terms"],
                    max_depth=0, max_rows=1, max_exact_trace_rows=1, max_exact_trace_terms=1)
        changes = [dict(alternatives=[]), dict(alternatives=["terms"] * 3),
                   dict(quality=[]), dict(quality=["rhs-terms"] * 2),
                   dict(quality=["made-up"]), dict(max_depth=-1), dict(max_depth=True),
                   dict(max_rows=0), dict(max_exact_trace_rows=0), dict(max_exact_trace_terms=1.5),
                   dict(trigger={"kind": "any-at-least", "thresholds": []}),
                   dict(trigger={"kind": "always", "extra": 1}),
                   dict(trigger={"kind": "any-at-least", "thresholds": [
                       {"feature": "rhs-terms", "minimum": True}]}),
                   dict(trigger={"kind": "any-at-least", "thresholds": [
                       {"feature": "rhs-terms", "minimum": 0}] * 2})]
        for change in changes:
            with self.subTest(change=change), self.assertRaises((ValueError, TypeError)):
                rule_portfolio(**(base | change))
        recipe = rule_portfolio(**base)
        for change in ({"version": 2}, {"version": True}, {"kind": "first-valid"}, {"unknown": 0}):
            with self.subTest(change=change), self.assertRaises((ValueError, TypeError)):
                discovery_strategy(rule_selection=recipe | change)

    def test_explicit_alternate_descriptor_is_copied_not_reinterpreted(self):
        alternate = {"kind": "features", "priorities": [
            {"feature": {"kind": "positive-shifts", "weights": [1, 0]}, "descending": True}]}
        recipe = rule_portfolio(alternatives=[alternate], quality=["max-numerator-shift-excursion"],
            max_depth=0, max_rows=10, max_exact_trace_rows=10, max_exact_trace_terms=100)
        self.assertEqual(recipe["alternatives"], [alternate])
        alternate["priorities"][0]["feature"]["weights"][0] = 7
        self.assertEqual(recipe["alternatives"][0]["priorities"][0]["feature"]["weights"], [1, 0])

    def test_total_positive_excursion_is_a_named_runtime_quality_and_trigger(self):
        name = "total-positive-shift-excursion"
        recipe = rule_portfolio(alternatives=["input-order"],
            quality=["max-numerator-shift-excursion", name, "total-numerator-shift-excursion"],
            max_depth=0, max_rows=10, max_exact_trace_rows=10, max_exact_trace_terms=100,
            trigger={"kind": "any-at-least", "thresholds": [{"feature": name, "minimum": 2}]})
        descriptor = json.loads(discovery_strategy(rule_selection=recipe))
        self.assertEqual(descriptor["rule_selection"], recipe)
        self.assertEqual(recipe["quality"][1], {"feature": name, "descending": False})
        with self.assertRaises(ValueError):
            rule_portfolio(alternatives=["input-order"], quality=[name + "-unknown"],
                max_depth=0, max_rows=10, max_exact_trace_rows=10, max_exact_trace_terms=100)


if __name__ == "__main__":
    unittest.main()
