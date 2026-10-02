"""Builder tests only; native admission remains the mathematical authority."""
import importlib.util
import json
from pathlib import Path
import unittest

_path = Path(__file__).parents[1] / "python/rustred/ordering.py"
_spec = importlib.util.spec_from_file_location("order_descriptor", _path)
_module = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_module)
integral_order = _module.integral_order


class IntegralOrderTests(unittest.TestCase):
    def test_default_data_matches_uncut_reference_priorities(self):
        value = json.loads(integral_order(3))
        self.assertEqual(value["version"], 1)
        self.assertEqual(value["support_weights"], [0, 0, 0])
        self.assertEqual(value["degree_rows"], [
            {"active": [1, 1, 1], "inactive": [1, 1, 1]},
            {"active": [0, 0, 0], "inactive": [1, 1, 1]}])
        self.assertEqual(value["active_direction"], "descending")
        self.assertNotIn("pre_support_degree_rows", value)
        self.assertEqual(integral_order(3), integral_order(3, pre_support_degree_rows=[]))

    def test_absolute_degree_prefix_is_distinct_runtime_data(self):
        prefix = {"active": [1, 1], "inactive": [1, 1]}
        value = json.loads(integral_order(2, pre_support_degree_rows=[prefix]))
        self.assertEqual(value["pre_support_degree_rows"], [prefix])
        self.assertEqual(value["degree_rows"][0], prefix)
        # Positive prefix alone bounds the coordinate tie fibres.
        self.assertEqual(json.loads(integral_order(2,
            pre_support_degree_rows=[prefix], degree_rows=[]))["degree_rows"], [])
        self.assertEqual(prefix, {"active": [1, 1], "inactive": [1, 1]})

    def test_all_priorities_are_runtime_data_and_inputs_stay_unchanged(self):
        row = {"active": [4, 2, 1], "inactive": [1, 2, 4]}
        value = json.loads(integral_order(3, support_weights=[5, 1, 0],
            support_priority=[2, 1, 0], degree_rows=[row],
            coordinate_priority=[1, 2, 0], coordinate_groups="interleaved",
            active_direction="ascending"))
        self.assertEqual(value["degree_rows"], [row])
        self.assertEqual(value["coordinate_priority"], [1, 2, 0])
        self.assertEqual(row, {"active": [4, 2, 1], "inactive": [1, 2, 4]})

    def test_python_shape_checks_do_not_replace_native_admissibility(self):
        # The native compiler, not this convenience JSON builder, checks sign
        # coverage, weighted-sum bounds, and its configurable resource limits.
        value = json.loads(integral_order(1,
            degree_rows=[{"active": [0], "inactive": [0]}]))
        self.assertEqual(value["degree_rows"][0]["active"], [0])
        for arity in (0, -1, True, 1.5):
            with self.assertRaises(ValueError): integral_order(arity)
        for options in ({"support_priority": [0, 0]}, {"support_weights": [True, 0]},
                {"support_weights": [-1, 0]}, {"support_weights": [1 << 64, 0]},
                {"degree_rows": []}, {"degree_rows": [{"active": [1, 1]}]},
                {"pre_support_degree_rows": [{"active": [1, 1]}]},
                {"pre_support_degree_rows": [{"active": [-1, 1], "inactive": [1, 1]}]},
                {"coordinate_groups": "unknown"}, {"active_direction": "unknown"}):
            with self.subTest(options=options), self.assertRaises(ValueError):
                integral_order(2, **options)


if __name__ == "__main__":
    unittest.main()
