from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("assert_oracle_pass", HERE / "assert_oracle_pass.py")
GATE = importlib.util.module_from_spec(spec)
spec.loader.exec_module(GATE)


def report(**overrides):
    base = {"schema": "rustred.walk-verify-closure.v2", "verdict": "PASS", "roots_total": 3,
            "roots_independently_verified": 3, "family_closure_claim": False}
    base.update(overrides)
    return base


class GateTests(unittest.TestCase):
    def test_certified_pass(self):
        self.assertEqual(GATE.gate(report()), [])

    def test_every_other_outcome_fails(self):
        for overrides in ({"verdict": "INCOMPLETE"}, {"verdict": "FAIL", "roots_independently_verified": 0},
                          {"roots_independently_verified": 2}, {"roots_total": 0, "roots_independently_verified": 0},
                          {"roots_total": None}, {"schema": "rustred.something-else"},
                          {"family_closure_claim": True}):
            with self.subTest(overrides=overrides):
                self.assertTrue(GATE.gate(report(**overrides)))
        missing = report()
        del missing["roots_independently_verified"]
        self.assertTrue(GATE.gate(missing))

    def test_exit_status(self):
        with tempfile.TemporaryDirectory() as directory:
            good, bad = Path(directory) / "good.json", Path(directory) / "bad.json"
            good.write_text(json.dumps(report()))
            bad.write_text(json.dumps(report(verdict="INCOMPLETE")))
            self.assertEqual(GATE.main([str(good), "--quiet"]), 0)
            self.assertEqual(GATE.main([str(good), str(bad), "--quiet"]), 1)
            self.assertEqual(GATE.main([str(Path(directory) / "missing.json")]), 2)


if __name__ == "__main__":
    unittest.main()
