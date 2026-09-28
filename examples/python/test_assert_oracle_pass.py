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
            "roots_independently_verified": 3, "family_closure_claim": False, "mutation": None,
            "reinspection": {"mode": "All", "complete": True}, "reference": {"native_levers": "Off"}}
    base.update(overrides)
    return base


class GateTests(unittest.TestCase):
    def test_certified_pass(self):
        self.assertEqual(GATE.gate(report()), [])

    def test_every_other_outcome_fails(self):
        for overrides in ({"verdict": "INCOMPLETE"}, {"verdict": "FAIL", "roots_independently_verified": 0},
                          {"roots_independently_verified": 2}, {"roots_total": 0, "roots_independently_verified": 0},
                          {"roots_total": None}, {"schema": "rustred.something-else"},
                          {"family_closure_claim": True},
                          {"mutation": {"kind": "alias-chain-detour", "applied": True}},
                          {"reinspection": {"mode": "None", "complete": False}},
                          {"reinspection": {"mode": "Sample", "complete": False}}, {"reinspection": None},
                          {"reference": {"native_levers": "AsRun"}}, {"reference": None}):
            with self.subTest(overrides=overrides):
                self.assertTrue(GATE.gate(report(**overrides)))
        missing = report()
        del missing["roots_independently_verified"]
        self.assertTrue(GATE.gate(missing))
        unmarked = report()
        del unmarked["mutation"]
        self.assertTrue(GATE.gate(unmarked))

    def test_as_run_reference_needs_a_declaration(self):
        as_run = report(reference={"native_levers": "AsRun"})
        self.assertTrue(GATE.gate(as_run))
        self.assertEqual(GATE.gate(as_run, allow_as_run_reference=True), [])
        # The declaration never excuses anything else.
        self.assertTrue(GATE.gate(report(reference={"native_levers": "AsRun"}, mutation={"kind": "x"}),
                                  allow_as_run_reference=True))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "as-run.json"
            path.write_text(json.dumps(as_run))
            self.assertEqual(GATE.main([str(path), "--quiet"]), 1)
            self.assertEqual(GATE.main([str(path), "--quiet", "--allow-as-run-reference"]), 0)

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
