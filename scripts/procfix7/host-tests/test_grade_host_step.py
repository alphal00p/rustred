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
campaign but not that job's leader fails P5 and the watcher's ABORT test. Round 9b: no tolerance after the attempt's
finished record (gate13b m13b-p5-slack); the unproven_end notes as the named class "P5-unproven" (INFO, next to the
strict P5); P1 names the supervisor's service unit and its end; P8 accounts for the step's own units. Round 14 (host step
3, M5): on the RECORDED run B (fixtures/host-step-3) P5 names the 21 signals delivered but recorded only in the intent log."""

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
        with open(seg) as f:
            lines = rewrite([json.loads(l) for l in f])
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


FIX3 = os.path.join(HERE, "fixtures", "host-step-3")
CID3 = "host-fake-1.e8090c684f269eb5"


def out_dir3(rewrite_ledger=None):
    """procfix7 round 14 (M5): a WRITABLE copy of the RECORDED run B (fixtures/host-step-3: exit 71 in a user-manager
    stall; 146 signals delivered, 125 in the ledger, the intents -126 .. -146 only in the intent log)."""
    d = tempfile.mkdtemp(prefix="rr-grade3-")
    for base, _dirs, files in os.walk(os.path.join(FIX3, "c")):
        rel = os.path.relpath(base, FIX3)
        os.makedirs(os.path.join(d, rel), exist_ok=True)
        for f in files:
            shutil.copyfile(os.path.join(base, f), os.path.join(d, rel, f))
    for f in ("run.log", "scopes.peak", "units.seen"):
        shutil.copyfile(os.path.join(FIX3, f), os.path.join(d, f))
    for f in ("stopped.txt", "units.before", "units.after"):
        open(os.path.join(d, f), "w").close()
    for base, dirs, files in os.walk(d):
        for x in dirs:
            os.chmod(os.path.join(base, x), stat.S_IRWXU)
        for x in files:
            os.chmod(os.path.join(base, x), stat.S_IRUSR | stat.S_IWUSR)
    if rewrite_ledger:
        rewrite_ledger(d)
    return d


def run3(d):
    return {k: (v, why) for k, v, why in G.grade(d, CID3, 71, 0, 0, 3, "--run",
                                                  os.path.join(FIX, "CPU_LEDGER.extract.md"), 0, "/nonexistent")}


class RunB(unittest.TestCase):
    """procfix7 round 14 (host step 3, M5): P5 reads the supervisor's intent log, not only the ledger."""

    def test_the_21_delivered_but_unrecorded_signals_of_run_b_are_a_named_p5_finding(self):
        d = out_dir3()
        try:
            r = run3(d)
        finally:
            shutil.rmtree(d)
        v, why = r["P5"]
        self.assertEqual(v, "FAIL", why)
        self.assertIn("146 delivered, 21 delivered but NOT recorded in the ledger", why)
        self.assertIn("(0 not accounted for by this campaign's records", why)
        self.assertIn("run.log 'not recorded' lines 4", why)
        for k in range(126, 147):
            self.assertIn(str(k), why)

    def test_with_every_delivered_intent_recorded_the_intent_finding_is_gone(self):
        def add_records(d):
            # what M1 makes the supervisor write: a signal_sent for each of the 21 (and no "not recorded" lines)
            seg = os.path.join(d, "c", "ledger", "segment-000001.jsonl")
            with open(seg) as f:
                recs = [json.loads(l) for l in f]
            with open(os.path.join(d, "c", "log", "signal-intents.jsonl")) as f:
                ints = [json.loads(l) for l in f]
            have = {r["event"]["body"].get("by", "").split("[intent ")[-1].rstrip("]")
                    for r in recs if r["event"]["kind"] == "signal_sent"}
            seq = max(r["seq"] for r in recs)
            for v in ints:
                if v["kind"] == "intent" and v["id"] not in have:
                    seq += 1
                    recs.append({"seq": seq, "utc": v["utc"], "event": {"kind": "signal_sent", "body": {
                        "job": v["label"].rsplit(":", 1)[0], "pid": v["pid"], "start_ticks": v.get("start_ticks"),
                        "signal": v["signal"], "target": v["target"], "marker": v.get("marker"),
                        "proof": v["proof"], "by": f"{v['by']} [intent {v['id']}]"}}})
            with open(seg, "w") as f:
                f.write("".join(json.dumps(r) + "\n" for r in recs))
            log = os.path.join(d, "run.log")
            with open(log) as f:
                kept = [l for l in f if "delivered signal(s) not recorded" not in l]
            with open(log, "w") as f:
                f.write("".join(kept))
        d = out_dir3(add_records)
        try:
            r = run3(d)
        finally:
            shutil.rmtree(d)
        v, why = r["P5"]
        self.assertIn("146 signal_sent (0 not accounted for", why)
        self.assertIn("0 delivered but NOT recorded in the ledger, 0 without completion", why)
        self.assertIn("run.log 'not recorded' lines 0", why)
        # P5 still fails on run B's own scope_unverified_limit note: a separate finding
        self.assertEqual(v, "FAIL", why)
        self.assertIn("scope_unverified_limit", why)

    def test_an_intent_without_completion_is_possibly_sent(self):
        def cut_done(d):
            p = os.path.join(d, "c", "log", "signal-intents.jsonl")
            with open(p) as f:
                lines = f.read().splitlines(keepends=True)
            last = max(k for k, l in enumerate(lines) if '"kind":"done"' in l)
            with open(p, "w") as f:
                f.write("".join(lines[:last] + lines[last + 1:]))
        d = out_dir3(cut_done)
        try:
            r = run3(d)
        finally:
            shutil.rmtree(d)
        self.assertIn("1 without completion (POSSIBLY SENT)", r["P5"][1])
        self.assertEqual(r["P5"][0], "FAIL")


class Recorded(unittest.TestCase):
    def test_the_run_as_recorded(self):
        r, abort = graded()
        want = {"P1": "PASS", "P2": "FAIL", "P3": "PASS", "P4": "PASS", "P5": "FAIL", "P5-unproven": "INFO",
                "P6": "PASS", "P7": "PASS", "P8": "PASS"}
        self.assertEqual({k: v for k, (v, _) in r.items()}, want, r)
        self.assertIn("3 unproven_end note(s)", r["P5-unproven"][1])
        self.assertIn("3 of 3 cleared later", r["P5-unproven"][1])
        self.assertIn("the strict P5 FAILs ONLY on these notes", r["P5-unproven"][1])
        self.assertIn("scope:rr- 245 + scope_acknowledged_ended 0 == started 270", r["P2"][1])
        self.assertIn("unproven_end", r["P5"][1])
        self.assertIn("554 signal_sent (0 not accounted for", r["P5"][1])
        self.assertIn("claims of this campaign 270, unreleased []", r["P6"][1])
        self.assertIn("270 seen, 270 recorded scopes", r["P8"][1])
        self.assertEqual(abort, [])

    def test_an_unproven_end_never_cleared_is_named_and_the_strict_p5_still_fails(self):
        def rw(recs):
            out, dropped = [], False
            for r in recs:
                t = (r["event"].get("body") or {}).get("text", "")
                if not dropped and t.startswith("unverified_end_cleared ") and "N0.i0210.L0-t0:a1" in t:
                    dropped = True
                    continue
                out.append(r)
            return out
        r, _ = graded(rw)
        self.assertEqual(r["P5"][0], "FAIL")
        self.assertEqual(r["P5-unproven"][0], "INFO")
        self.assertIn("2 of 3 cleared later", r["P5-unproven"][1])

    def test_p1_names_the_supervisor_service_and_p8_accounts_for_the_steps_own_units(self):
        sup, pre = "hoststep-fake1-120000.service", "hoststep-scopecheck-120000.scope"

        def put(d, name, text):
            with open(os.path.join(d, name), "w") as f:
                f.write(text)

        def extra(d, after):
            put(d, "supervisor.unit", sup + "\n")
            put(d, "preflight.unit", pre + "\n")
            put(d, "launcher.log", f"Running as unit: {sup}; invocation ID: 0123456789abcdef0123456789abcdef\n"
                "Finished with result: success\nMain processes terminated with: code=exited, status=0/SUCCESS\n")
            put(d, "units.after", "".join(u + "\n" for u in after))
        for after, want8 in (([], "PASS"), ([sup], "FAIL")):
            d = out_dir(round9)
            try:
                extra(d, after)
                r = run(d)
            finally:
                shutil.rmtree(d)
            self.assertIn(f"service {sup}: Finished with result: success", r["P1"][1])
            self.assertEqual(r["P8"][0], want8, r["P8"])
            self.assertIn(f"the step's own units {sorted([pre, sup])}: accounted", r["P8"][1])
            self.assertNotIn("1 foreign", r["P8"][1])

    def test_p1_needs_systemds_report_of_a_normal_exit_with_status_0(self):
        """procfix7 round 10 (gate13c m13c-p1-clean-signal): systemd-run --wait returns 0 for a SIGTERM death too."""
        sup = "hoststep-fake1-120000.service"
        for line, want in (("Main processes terminated with: code=exited, status=0/SUCCESS", "PASS"),
                           ("Main processes terminated with: code=killed, status=15/TERM", "FAIL")):
            d = out_dir(round9)
            try:
                with open(os.path.join(d, "supervisor.unit"), "w") as f:
                    f.write(sup + "\n")
                with open(os.path.join(d, "launcher.log"), "w") as f:
                    f.write("Finished with result: success\n" + line + "\n")
                r = run(d)
            finally:
                shutil.rmtree(d)
            self.assertEqual(r["P1"][0], want, r["P1"])

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

    def test_a_group_signal_recorded_after_its_attempts_finish_fails_with_no_tolerance(self):
        def f(recs):
            # the first attempt's SIGKILL moved just after its attempt's finished record (same second)
            s = signals(recs)[1]
            b = s["event"]["body"]
            fin = next(r for r in recs if r["event"]["kind"] == "job" and r["event"]["body"]["job"] == b["job"]
                       and r["event"]["body"]["ev"].get("ev") == "finished")
            recs.remove(s)
            s["seq"], s["utc"] = fin["seq"] + 0.5, fin["utc"]
            recs.insert(recs.index(fin) + 1, s)
        r, abort = self.mutate(f)
        self.assertEqual(r["P5"][0], "FAIL", r["P5"])
        self.assertIn("recorded after its attempt's finished record", r["P5"][1])
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
