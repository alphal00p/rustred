"""Focused native lifecycle/lazy-view tests; no CLI or notebook mocks."""
from __future__ import annotations

import unittest
import os
import subprocess
import sys
import tempfile
from pathlib import Path
import rustred
from test_python_api import UNIT_MASS_PROJECT_K1


class SessionApiTests(unittest.TestCase):
    def test_result_policy_and_explicit_file_read_limits(self):
        session = rustred.start_family_candidates(UNIT_MASS_PROJECT_K1, bundle_max_entries=10_000_000)
        self.assertTrue(session.wait(timeout=60))
        result = session.result()
        view = result.artifact()
        self.assertEqual(view.metadata()["transport_limits"]["bundle_max_entries"],10_000_000)
        self.assertEqual(view.metadata()["decoded_coefficients"],0)
        with tempfile.TemporaryDirectory(prefix="rustred-view-limits-") as directory:
            path=Path(directory)/"candidate.rrbin"
            path.write_bytes(result.bundle)
            reopened=rustred.CandidateArtifact.open_file(path,bundle_max_entries=10_000_000)
            self.assertEqual(reopened.metadata(),view.metadata())
            with self.assertRaises(rustred.RustRedLimitError):
                rustred.CandidateArtifact.open_file(path,bundle_max_bytes=len(result.bundle)-1)
        for value in (0,-1,True,2**64):
            with self.subTest(value=value), self.assertRaises((rustred.RustRedError,TypeError,OverflowError)):
                rustred.CandidateArtifact.open(result.bundle,bundle_max_entries=value)
    @unittest.skipUnless(hasattr(os, "fork"), "POSIX fork contract")
    def test_inherited_session_and_view_reject_before_locking(self):
        code = f'''
import os, signal, rustred
session = rustred.start_family_candidates({UNIT_MASS_PROJECT_K1!r})
assert session.wait(timeout=30)
artifact = session.result().artifact()
pid = os.fork()
if pid == 0:
    signal.alarm(3)
    methods = [lambda: session.done, session.cancel, session.snapshot,
               session.poll_events, session.wait, session.result,
               artifact.metadata, artifact.sectors, lambda: artifact.rules(0),
               lambda: artifact.terminals(0), lambda: artifact.rule(0,0),
               lambda: artifact.coefficient(0)]
    for method in methods:
        try:
            method()
        except rustred.RustRedError as error:
            assert "fork" in str(error)
        else:
            os._exit(3)
    del methods, session, artifact
    os._exit(0)
_, status = os.waitpid(pid, 0)
assert status == 0, status
'''
        subprocess.run([sys.executable, "-c", code], check=True, timeout=45)

    def test_completed_session_and_lazy_details(self):
        session = rustred.start_family_candidates(UNIT_MASS_PROJECT_K1, event_capacity=1)
        self.assertTrue(session.wait(timeout=60))
        batch = session.poll_events()
        self.assertEqual(batch["snapshot"]["state"], "completed")
        self.assertTrue(session.done)
        self.assertIn("RustRed candidate generation", session._repr_html_())
        self.assertGreater(batch["dropped_events"], 0)
        result = session.result()
        self.assertEqual(result.status, "uncertified-candidates")
        artifact = result.artifact()
        meta = artifact.metadata()
        self.assertIn("RustRed candidate artifact", artifact._repr_html_())
        self.assertEqual(meta["decoded_coefficients"], 0)
        self.assertFalse(meta["closure_claim"])
        sectors = artifact.sectors(limit=1)
        self.assertEqual(sectors["items"][0]["ordinal"], 0)
        self.assertGreater(artifact.rules(0)["total"], 0)
        rule = artifact.rule(0, 0)
        self.assertEqual(artifact.metadata()["decoded_coefficients"], 0)
        cid = rule["rhs"][0]["coefficient_id"]
        detail = artifact.coefficient(cid)
        self.assertEqual(detail["id"], cid)
        self.assertEqual(artifact.metadata()["decoded_coefficients"], 1)
        self.assertEqual(artifact.coefficient(cid), detail)
        self.assertEqual(artifact.metadata()["decoded_coefficients"], 1)

    def test_argument_limits_do_not_round_integers(self):
        for value in (0, -1, 2**64, True):
            with self.subTest(value=value), self.assertRaises((rustred.RustRedError, TypeError, OverflowError)):
                rustred.start_family_candidates(UNIT_MASS_PROJECT_K1, event_capacity=value)
        with self.assertRaises((rustred.RustRedError, TypeError)):
            rustred.start_family_candidates(UNIT_MASS_PROJECT_K1, unknown_option=True)

    def test_invalid_source_finishes_failed_not_completed(self):
        session = rustred.candidate_generation_request("not a family").start()
        self.assertTrue(session.wait(timeout=60))
        self.assertEqual(session.snapshot()["state"], "failed")
        with self.assertRaises(rustred.RustRedError):
            session.result()


if __name__ == "__main__":
    unittest.main()
