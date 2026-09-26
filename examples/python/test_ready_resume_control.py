"""Pure logic of the fresh-process Ready resume gate; no native process is started."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


GATE = module("ready_resume_control")

CONTROL = ["/old/rustred", "owner-domain-match", "--manifest", "/in/selection.json", "--owner-base", "/in",
           "--output", "/old/result.json", "--events", "/old/events.jsonl", "--stop-file", "/old/stop.json",
           "--workers", "6", "--queries", "/in/queries.json", "--follow-successors",
           "--transfer-unreserved-lookahead", "256", "--publication-policy", "ordered",
           "--route-domain-overcover", "--reuse-initial-d-bands", "--checkpoint", "/old/checkpoint",
           "--checkpoint-interval-seconds", "3600", "--unbounded-work", "--no-progress"]


def option(argv, name):
    return argv[argv.index(name) + 1]


def receipt(prefixes=2, holes=3, label="ready-multi-prefix"):
    return {"status": "paused", "resume_supported": True, "family_closure_claim": False,
            "ready_accepted_source_prefixes": prefixes, "ready_published_holes": holes,
            "ready_stream_contexts": 3, "committed_domains": 40, "contiguous_publication_watermark": 7,
            "completed_nodes": 30, "events": 1000,
            "checkpoint": {"state": "saved", "paused": True, "generation": 4, "diagnostic_pause": label}}


def manifest(label="ready-multi-prefix", paused=True):
    metadata = {"state": "saved", "paused": paused, "generation": 4, "bytes": 123}
    if label is not None:
        metadata["diagnostic_pause"] = label
    return {"schema": 5, "kind": "state", "metadata": metadata}


def trigger(prefixes=2, holes=1):
    return {"event": "diagnostic_pause", "diagnostic_pause": "ready-multi-prefix",
            "ready_accepted_source_prefixes": prefixes, "ready_published_holes": holes,
            "committed_domains": 38, "contiguous_publication_watermark": 7}


class RewriteTests(unittest.TestCase):
    def test_phases_share_the_request_and_differ_only_in_transport(self):
        paused = GATE.rewrite_command(CONTROL, "/new/rustred", Path("/out/paused"), Path("/out/paused/checkpoint"),
                                      workers=12, replacements=[("/in", "/moved")])
        resumed = GATE.rewrite_command(CONTROL, "/new/rustred", Path("/out/resumed"), Path("/out/paused/checkpoint"),
                                       resume=True, workers=12, replacements=[("/in", "/moved")])
        self.assertEqual(paused[0], "/new/rustred")
        self.assertEqual(option(paused, "--publication-policy"), "ready")
        self.assertEqual(option(paused, "--workers"), "12")
        self.assertEqual(option(paused, "--transfer-unreserved-lookahead"), "256")
        self.assertEqual(option(paused, "--queries"), "/moved/queries.json")
        self.assertEqual(option(paused, "--output"), "/out/paused/result.json")
        self.assertEqual(option(paused, "--checkpoint"), "/out/paused/checkpoint")
        self.assertNotIn("--resume", paused)
        self.assertEqual(option(resumed, "--resume"), "/out/paused/checkpoint")
        self.assertNotIn("--checkpoint", resumed)
        self.assertEqual(option(resumed, "--events"), "/out/resumed/events.jsonl")
        transport = {"--output", "--events", "--stop-file", "--checkpoint", "--resume"}

        def request(argv):
            kept, skip = [], False
            for item in argv:
                if skip:
                    skip = False
                elif item in transport:
                    skip = True
                else:
                    kept.append(item)
            return kept

        self.assertEqual(request(paused), request(resumed))
        self.assertEqual(CONTROL[0], "/old/rustred")  # The source argv is not mutated.

    def test_overrides_and_refusals(self):
        argv = GATE.rewrite_command(CONTROL, "/b", Path("/o"), Path("/o/c"), lookahead=16, inspection_workers=5)
        self.assertEqual(option(argv, "--transfer-unreserved-lookahead"), "16")
        self.assertEqual(option(argv, "--inspection-workers"), "5")
        self.assertEqual(argv.count("--checkpoint"), 1)
        with self.assertRaises(ValueError):
            GATE.rewrite_command([c for c in CONTROL if c != "--follow-successors"], "/b", Path("/o"), Path("/c"))
        with self.assertRaises(ValueError):
            GATE.rewrite_command(["/b", "owner-domain-scan"], "/b", Path("/o"), Path("/c"))
        with self.assertRaises(ValueError):
            GATE.rewrite_command("not a list", "/b", Path("/o"), Path("/c"))


class EvidenceTests(unittest.TestCase):
    def test_complete_pause_evidence_passes(self):
        evidence = GATE.pause_evidence(receipt(), manifest(), [trigger()])
        self.assertTrue(evidence["passed"], evidence["checks"])
        self.assertEqual(evidence["ready_accepted_source_prefixes"], 2)
        self.assertEqual(evidence["trigger"]["ready_published_holes"], 1)
        self.assertEqual(evidence["checkpoint_generation"], 4)

    def test_each_missing_piece_fails_its_own_check(self):
        cases = {
            "receipt_two_prefixes": (receipt(prefixes=1), manifest(), [trigger()]),
            "receipt_hole": (receipt(holes=0), manifest(), [trigger()]),
            "receipt_label": (receipt(label=None), manifest(), [trigger()]),
            "manifest_label": (receipt(), manifest(label=None), [trigger()]),
            "manifest_paused": (receipt(), manifest(paused=False), [trigger()]),
            "trigger_journaled": (receipt(), manifest(), []),
            "trigger_two_prefixes": (receipt(), manifest(), [trigger(prefixes=1)]),
            "trigger_hole": (receipt(), manifest(), [trigger(holes=0)]),
        }
        for name, arguments in cases.items():
            evidence = GATE.pause_evidence(*arguments)
            self.assertFalse(evidence["passed"], name)
            self.assertFalse(evidence["checks"][name], name)
        self.assertFalse(GATE.pause_evidence(None, None, [])["passed"])

    def test_counts_within_tolerance(self):
        baseline = {"native_inspections": 1000, "events": 50_000, "logical_records": 1200, "aliases": 200}
        close = {"native_inspections": 1019, "events": 49_100, "logical_records": 1250, "aliases": 231}
        counts = GATE.compare_counts(baseline, close, 0.02)
        self.assertTrue(counts["passed"])
        self.assertAlmostEqual(counts["rows"]["events"]["relative_difference"], 0.018)
        far = dict(close, native_inspections=1021)
        self.assertFalse(GATE.compare_counts(baseline, far, 0.02)["passed"])
        self.assertFalse(GATE.compare_counts(baseline, dict(close, events=None), 0.02)["passed"])
        self.assertIsNone(GATE.relative_difference(0, 5))


class VerdictTests(unittest.TestCase):
    def passing(self):
        phases = {"baseline": {"exit": 0}, "paused": {"exit": 4}, "resumed": {"exit": 0}}
        pause = GATE.pause_evidence(receipt(), manifest(), [trigger()])
        audits = {"baseline": {"audit": "PASS", "publication_policy": "ready"},
                  "resumed": {"audit": "PASS", "publication_policy": "ready"}}
        counts = {"passed": True, "tolerance": 0.02}
        return phases, pause, audits, counts

    def test_pass_only_when_every_stage_passes(self):
        self.assertEqual(GATE.verdict(*self.passing()), ("PASS", []))
        phases, pause, audits, counts = self.passing()
        audits["resumed"] = {"audit": "FAIL"}
        self.assertEqual(GATE.verdict(phases, pause, audits, counts)[0], "FAIL")
        phases, pause, audits, counts = self.passing()
        counts = {"passed": False, "tolerance": 0.02}
        verdict, reasons = GATE.verdict(phases, pause, audits, counts)
        self.assertEqual(verdict, "FAIL")
        self.assertIn("tolerance", reasons[0])

    def test_drained_before_trigger_is_an_explicit_failure(self):
        phases = {"baseline": {"exit": 0}, "paused": {"exit": 0}}
        audits = {"baseline": {"audit": "PASS", "publication_policy": "ready"}, "resumed": None}
        verdict, reasons = GATE.verdict(phases, None, audits, None)
        self.assertEqual(verdict, "FAIL")
        self.assertEqual(reasons, ["trigger never fired: the paused run drained to exhaustion"])

    def test_unaudited_resume_fails(self):
        phases, pause, audits, _ = self.passing()
        audits["resumed"] = None
        verdict, reasons = GATE.verdict(phases, pause, audits, None)
        self.assertEqual(verdict, "FAIL")
        self.assertIn("resumed run was not audited", reasons)

    def test_unverified_or_ordered_baseline_fails(self):
        phases, pause, audits, counts = self.passing()
        phases["baseline"] = {"reused": "/x/baseline", "exit": None, "problems": ["baseline argv differs"]}
        verdict, reasons = GATE.verdict(phases, pause, audits, counts)
        self.assertEqual((verdict, reasons), ("FAIL", ["reused baseline unverified: baseline argv differs"]))
        phases, pause, audits, counts = self.passing()
        phases["baseline"] = {"reused": "/x/baseline", "exit": 0, "problems": []}
        self.assertEqual(GATE.verdict(phases, pause, audits, counts), ("PASS", []))
        audits["baseline"]["publication_policy"] = "ordered"
        self.assertEqual(GATE.verdict(phases, pause, audits, counts), ("FAIL", ["baseline is not a Ready walk"]))


class BaselineProvenanceTests(unittest.TestCase):
    """A reused baseline stands in only for the same request run by the same binary."""

    SHA = "ab" * 32

    def earlier_run(self, root, argv=CONTROL, report=True, sha=SHA, exit_status=0, reused=False, **options):
        baseline = root / "earlier" / "baseline"
        baseline.mkdir(parents=True)
        command = GATE.rewrite_command(argv, "/old/bin/rustred", baseline, baseline / "checkpoint", **options)
        (baseline / "command.json").write_text(json.dumps(command))
        if report:
            phase = {"exit": exit_status}
            if reused:
                phase["reused"] = "/elsewhere/baseline"
            (root / "earlier" / "report.json").write_text(json.dumps(
                {"schema": GATE.SCHEMA, "binary_sha256": sha, "phases": {"baseline": phase}}))
        return baseline

    def provenance(self, baseline, **options):
        return GATE.baseline_provenance(baseline, CONTROL, dict(dict(workers=None, lookahead=None,
                                        inspection_workers=None, replacements=()), **options), self.SHA)

    def test_harness_baseline_of_the_same_request_and_binary_is_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.earlier_run(Path(temporary), workers=12)
            provenance = self.provenance(baseline, workers=12)
            self.assertEqual(provenance["problems"], [])
            self.assertEqual((provenance["binary_sha256"], provenance["exit"]), (self.SHA, 0))
            self.assertEqual(provenance["command"][0], "/old/bin/rustred")

    def test_each_mismatch_is_named(self):
        cases = {
            "baseline argv differs from this request": (dict(workers=12), dict(workers=6)),
            "no binary digest was recorded when the baseline ran": (dict(report=False), {}),
            "baseline binary differs from the binary under test": (dict(sha="cd" * 32), {}),
            "baseline exit status is not a recorded 0": (dict(exit_status=1), {}),
        }
        for problem, (earlier, options) in cases.items():
            with self.subTest(problem=problem), tempfile.TemporaryDirectory() as temporary:
                baseline = self.earlier_run(Path(temporary), **earlier)
                self.assertIn(problem, self.provenance(baseline, **options)["problems"])
        with tempfile.TemporaryDirectory() as temporary:
            # A baseline that was itself reused carries no run-time digest.
            baseline = self.earlier_run(Path(temporary), reused=True)
            self.assertIn("no binary digest was recorded when the baseline ran",
                          self.provenance(baseline)["problems"])
            (baseline / "command.json").unlink()
            self.assertIn("baseline command.json is missing or not an argv list",
                          self.provenance(baseline)["problems"])

    def test_transport_options_do_not_bind_the_request(self):
        moved = GATE.rewrite_command(CONTROL, "/b", Path("/elsewhere"), Path("/elsewhere/c"), resume=True)
        here = GATE.rewrite_command(CONTROL, "/a", Path("/here"), Path("/here/c"))
        self.assertEqual(GATE.request_argv(moved), GATE.request_argv(here))
        self.assertNotEqual(GATE.request_argv(here),
                            GATE.request_argv(GATE.rewrite_command(CONTROL, "/a", Path("/here"), Path("/here/c"),
                                                                   lookahead=16)))

    def test_main_refuses_a_foreign_baseline_before_running_anything(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            baseline = self.earlier_run(root, sha="cd" * 32)
            binary = root / "rustred"
            binary.write_text("#!/bin/sh\nexit 99\n")
            binary.chmod(0o755)
            command = root / "command.json"
            command.write_text(json.dumps(CONTROL))
            out = root / "out"
            arguments = ["--command", str(command), "--binary", str(binary), "--output", str(out),
                         "--cpus", str(min(os.sched_getaffinity(0))), "--baseline", str(baseline)]
            stderr = io.StringIO()
            with self.assertRaises(SystemExit), contextlib.redirect_stderr(stderr):
                GATE.main(arguments)
            self.assertIn("baseline binary differs from the binary under test", stderr.getvalue())
            self.assertFalse(out.exists())


class JournalTests(unittest.TestCase):
    def test_trigger_restore_and_heartbeat_maxima(self):
        lines = [
            {"event": "heartbeat", "progress": {"event": "domain_progress", "ready_accepted_source_prefixes": 1,
                                                 "ready_published_holes": 4, "ready_stream_contexts": 2}},
            {"event": "diagnostic_pause", "ready_accepted_source_prefixes": 2, "ready_published_holes": 1},
            {"event": "heartbeat", "progress": {"event": "domain_delegated", "ready_accepted_source_prefixes": 0,
                                                 "ready_published_holes": 9, "ready_stream_contexts": 3}},
            {"event": "checkpoint_restored", "restore": {"generation": 5}},
        ]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "events.jsonl"
            path.write_text("\n".join(json.dumps(line) for line in lines) + "\n{torn")
            found = GATE.journal(path, ("diagnostic_pause", "checkpoint_restored"))
            self.assertEqual(len(found["diagnostic_pause"]), 1)
            self.assertEqual(found["checkpoint_restored"][0]["restore"]["generation"], 5)
            maxima = GATE.ready_maxima(path)
            self.assertEqual(maxima, {"heartbeats": 2, "ready_accepted_source_prefixes": 1,
                                      "ready_published_holes": 9, "ready_stream_contexts": 3})
            missing = GATE.ready_maxima(Path(temporary) / "absent.jsonl")
            self.assertEqual(missing["heartbeats"], 0)
            self.assertIsNone(missing["ready_accepted_source_prefixes"])


if __name__ == "__main__":
    unittest.main()
