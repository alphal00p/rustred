"""Frontier rescue steering (supervisor auto-rescue and production launcher).

Steering-only tests: a fake native executable plays the walk (a frontier stop,
then a resumed completion) and the planner (`walk-rescue-plan`); no license,
native build or algebra. The real engine drill is in the W1 rescue note.
"""
import importlib.util
from contextlib import redirect_stderr, redirect_stdout
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SOURCE = Path(__file__).with_name("shared_owner_campaign.py")
SPEC = importlib.util.spec_from_file_location("campaign_rescue", SOURCE)
CAMPAIGN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAMPAIGN)
PRODUCTION_SPEC = importlib.util.spec_from_file_location(
    "production_rescue", SOURCE.with_name("production_saved_owner_campaign.py"))
PRODUCTION = importlib.util.module_from_spec(PRODUCTION_SPEC)
PRODUCTION_SPEC.loader.exec_module(PRODUCTION)

# The fake native: owner-domain-match stops on a frontier unless it was
# resumed with an amendment (then it completes); walk-rescue-plan writes the
# plan named by FAKE_VERDICT and, for "rescue", the next amendment. Every
# invocation's argv is appended to calls.jsonl beside the executable.
FAKE = r'''
import json, os, sys
from pathlib import Path
here = Path(sys.argv[0]).resolve().parent
with (here / "calls.jsonl").open("a") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")
arg = lambda name: sys.argv[sys.argv.index(name) + 1] if name in sys.argv else None
if sys.argv[1] == "walk-rescue-plan":
    verdict = os.environ.get("FAKE_VERDICT", "rescue")
    plan = {"verdict": verdict, "reason": "fake " + verdict, "frontier_nodes": 1, "classes": {"x": 1},
            "physics_blocked": ["p"] if verdict == "rescue" else []}
    request = json.loads(Path(arg("--command")).read_text())["command"]
    sequence = request.count("--amend-queries") + 1
    if verdict == "rescue":
        text = json.dumps({"schema": "rustred.owner-domain-walk-amendment.json.v1", "sequence": sequence,
                           "parent": "0" * 64, "queries": []}) + "\n"
        Path(arg("--amendment-output")).write_text(text)
        plan["amendment"] = {"sequence": sequence, "digest": "d" * 64, "parent": "0" * 64, "queries": 1,
                             "helpers": [{"id": "h"}]}
    Path(arg("--output")).write_text(json.dumps(plan))
    raise SystemExit(0 if verdict in ("rescue", "no_amendment_needed") else 1)
output = Path(arg("--output"))
checkpoint = arg("--checkpoint") or arg("--resume")
Path(checkpoint).mkdir(exist_ok=True)
if os.environ.get("FAKE_DRAIN") and sys.argv.count("--amend-queries") == 0:
    output.write_text(json.dumps({"status": "incomplete", "recursive_worklist_exhausted": True,
                                  "frontiers": 3, "error": None}))
    Path(arg("--events")).write_text(json.dumps({"event": "finished", "status": "incomplete"}) + "\n")
    raise SystemExit(4)
amendments = sys.argv.count("--amend-queries")
stops = int(os.environ.get("FAKE_STOPS", "1"))
if amendments < stops and "--no-amendment-counts" not in sys.argv:
    resumes = sum(1 for line in (here / "calls.jsonl").read_text().splitlines()
                  if "--resume" in json.loads(line) and json.loads(line)[0] == "owner-domain-match")
    if os.environ.get("FAKE_VERDICT") != "no_amendment_needed" or resumes < stops:
        output.write_text(json.dumps({"status": "paused", "stop_reason": "frontier_policy"}))
        Path(arg("--events")).write_text(json.dumps({"event": "checkpoint_saved", "checkpoint":
            {"state": "saved", "generation": 2, "directory": checkpoint}}) + "\n")
        raise SystemExit(4)
output.write_text(json.dumps({"status": "locally_resolved", "amendments": amendments}))
Path(arg("--events")).write_text(json.dumps({"event": "finished", "status": "locally_resolved"}) + "\n")
raise SystemExit(0)
'''


def fake_native(directory):
    child = directory / "fake-rustred"
    child.write_text(f"#!{sys.executable}\n" + FAKE)
    child.chmod(0o700)
    return child


def supervise(directory, *extra, env=None):
    child = fake_native(directory)
    manifest = directory / "selection.json"; manifest.write_text("{}")
    queries = directory / "queries.json"
    queries.write_text(json.dumps({"queries":[{"id":"p"}],"query_roles":{"required":["p"],"auxiliary":[]}}))
    cpu = min(os.sched_getaffinity(0))
    command = [sys.executable, str(SOURCE), "--executable", str(child), "--manifest", str(manifest),
               "--queries", str(queries), "--workers", "1", "--cpus", str(cpu), "--sample-seconds", "0.1",
               "--run-directory", str(directory / "run"), "--no-progress", *extra]
    environment = dict(os.environ, **(env or {}))
    environment.pop("RUSTRED_WALK_DIAGNOSTIC_PAUSE", None)
    return subprocess.run(command, capture_output=True, text=True, timeout=60, env=environment)


def calls(directory):
    return [json.loads(line) for line in (directory / "calls.jsonl").read_text().splitlines()]


class SupervisorRescueTests(unittest.TestCase):
    def test_known_class_writes_the_amendment_and_resumes_to_completion(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            checkpoint = directory / "checkpoint"
            result = supervise(directory, "--checkpoint", str(checkpoint), "--frontier-policy", "stop",
                               "--auto-rescue", "--helper-id-prefix", "owner-anchor-",
                               env={"FAKE_STOPS": "2"})
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
            self.assertIn("Frontier stop rescued automatically", result.stdout)
            amendments = directory / "checkpoint.amendments"
            files = sorted(amendments.glob("amendment-*.json"))
            self.assertEqual([p.name for p in files], ["amendment-0001.json", "amendment-0002.json"])
            for path in files:
                self.assertEqual(path.stat().st_mode & 0o222, 0, "amendments are read-only")
            rows = [json.loads(line) for line in (amendments / "rescues.jsonl").read_text().splitlines()]
            self.assertEqual([row["action"] for row in rows], ["resume", "resume"])
            self.assertEqual([row["attempt"] for row in rows], [1, 2])
            self.assertEqual(rows[1]["amendment"]["sequence"], 2)
            run = directory / "run"
            receipt = json.loads((run / "rescue.json").read_text())
            self.assertEqual(receipt["plan"]["verdict"], "rescue")
            self.assertIn("--amend-queries", receipt["resume_command"])
            self.assertFalse(receipt["family_closure_claim"])
            native = [c for c in calls(directory) if c[0] == "owner-domain-match"]
            plans = [c for c in calls(directory) if c[0] == "walk-rescue-plan"]
            self.assertEqual(len(native), 3)
            self.assertEqual(len(plans), 2)
            self.assertIn("--checkpoint", native[0])
            self.assertNotIn("--amend-queries", native[0])
            # Each resume re-supplies the whole chain, in order.
            self.assertEqual([native[2][i + 1] for i, a in enumerate(native[2]) if a == "--amend-queries"],
                             [str(p) for p in files])
            self.assertEqual(native[1].count("--amend-queries"), 1)
            self.assertIn("--helper-id-prefix", plans[0])
            self.assertEqual(plans[0][plans[0].index("--helper-id-prefix") + 1], "owner-anchor-")
            # Flat resume names (the chain would otherwise outgrow NAME_MAX).
            resumed = sorted(directory.glob("run.resume-*"), key=lambda p: (p / "request.json").stat().st_mtime_ns)
            self.assertEqual(len(resumed), 2)
            self.assertTrue(all(p.name.count(".resume-") == 1 for p in resumed))
            final = resumed[1:]
            request = json.loads((final[0] / "request.json").read_text())
            self.assertEqual(len(request["amend_queries"]), 2)
            self.assertEqual(request["auto_rescue"]["helper_id_prefix"], "owner-anchor-")
            self.assertEqual(json.loads((final[0] / "result.json").read_text())["amendments"], 2)

    def test_unknown_class_waits_for_the_owner(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result = supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                               "--frontier-policy", "stop", "--auto-rescue",
                               env={"FAKE_VERDICT": "unknown_frontier_class"})
            self.assertEqual(result.returncode, 4, result.stderr)
            self.assertIn("WAITS FOR THE OWNER", result.stderr)
            receipt = json.loads((directory / "run" / "rescue.json").read_text())
            self.assertEqual(receipt["action"], "wait_for_owner")
            self.assertIn("unknown_frontier_class", receipt["reason"])
            self.assertFalse(list((directory / "checkpoint.amendments").glob("amendment-*.json")))
            self.assertFalse(list(directory.glob("run.resume-*")))

    def test_no_amendment_needed_resumes_with_the_existing_chain(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result = supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                               "--frontier-policy", "stop", "--auto-rescue",
                               env={"FAKE_VERDICT": "no_amendment_needed"})
            self.assertEqual(result.returncode, 0, result.stderr)
            rows = (directory / "checkpoint.amendments" / "rescues.jsonl").read_text().splitlines()
            self.assertEqual(json.loads(rows[0])["amendment"], None)
            native = [c for c in calls(directory) if c[0] == "owner-domain-match"]
            self.assertEqual(len(native), 2)
            self.assertIn("--resume", native[1])
            self.assertNotIn("--amend-queries", native[1])

    def test_drained_walk_with_frontiers_is_planned_and_completes_or_rescues(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result = supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                               "--frontier-policy", "stop", "--auto-rescue",
                               env={"FAKE_DRAIN": "1", "FAKE_VERDICT": "no_amendment_needed"})
            self.assertEqual(result.returncode, 4, result.stderr)
            self.assertIn("every physics query has an untainted containing root", result.stdout)
            receipt = json.loads((directory / "run" / "rescue.json").read_text())
            self.assertEqual((receipt["trigger"], receipt["action"]), ("drained_with_frontiers", "complete"))
            self.assertFalse(list(directory.glob("run.resume-*")))
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result = supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                               "--frontier-policy", "stop", "--auto-rescue", env={"FAKE_DRAIN": "1"})
            self.assertEqual(result.returncode, 0, result.stderr)
            rows = [json.loads(line) for line in
                    (directory / "checkpoint.amendments" / "rescues.jsonl").read_text().splitlines()]
            self.assertEqual([(row["trigger"], row["action"]) for row in rows],
                             [("drained_with_frontiers", "resume")])

    def test_attempt_bound_stops_the_automatic_rescue(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            amendments = directory / "checkpoint.amendments"
            amendments.mkdir()
            (amendments / "rescues.jsonl").write_text(json.dumps({"action": "resume"}) + "\n")
            result = supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                               "--frontier-policy", "stop", "--auto-rescue", "--max-rescues", "1")
            self.assertEqual(result.returncode, 4, result.stderr)
            self.assertIn("attempts exhausted", result.stderr)
            self.assertFalse([c for c in calls(directory) if c[0] == "walk-rescue-plan"])

    def test_record_policy_or_missing_resume_are_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            result = supervise(directory, "--checkpoint", str(directory / "checkpoint"), "--auto-rescue")
            self.assertEqual(result.returncode, 2)
            self.assertIn("--auto-rescue requires", result.stderr)
            amendment = directory / "a.json"; amendment.write_text("{}")
            result = supervise(directory, "--checkpoint", str(directory / "checkpoint"),
                               "--amend-queries", str(amendment))
            self.assertEqual(result.returncode, 2)
            self.assertIn("--amend-queries requires", result.stderr)

    def test_planner_helpers_and_receipt_logic(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            output = directory / "run"; output.mkdir()
            (output / "result.json").write_text(json.dumps({"status": "paused", "stop_reason": "frontier_policy"}))
            self.assertTrue(CAMPAIGN.native_frontier_stop(output, 4))
            self.assertFalse(CAMPAIGN.native_frontier_stop(output, 0))
            (output / "result.json").write_text(json.dumps({"status": "paused", "stop_reason": "paused"}))
            self.assertFalse(CAMPAIGN.native_frontier_stop(output, 4))
            amendments = directory / "amendments"
            self.assertEqual(CAMPAIGN.rescue_attempts(amendments), 0)
            (output / "request.json").write_text(json.dumps({"command": ["x", "--resume", "c"]}))

            def runner(command, **_):
                arg = lambda name: Path(command[command.index(name) + 1])
                arg("--amendment-output").write_text("amendment-bytes\n")
                arg("--output").write_text(json.dumps({"verdict": "rescue", "reason": "r", "amendment": {
                    "sequence": 1, "digest": "a" * 64, "parent": "b" * 64, "queries": 2}}))
                return subprocess.CompletedProcess(command, 0, "", "")

            receipt = CAMPAIGN.plan_rescue(Path("rustred"), output, amendments, "anchor", None, {}, 4, 0,
                                           runner=runner)
            self.assertEqual(receipt["action"], "resume")
            path = Path(receipt["amendment"]["path"])
            self.assertEqual(path.name, "amendment-0001.json")
            self.assertEqual(path.read_text(), "amendment-bytes\n")
            # Planning the same stop again reuses the identical file.
            again = CAMPAIGN.plan_rescue(Path("rustred"), output, amendments, "anchor", None, {}, 4, 0,
                                         runner=runner)
            self.assertEqual(again["amendment"]["path"], str(path))

            def other(command, **_):
                runner(command)
                Path(command[command.index("--amendment-output") + 1]).write_text("other\n")
                return subprocess.CompletedProcess(command, 0, "", "")

            refused = CAMPAIGN.plan_rescue(Path("rustred"), output, amendments, "anchor", None, {}, 4, 0,
                                           runner=other)
            self.assertEqual(refused["action"], "wait_for_owner")
            self.assertIn("refusing to rewrite", refused["reason"])
            CAMPAIGN.record_rescue(amendments, output, receipt)
            self.assertEqual(CAMPAIGN.rescue_attempts(amendments), 1)
            self.assertEqual(json.loads((output / "rescue.json").read_text())["action"], "resume")

            def broken(command, **_):
                return subprocess.CompletedProcess(command, 2, "", "boom")

            (output / "rescue-plan.json").unlink()
            failed = CAMPAIGN.plan_rescue(Path("rustred"), output, amendments, "anchor", None, {}, 4, 0,
                                          runner=broken)
            self.assertEqual(failed["action"], "wait_for_owner")
            self.assertIn("boom", failed["reason"])


def production_fixture(directory):
    inputs = directory / "inputs"; inputs.mkdir()
    (inputs / "selection.json").write_text("{}")
    (inputs / "queries.json").write_text(json.dumps({
        "schema": "rustred.owner-domain-queries.json.v2", "queries": [{"id":"p"}],
        "query_roles":{"required":["p"],"auxiliary":[]}}))
    (inputs / "input-receipt.json").write_text(json.dumps({
        "selection_sha256": PRODUCTION.digest(inputs / "selection.json"),
        "queries_sha256": PRODUCTION.digest(inputs / "queries.json"), "owners": []}))
    executable = directory / "fake-rustred"
    executable.write_text(f"#!{sys.executable}\nraise SystemExit('must not execute')\n")
    executable.chmod(0o700)
    return executable


def production_plan(directory, *options):
    output = io.StringIO()
    with redirect_stdout(output):
        result = PRODUCTION.main(["--campaign-directory", str(directory), "--json", *options])
    assert result == 0
    return json.loads(output.getvalue())


class LauncherRescueTests(unittest.TestCase):
    def test_new_steering_freezes_auto_rescue_and_resume_supplies_the_chain(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            plan = production_plan(directory, "--executable", str(production_fixture(directory)), "--workers", "1")
            policy = plan["steering_policy"]
            self.assertEqual(policy["schema"], "rustred.production-steering.v5")
            self.assertEqual(policy["options"]["auto_rescue"], True)
            self.assertEqual(policy["options"]["helper_id_prefix"], "owner-anchor-")
            command = policy["command_arguments"]
            self.assertIn("--auto-rescue", command)
            self.assertEqual(command[command.index("--amendments-directory") + 1], str(directory / "amendments"))
            self.assertEqual(plan["amendments"], [])
            amendments = directory / "amendments"; amendments.mkdir()
            for name in ("amendment-0002.json", "amendment-0001.json", "rescues.jsonl"):
                (amendments / name).write_text("{}")
            resumed = production_plan(directory, "--resume")
            flags = [resumed["command"][i + 1] for i, a in enumerate(resumed["command"]) if a == "--amend-queries"]
            self.assertEqual(flags, [str(amendments / "amendment-0001.json"), str(amendments / "amendment-0002.json")])
            with redirect_stderr(io.StringIO()) as errors, self.assertRaises(SystemExit):
                production_plan(directory, "--resume", "--no-auto-rescue")
            self.assertIn("differs from frozen policy", errors.getvalue())

    def test_opt_out_and_record_campaigns_have_no_auto_rescue(self):
        for options in (("--no-auto-rescue",), ("--frontier-policy", "record")):
            with self.subTest(options=options), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                plan = production_plan(directory, "--executable", str(production_fixture(directory)),
                                       "--workers", "1", *options)
                self.assertFalse(plan["steering_policy"]["options"]["auto_rescue"])
                self.assertNotIn("--auto-rescue", plan["steering_policy"]["command_arguments"])
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with redirect_stderr(io.StringIO()) as errors, self.assertRaises(SystemExit):
                production_plan(directory, "--executable", str(production_fixture(directory)), "--workers", "1",
                                "--frontier-policy", "record", "--auto-rescue")
            self.assertIn("requires --frontier-policy stop", errors.getvalue())


if __name__ == "__main__":
    unittest.main()
