#!/usr/bin/env python3
"""Tests of P8 (p8_units.py; lane procfix7 round 8, gate12 M12-2). Run: python3 -I test_p8_units.py (exit 0 = all pass).
The case gate12 measured: a PERFECT run during which the shared uid created foreign units (15 run-p*-i*.scope of light
builds, vk5l-*, t3code-claude-*) must PASS; a unit of this campaign that the ledger does not record, or one still loaded
after the run, must FAIL."""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import p8_units as P  # noqa: E402

CID = "fake1-0123.0123456789abcdef"
OTHER = "N0-other.fedcba9876543210"


def note(job, unit):
    return ('job_containment {"job":"%s","attempt":1,"containment":"scope:%s","why":"verified: the job leader\'s cgroup '
            'is this scope","path":"/sys/fs/cgroup/user.slice/x/%s"}' % (job, unit, unit))


S1 = f"rr-{CID}-N0.i0001.L0-t0-1111111111111111.scope"
S2 = f"rr-{CID}-N0.i0002.L0-t0-2222222222222222.scope"
BEFORE = ["dbus.service", "run-p100-i101.scope", f"rr-{OTHER}-x-3333333333333333.scope"]
FOREIGN_NEW = [f"run-p{n}-i{n + 1}.scope" for n in range(200, 230, 2)] + ["vk5l-lift.service",
               "t3code-claude-abc.scope", f"rr-{OTHER}-y-4444444444444444.scope"]


class P8(unittest.TestCase):
    def test_a_perfect_run_with_foreign_units_of_the_shared_uid_passes(self):
        after = BEFORE + FOREIGN_NEW + ["hoststep-fake1-081500.scope"]
        ok, why, foreign = P.grade_p8(CID, BEFORE, after, [S1, S2], [note("a", S1), note("b", S2)])
        self.assertTrue(ok, why)
        self.assertEqual(len(foreign), len(FOREIGN_NEW), foreign)  # listed, the hoststep scope excluded
        self.assertNotIn("hoststep-fake1-081500.scope", foreign)

    def test_a_scope_of_this_campaign_still_loaded_after_the_run_fails(self):
        after = BEFORE + FOREIGN_NEW + [S2]
        ok, why, _ = P.grade_p8(CID, BEFORE, after, [S1, S2], [note("a", S1), note("b", S2)])
        self.assertFalse(ok)
        self.assertIn(S2, why)

    def test_a_unit_of_this_campaign_the_ledger_does_not_record_fails(self):
        stray = f"rr-{CID}-N0.i0003.L0-t0-5555555555555555.scope"
        ok, why, _ = P.grade_p8(CID, BEFORE, BEFORE, [S1, stray], [note("a", S1)])
        self.assertFalse(ok)
        self.assertIn(stray, why)

    def test_a_requested_but_unverified_scope_counts_as_recorded(self):
        n = ('job_containment {"job":"a","attempt":1,"containment":"marker_only","why":"scope %s requested but not '
             'verified (the leader\'s cgroup does not name it)"}' % S1)
        ok, why, _ = P.grade_p8(CID, BEFORE, BEFORE, [S1], [n])
        self.assertTrue(ok, why)

    def test_the_recorded_formats_of_host_step_1(self):
        # host step 1's notes name an unverified scope WITHOUT ".scope"; the launch record names every requested unit
        u = f"rr-{CID}-N2.i0005.L0-t0_a2-9c35c5094c13f37b"
        n = ('job_containment {"job":"N2.i0005.L0-t0","attempt":2,"containment":"marker_only","why":"scope %s requested '
             'but not verified (the leader\'s cgroup does not name it)"}' % u)
        ok, why, _ = P.grade_p8(CID, BEFORE, BEFORE, [u + ".scope"], [n])
        self.assertTrue(ok, why)
        ok, why, _ = P.grade_p8(CID, BEFORE, BEFORE, [u + ".scope"], [], launched=[u])
        self.assertTrue(ok, why)

    def test_another_campaigns_units_are_foreign_never_ours(self):
        o = f"rr-{OTHER}-z-6666666666666666.scope"
        ok, why, foreign = P.grade_p8(CID, BEFORE, BEFORE + [o], [], [])
        self.assertTrue(ok, why)
        self.assertEqual(foreign, [o])

    def test_no_campaign_id_never_passes(self):
        ok, why, _ = P.grade_p8("", BEFORE, BEFORE, [], [])
        self.assertFalse(ok)
        self.assertIn("no campaign id", why)

    def test_a_prefix_of_this_id_is_not_a_unit_of_this_campaign(self):
        # rr-<cid>X-... is another campaign whose id starts like ours: the separator after the id is part of the match
        o = f"rr-{CID}X-n-7777777777777777.scope"
        ok, why, foreign = P.grade_p8(CID, BEFORE, BEFORE + [o], [], [])
        self.assertTrue(ok, why)
        self.assertEqual(foreign, [o])


class OwnUnits(unittest.TestCase):
    """procfix7 round 9b: the step's own units (the supervisor's service, the pre-flight's scope) are accounted for by
    their exact names: never foreign, never units of this campaign; one still loaded after the run fails P8."""

    OWN = ["hoststep-fake1-081500.service", "hoststep-scopecheck-081500.scope"]

    def test_the_own_units_are_accounted_not_foreign(self):
        after = BEFORE + FOREIGN_NEW
        ok, why, foreign = P.grade_p8(CID, BEFORE, after, [S1, S2], [note("a", S1), note("b", S2)], own_units=self.OWN)
        self.assertTrue(ok, why)
        self.assertIn("accounted, still loaded after the run none", why)
        self.assertFalse(set(self.OWN) & set(foreign))

    def test_an_own_unit_still_loaded_after_the_run_fails(self):
        after = BEFORE + FOREIGN_NEW + [self.OWN[0]]
        ok, why, foreign = P.grade_p8(CID, BEFORE, after, [S1, S2], [note("a", S1), note("b", S2)], own_units=self.OWN)
        self.assertFalse(ok, why)
        self.assertIn(f"still loaded after the run ['{self.OWN[0]}']", why)
        self.assertNotIn(self.OWN[0], foreign)


if __name__ == "__main__":
    unittest.main(verbosity=2)
