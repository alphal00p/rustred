"""The performance table must never turn absent oracle evidence into PASS."""
import json
import tempfile
import unittest
from pathlib import Path

import g2prod_gate as gate


class GateTests(unittest.TestCase):
    def record(self, verify=None):
        return {"metrics": {"exit_code": 0, "frontiers": 0},
                "audit": {"audit": "PASS"}, "verify": verify}

    def test_requires_complete_nonempty_integer_reinspection(self):
        self.assertTrue(gate.ok(self.record({"verdict": "PASS",
                                           "roots_independently_verified": 4, "roots_total": 4})))
        for verify in (None, {}, [], {"verdict": "PASS"},
                       {"verdict": "INCOMPLETE", "roots_independently_verified": 4, "roots_total": 4}):
            with self.subTest(verify=verify):
                self.assertFalse(gate.ok(self.record(verify)))
        for verified, total in ((None, None), (True, True), (1, True), (4.0, 4),
                                ("4", "4"), (0, 0), (-1, -1), (3, 4), (5, 4)):
            with self.subTest(verified=verified, total=total):
                self.assertFalse(gate.ok(self.record({"verdict": "PASS",
                                                     "roots_independently_verified": verified,
                                                     "roots_total": total})))

    def test_missing_invalid_or_nonobject_report_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            for name, value in (("metrics", {"exit_code": 0, "frontiers": 0}),
                                ("audit", {"audit": "PASS"}), ("g2stats", {"records": 1})):
                (path / f"{name}.json").write_text(json.dumps(value))
            self.assertFalse(gate.ok(gate.load(path)))
            for text in ("", "{bad json", "[]", "null"):
                (path / "verify.json").write_text(text)
                self.assertFalse(gate.ok(gate.load(path)))

    def test_exit_frontier_and_audit_gates_still_apply(self):
        valid = {"verdict": "PASS", "roots_independently_verified": 1, "roots_total": 1}
        for field, value in (("exit_code", 4), ("frontiers", 1),
                             ("stopped_by_time_limit_at", 1200), ("killed_after_grace", True),
                             ("stop_reason", "operator_signal_15"),
                             ("stop_reason", "host_headroom_below_minimum"),
                             ("censored", True), ("runner_error", "failed")):
            record = self.record(valid)
            record["metrics"][field] = value
            self.assertFalse(gate.ok(record))
        record = self.record(valid)
        record["audit"]["audit"] = "FAIL"
        self.assertFalse(gate.ok(record))

    def test_worker_label_requires_present_matched_positive_counts(self):
        self.assertEqual(gate.matched_workers([{"metrics": {"workers": 16}}] * 4), 16)
        for widths in ((16, 24), (True, True), (None, None), (0, 0)):
            self.assertIsNone(gate.matched_workers([{"metrics": {"workers": width}} for width in widths]))


if __name__ == "__main__":
    unittest.main()
