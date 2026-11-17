#!/usr/bin/env python3
"""Tests of the host step's grading (grade_host_step.py; lane procfix7 rounds 8b / 9) ON THE RECORDED HOST STEP 1
(fixtures/host-step-1: its ledger, run.log, scopes.peak, its CPU-ledger lines, the 270 scope units the user journal shows).
Run: python3 -I -B test_grade_host_step.py (exit 0 = all pass). Standard library only; reads the fixture (it may be
READ-ONLY, as in a frozen directory: round 9, gate13 m13-frozen-test), writes only temporary copies with writable modes.

Expected for the run as recorded (reports/HOST-STEP-1-ANALYSIS.md section 4): P1 PASS, P2 FAIL (245 verified scope notes
for 270 started: the 25 notes of that run say marker_only), P3 PASS, P4 PASS (the coordinator's verify), P5 FAIL (3
unproven_end notes; its 554 signals are all accounted for), P6 PASS, P7 PASS, P8 PASS; with the round-9 notes (the 25
recorded as scope_acknowledged_ended from the kernel's record at the exit, the 39 failed scopes recorded and reset, no
unproven_end) every criterion passes. Round 9 (gate13 m13-p5-lenient): a signal record that names a job of this
campaign but not that job's leader fails P5 and the watcher's ABORT test."""

import json
import os
import shutil
import stat
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import grade_host_step as G  # noqa: E402

FIX = os.path.join(HERE, "fixtures", "host-step-1")
CID = "host-fake-1.3cc7bd03c55c83b7"
ROOT = "/sys/fs/cgroup/user.slice/user-1002.slice/user@1002.service/app.slice"


def out_dir(rewrite=None):
    """A WRITABLE copy of the fixture (contents only: the fixture's read-only modes are not copied)."""
    d = tempfile.mkdtemp(prefix="rr-grade-")
    for base, _dirs, files in os.walk(os.path.join(FIX, "c")):
        rel = os.path.relpath(base, FIX)
        os.makedirs(os.path.join(d, rel), exist_ok=True)
        for f in files:
            shutil.copyfile(os.path.join(base, f), os.path.join(d, rel, f))
    for f in ("run.log", "scopes.peak", "units.seen"):
        shutil.copyfile(os.path.join(FIX, f), os.path.join(d, f))
    for f in ("stopped.txt", "units.before", "units.after"):
        open(os.path.join(d, f), "w").close()
    for base, dirs, files in os.walk(d):
        for x in dirs:
            os.chmod(os.path.join(base, x), stat.S_IRWXU)
        for x in files:
            os.chmod(os.path.join(base, x), stat.S_IRUSR | stat.S_IWUSR)
    if rewrite:
        seg = os.path.join(d, "c", "ledger", "segment-000001.jsonl")
        lines = rewrite([json.loads(l) for l in open(seg)])
        with open(seg, "w") as f:
            f.write("".join(json.dumps(r) + "\n" for r in lines))
    return d


def run(d):
    return {k: (v, why) for k, v, why in G.grade(d, CID, 0, 0, 0, 0, "--run",
                                                  os.path.join(FIX, "CPU_LEDGER.extract.md"), 0, "/nonexistent")}


def graded(rewrite=None):
    d = out_dir(rewrite)
    try:
        return run(d), G.abort_findings(os.path.join(d, "c"), CID)
    finally:
        shutil.rmtree(d)


def round9(recs):
    """The round-9 records of the same events: the 25 dead-before-check attempts acknowledged from the kernel's record
    at the exit; every scope recorded failed (as the 39 oom-kill ones were) recorded before its reset; the round-7
    walk's unproven_end notes absent."""
    unit, out = {}, []
    for r in recs:
        e = r["event"]
        b = e.get("body") or {}
        if e["kind"] == "job" and b["ev"].get("ev") == "launch":
            unit[(b["job"], b["attempt"])] = b["ev"].get("scope_unit")
        t = b.get("text", "") if e["kind"] == "note" else ""
        if t.startswith("unproven_end "):
            continue
        if t.startswith("job_containment ") and '"containment":"marker_only"' in t:
            j = json.loads(t[len("job_containment "):])
            u = unit[(j["job"], j["attempt"])]
            j["containment"] = "scope_acknowledged_ended"
            j["why"] = (f"scope {u} existed: the kernel's record of the job's leader at its exit names it "
                        f"({ROOT}/{u}.scope, removed since); the job ended before its cgroup was checked")
            b["text"] = "job_containment " + json.dumps(j, separators=(",", ":"))
        out.append(r)
    return out


def signals(recs):
    return [r for r in recs if r["event"]["kind"] == "signal_sent"]


class Recorded(unittest.TestCase):
    def test_the_run_as_recorded(self):
        r, abort = graded()
        want = {"P1": "PASS", "P2": "FAIL", "P3": "PASS", "P4": "PASS", "P5": "FAIL", "P6": "PASS", "P7": "PASS",
                "P8": "PASS"}
        self.assertEqual({k: v for k, (v, _) in r.items()}, want, r)
        self.assertIn("scope:rr- 245 + scope_acknowledged_ended 0 == started 270", r["P2"][1])
        self.assertIn("unproven_end", r["P5"][1])
        self.assertIn("554 signal_sent (0 not accounted for", r["P5"][1])
        self.assertIn("claims of this campaign 270, unreleased []", r["P6"][1])
        self.assertIn("270 seen, 270 recorded scopes", r["P8"][1])
        self.assertEqual(abort, [])

    def test_with_the_round_9_notes_every_criterion_passes(self):
        r, abort = graded(round9)
        self.assertTrue(all(v == "PASS" for v, _ in r.values()), r)
        self.assertIn("scope:rr- 245 + scope_acknowledged_ended 25 == started 270", r["P2"][1])
        self.assertEqual(abort, [])


class P5Strict(unittest.TestCase):
    """gate13 m13-p5-lenient: a job label alone no longer accounts for a signal."""

    def mutate(self, f):
        def rw(recs):
            recs = round9(recs)
            f(recs)
            return recs
        return graded(rw)

    def test_a_group_signal_to_the_leader_of_another_job_fails_and_aborts(self):
        def f(recs):
            s = signals(recs)
            other = next(x for x in s if x["event"]["body"]["job"] != s[0]["event"]["body"]["job"])
            s[0]["event"]["body"]["pid"] = other["event"]["body"]["pid"]  # a real leader, of ANOTHER job
        r, abort = self.mutate(f)
        self.assertEqual(r["P5"][0], "FAIL", r["P5"])
        self.assertIn("not the leader of a started attempt of that job", r["P5"][1])
        self.assertEqual(len(abort), 1, abort)

    def test_a_group_signal_to_an_unknown_pid_with_a_job_label_fails(self):
        def f(recs):
            signals(recs)[5]["event"]["body"]["pid"] = 4194301
        r, abort = self.mutate(f)
        self.assertEqual(r["P5"][0], "FAIL", r["P5"])
        self.assertEqual(len(abort), 1, abort)

    def test_a_group_signal_long_after_its_attempt_fails(self):
        def f(recs):
            # the first attempt's SIGKILL moved to the end of the ledger (after that job's later records)
            s = signals(recs)[1]
            recs.remove(s)
            s["utc"] = "2026-11-17T09:37:00.000Z"
            s["seq"] = 10**6
            recs.append(s)
        r, abort = self.mutate(f)
        self.assertEqual(r["P5"][0], "FAIL", r["P5"])
        self.assertEqual(len(abort), 1, abort)

    def test_process_signals_need_the_start_time_and_this_campaigns_marker(self):
        def f(recs):
            b = dict(signals(recs)[0]["event"]["body"])
            job = b["job"]
            good = dict(b, target="process", proof="attempt marker", start_ticks=123, marker=f"{CID}:{job}:a1")
            foreign = dict(good, marker=f"other.0123456789abcdef:{job}:a1")
            no_start = dict(good, start_ticks=None)
            other_job = dict(good, marker=f"{CID}:N9.i9999.L0-t0:a1")
            for body in (good, foreign, no_start, other_job):
                recs.append({"seq": 10**6, "utc": "2026-11-17T09:30:00.000Z",
                             "event": {"kind": "signal_sent", "body": body}})
        r, abort = self.mutate(f)
        self.assertEqual(r["P5"][0], "FAIL", r["P5"])
        self.assertEqual(len(abort), 3, abort)  # the good one is accounted for

    def test_a_scope_kill_must_name_a_scope_this_campaign_launched(self):
        def f(recs):
            unit = next(r["event"]["body"]["ev"]["scope_unit"] for r in recs if r["event"]["kind"] == "job"
                        and r["event"]["body"]["ev"].get("ev") == "launch")
            for d in (f"{ROOT}/{unit}.scope", f"{ROOT}/rr-other.0123456789abcdef-x-0011223344556677.scope"):
                recs.append({"seq": 10**6, "utc": "2026-11-17T09:30:00.000Z", "event": {"kind": "signal_sent", "body": {
                    "job": None, "pid": 0, "start_ticks": None, "signal": 9, "target": "cgroup", "marker": None,
                    "proof": "job scope", "by": f"recovery scope backstop ({d})"}}})
        r, abort = self.mutate(f)
        self.assertEqual(r["P5"][0], "FAIL", r["P5"])
        self.assertEqual(len(abort), 1, abort)


if __name__ == "__main__":
    unittest.main(verbosity=2)
