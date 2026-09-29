"""CP6 outer contracts using fake commands only; no native process or algebra."""
import argparse
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from test_audit_owner_domain_walk import AUDIT, build_run, module

MATCH = module("match_shared_owner_domains")
PRODUCTION = module("production_saved_owner_campaign")
SUPERVISOR = module("shared_owner_campaign")
MONITOR = module("campaign_monitor")


class EpochCheckpointSteeringTests(unittest.TestCase):
    def test_match_fresh_and_resume_forward_epoch_without_changing_default(self):
        arguments = ["match", "--executable", "native", "--manifest", "selection.json",
                     "--queries", "queries.json", "--output", "result.json",
                     "--follow-successors"]
        for operation in ("--checkpoint", "--resume"):
            flags = [operation, "checkpoint", "--publication-policy", "epoch",
                     "--transfer-unreserved-lookahead", "16"]
            with patch("sys.argv", arguments + flags), patch.object(MATCH.os, "execve") as execute:
                MATCH.main()
            command = execute.call_args.args[1]
            for flag in (operation, "--publication-policy", "--transfer-unreserved-lookahead"):
                self.assertEqual(command.count(flag), 1)
                self.assertEqual(command[command.index(flag) + 1], flags[flags.index(flag) + 1])
        with patch("sys.argv", arguments), patch.object(MATCH.os, "execve") as execute:
            MATCH.main()
        self.assertNotIn("--publication-policy", execute.call_args.args[1])
        for extra in ([], ["--apply-subdivision-axis", "0", "--apply-subdivision-cut", "2",
                          "--transfer-unreserved-lookahead", "16"]):
            with patch("sys.argv", arguments + ["--publication-policy", "epoch"] + extra), \
                    patch.object(MATCH.os, "execve") as execute, patch("sys.stderr", new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    MATCH.main()
                execute.assert_not_called()

    def test_production_epoch_is_explicit_frozen_and_rescue_off(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = root / "inputs"
            inputs.mkdir()
            (root / "bin").mkdir()
            (inputs / "queries.json").write_text('{"queries":[]}')
            args = argparse.Namespace(**{key: None for key in PRODUCTION.FROZEN_OPTIONS})
            args.publication_policy = "epoch"
            args.workers = 1
            args.resume = False
            policy = PRODUCTION.frozen_policy(root, args, Path("/frozen/native"), inputs, 0, 0)
            self.assertFalse(policy["options"]["auto_rescue"])
            command = policy["command_arguments"]
            self.assertEqual(command[command.index("--publication-policy") + 1], "epoch")
            self.assertNotIn("--auto-rescue", command)
            self.assertNotIn("--g2-residual-anchors", command)
            before = (root / "bin" / "steering.json").read_bytes()
            args.resume = True
            self.assertEqual(PRODUCTION.frozen_policy(root, args, Path("/frozen/native"),
                                                     inputs, 0, 0), policy)
            self.assertEqual((root / "bin" / "steering.json").read_bytes(), before)
            args.publication_policy = "ready"
            with self.assertRaisesRegex(ValueError, "differs from frozen"):
                PRODUCTION.frozen_policy(root, args, Path("/frozen/native"), inputs, 0, 0)
            args.resume = False
            args.publication_policy = "epoch"
            args.auto_rescue = True
            refused = root / "refused"
            (refused / "bin").mkdir(parents=True)
            with self.assertRaisesRegex(ValueError, "explicit complete query_roles"):
                PRODUCTION.frozen_policy(refused, args, Path("/frozen/native"), inputs, 0, 0)
            self.assertFalse((refused / "bin" / "steering.json").exists())
            args.auto_rescue = False
            args.g2_residual_anchors = "union"
            union = PRODUCTION.frozen_policy(refused, args, Path("/frozen/native"), inputs, 0, 0)
            self.assertEqual(union["options"]["g2_residual_anchors"], "union")
            self.assertEqual(union["options"]["publication_policy"], "epoch")

    def test_explicit_epoch_rescue_preserves_role_scope_and_frozen_resume(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs = root / "inputs"
            inputs.mkdir()
            (root / "bin").mkdir()
            document = {"queries": [{"id": "physical"}, {"id": "helper"}],
                        "query_roles": {"required": ["physical"], "auxiliary": ["helper"]}}
            text = json.dumps(document)
            (inputs / "queries.json").write_text(text)
            args = argparse.Namespace(**{key: None for key in PRODUCTION.FROZEN_OPTIONS})
            args.publication_policy = "epoch"
            args.workers = 1
            args.resume = False
            args.auto_rescue = True
            args.g2_residual_anchors = "union"
            args.epoch_rolling = True
            policy = PRODUCTION.frozen_policy(root, args, Path("/frozen/native"), inputs, 2, len(text))
            self.assertTrue(policy["options"]["auto_rescue"])
            self.assertEqual(policy["command_arguments"].count("--auto-rescue"), 1)
            before = (root / "bin" / "steering.json").read_bytes()
            args.resume = True
            self.assertEqual(PRODUCTION.frozen_policy(root, args, Path("/frozen/native"),
                                                     inputs, 2, len(text)), policy)
            self.assertEqual((root / "bin" / "steering.json").read_bytes(), before)
            self.assertEqual((inputs / "queries.json").read_text(), text)
            args.auto_rescue = False
            with self.assertRaisesRegex(ValueError, "differs from frozen"):
                PRODUCTION.frozen_policy(root, args, Path("/frozen/native"), inputs, 2, len(text))

    def test_cp6_cannot_enter_executable_upgrade_protocol(self):
        with tempfile.TemporaryDirectory() as temporary:
            checkpoint = Path(temporary)
            (checkpoint / "latest.json").write_text(json.dumps({"manifest": {
                "format": "RUSTRED-WALK-CP6", "schema": 1, "generation": 1}, "blake3": [0] * 32}))
            before = (checkpoint / "latest.json").read_bytes()
            with self.assertRaises(ValueError):
                PRODUCTION.checkpoint_identity(checkpoint)
            self.assertEqual((checkpoint / "latest.json").read_bytes(), before)

    def test_summary_never_becomes_full_python_audit_even_with_closed_raw_report(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = build_run(Path(temporary), policy="epoch")
            (run / "result.json").write_text(json.dumps({
                "checkpoint": {"format": "RUSTRED-WALK-CP6", "state": "saved", "generation": 1},
                "full_result_in_output_document": False, "full_state_in_checkpoint": True,
                "recursive_worklist_exhausted": True, "all_scheduled_domains_resolved": False,
                "finalization": "not_evaluated"}))
            verifier = run / "raw-verifier.json"
            verifier.write_text('{"verdict":"PASS","roots_total":1,"roots_independently_verified":1}')
            report = AUDIT.audit_walk(run, require_closure=True, verify_report=verifier)
            self.assertEqual(report["audit"], "INCOMPLETE")
            self.assertFalse(report["all_local_obligations_discharged"])
            self.assertNotIn("verifier_pairing", report)
            self.assertIn("with --no-result", report["incomplete_reason"])
            self.assertIn("omitting --result still auto-selects", report["incomplete_reason"])

    def test_cp6_flat_native_event_survives_real_cli_progress_envelope(self):
        checkpoint = {"state": "saved", "format": "RUSTRED-WALK-CP6", "schema": 1,
                      "generation": 4, "paused": True, "resumable": True}
        native = {"event": "checkpoint_saved", "phase": "epoch_merge",
                  "scheduled_nodes": 17, "completed_nodes": 9, "queued_nodes": 8,
                  "initial_entry_domains_total": 3, "initial_entry_domains_inspected": 2,
                  "initial_entry_domains_published": 2, "frontiers": 0,
                  "checkpoint": checkpoint,
                  "descendant_closure": {
                      "available": True, "initial_total": 3, "initial_closed": 1,
                      "total_domains": 17, "total_closed": 4, "unresolved_domains": 13,
                      "snapshot_stale": True, "graph_revision": 25, "snapshot_revision": 19,
                      "family_closure_claim": False}}
        # cli/owner_match wraps the observer payload once under progress.
        record = {"event": "progress", "progress": native,
                  "queue_growth_per_second": 2.5, "recent_nodes_per_second": 5}
        progress = MONITOR.progress_summary(record, 10, 12)
        self.assertEqual(progress["phase"], "epoch_merge")
        self.assertEqual(progress["work"]["scheduled"], 17)
        self.assertEqual(progress["work"]["locally_completed"], 9)
        self.assertEqual(progress["work"]["pending"], 8)
        self.assertEqual(progress["work"]["pending_growth_per_second"], 2.5)
        self.assertEqual(progress["checkpoint"], checkpoint)
        self.assertEqual(progress["initial_entry_progress"]["total"], 3)
        self.assertFalse(progress["family_closure_claim"])
        self.assertTrue(progress["descendant_closure"]["available"])
        self.assertEqual(progress["descendant_closure"]["unresolved_domains"], 13)
        self.assertTrue(progress["descendant_closure"]["snapshot_stale"])
        finished = dict(native, event="finished", status="incomplete",
                        full_result_in_output_document=False, full_state_in_checkpoint=True,
                        recursive_worklist_exhausted=True, all_scheduled_domains_resolved=False,
                        finalization="not_evaluated")
        final = MONITOR.progress_summary({"event": "progress", "progress": finished}, 10, 12)
        self.assertEqual(final["descendant_closure"], progress["descendant_closure"])
        self.assertEqual(final["native_status"], "incomplete")

    def test_saved_epoch_terminal_states_retain_resume_without_claiming_completion(self):
        checkpoint = {"state": "saved", "format": "RUSTRED-WALK-CP6", "generation": 1, "resumable": True}
        native = {"status": "incomplete", "full_result_in_output_document": False,
                  "recursive_worklist_exhausted": True, "all_scheduled_domains_resolved": False,
                  "finalization": "not_evaluated", "checkpoint": checkpoint}
        for stop, expected in ((None, "checkpoint_only"), ("paused", "paused"),
                               ("ram_guard", "stopped"), ("domain_allowance", "stopped"),
                               ("frontier_stop", "stopped"), ("error_stop", "stopped")):
            row = dict(native, status="stopped" if stop else "incomplete", stop_reason=stop,
                       recursive_worklist_exhausted=stop is None)
            event = {"progress": row}
            progress = MONITOR.progress_summary(event, 0, 0)
            self.assertEqual(SUPERVISOR.terminal_state(4, "epoch", event, progress, checkpoint, None), expected)
        event = {"progress": native}
        progress = MONITOR.progress_summary(event, 0, 0)
        self.assertEqual(SUPERVISOR.terminal_state(0, "epoch", event, progress, checkpoint, None), "failed")
        self.assertEqual(SUPERVISOR.terminal_state(0, "ready", {}, {}, None, None), "completed")
        self.assertTrue(SUPERVISOR.resumable_checkpoint(checkpoint))
        self.assertFalse(SUPERVISOR.resumable_checkpoint(dict(checkpoint, resumable=False)))
        self.assertFalse(SUPERVISOR.resumable_checkpoint({k:v for k,v in checkpoint.items() if k != "resumable"}))
        self.assertTrue(SUPERVISOR.resumable_checkpoint({"state": "saved", "format": "RUSTRED-WALK-CP5"}))
        args = argparse.Namespace(publication_policy="epoch", checkpoint=Path("/saved"), resume=None,
                                  run_directory=Path("/old-run"), cpus="0", auto_rescue=False)
        restarted = SUPERVISOR.restart_command(args, Path("/old-run"), "/saved", {0})
        self.assertEqual(restarted[restarted.index("--publication-policy") + 1], "epoch")
        self.assertEqual(restarted[restarted.index("--resume") + 1], "/saved")
        self.assertNotIn("--auto-rescue", restarted)

    def test_cp6_poison_and_missing_terminal_handoff_invalidate_stale_saved_receipts(self):
        saved = {"state": "saved", "format": "RUSTRED-WALK-CP6", "generation": 3, "resumable": True}
        poison = dict(saved, state="poisoned", resumable=False, generation=None)
        current = SUPERVISOR.advance_checkpoint(saved, poison)
        self.assertEqual(SUPERVISOR.advance_checkpoint(current, saved), poison)
        for status in (0, 1, -9, 4):
            stale = SUPERVISOR.terminal_checkpoint("epoch", status, {"progress": {"event": "heartbeat"}}, saved)
            self.assertFalse(SUPERVISOR.resumable_checkpoint(stale))
            # Actual terminal order: invalidate, then final publish_status
            # replays both progress.checkpoint and tail.saved_checkpoint.
            for candidate in (dict(saved), saved):
                stale = SUPERVISOR.advance_checkpoint(stale, candidate)
            self.assertEqual(stale["state"], "unconfirmed_after_process_exit")
            self.assertFalse(SUPERVISOR.resumable_checkpoint(stale))
            self.assertEqual(SUPERVISOR.terminal_state(
                status, "epoch", {}, {}, stale, None), "failed")
        event = {"progress": {"event": "finished", "checkpoint": saved,
                              "full_result_in_output_document": False, "full_state_in_checkpoint": True}}
        self.assertEqual(SUPERVISOR.terminal_checkpoint("epoch", 4, event, saved), saved)
        self.assertEqual(SUPERVISOR.terminal_checkpoint("ready", 1, {}, {"state": "saved"}), {"state": "saved"})


if __name__ == "__main__":
    unittest.main()
