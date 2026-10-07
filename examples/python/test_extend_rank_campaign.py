"""Pure steering tests, not native checkpoint or closure certification."""

from contextlib import redirect_stdout
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import extend_rank_campaign as EXTEND
import prepare_rank_campaign as PREPARE
import test_prepare_rank_campaign as PREPARE_TESTS

snapshot = PREPARE_TESTS.snapshot


class RankContinuationTests(unittest.TestCase):
    def setUp(self):
        PREPARE.WORKSPACE_TEMP.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=PREPARE.WORKSPACE_TEMP)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        source = PREPARE_TESTS.RankCampaignStagingTests().fixture(self.root)
        self.campaign = self.root / "scalar"
        PREPARE.prepare(source, self.campaign, 0, max_power_difference=4)
        self.checkpoint = self.campaign / "checkpoints/main"
        self.checkpoint.mkdir(parents=True)
        (self.checkpoint / "checkpoint.lock").touch()
        (self.campaign / "steering").mkdir()
        (self.campaign / "steering/production_saved_owner_campaign.py").write_text("# frozen fixture\n")
        (self.campaign / "bin").mkdir()
        (self.campaign / "bin/steering.json").write_text("{}")
        self.metadata = {"schema": 5, "request": "a" * 64, "initial_admission": "complete",
                         "processed_queries": 2, "total_queries": 2,
                         "engine_certification_void": False, "amendments": [], "p0": 1}
        self.save()

    def save(self):
        raw = EXTEND.encoded(self.metadata)
        (self.checkpoint / "metadata.part").write_bytes(raw)
        (self.checkpoint / "latest.json").write_bytes(EXTEND.encoded({"manifest": {
            "format": "RUSTRED-WALK-CP6", "schema": 3, "generation": 1,
            "resumable": True, "files": [{"key": "meta", "file": "metadata.part", "bytes": len(raw)}]}}))

    def record(self, sequence):
        document = json.loads((self.campaign / f"amendments/amendment-{sequence:04d}.json").read_bytes())
        self.metadata["amendments"].append({"sequence": sequence, "parent": document["parent"],
                                           "queries": len(document["queries"]),
                                           "first_input": 2 + sum(row["queries"] for row in self.metadata["amendments"]),
                                           "digest": str(sequence) * 64})
        self.save()
        return document

    def test_preview_and_prepare_preserve_checkpoint_inputs_and_never_run_native(self):
        before = snapshot(self.campaign)
        with patch.object(EXTEND.subprocess, "call") as run:
            preview = EXTEND.extend(self.campaign, 1, dry_run=True)
            self.assertEqual(snapshot(self.campaign), before)
            receipt = EXTEND.extend(self.campaign, 1)
            run.assert_not_called()
        self.assertEqual(receipt["cumulative_required_queries"], 4)
        self.assertEqual(receipt["new_required_queries"], 2)
        self.assertEqual(receipt["unchanged_auxiliary_queries"], 0)
        self.assertEqual(receipt["original_root_prefix"], 1)
        self.assertFalse(receipt["closure_claim"])
        self.assertFalse(receipt["native_work_started"])
        after = snapshot(self.campaign)
        self.assertEqual({key: after[key] for key in before}, before)
        self.assertEqual(set(after) - set(before), {"amendments/amendment-0001.json"})
        amendment = json.loads(after["amendments/amendment-0001.json"])
        self.assertEqual(amendment["schema"], EXTEND.SCHEMA)
        self.assertEqual(amendment["parent"], self.metadata["request"])
        self.assertNotIn("supersede", amendment)
        self.assertEqual(amendment["query_roles"]["auxiliary"], [])
        self.assertEqual(amendment["query_roles"]["required"], [row["id"] for row in amendment["queries"]])
        self.assertEqual(preview["amendment_sha256"], receipt["amendment_sha256"])
        self.assertEqual(EXTEND.extend(self.campaign, 1), receipt)  # pending stage idempotent

    def test_two_stages_rederive_original_geometry_with_monotone_caps(self):
        EXTEND.extend(self.campaign, 1)
        first = self.record(1)
        receipt = EXTEND.extend(self.campaign, 2, max_power_difference=5)
        second = json.loads(Path(receipt["amendment"]).read_bytes())
        self.assertEqual(second["parent"], "1" * 64)
        self.assertEqual(receipt["cumulative_required_queries"], 6)
        self.assertTrue(set(first["query_roles"]["required"]).isdisjoint(second["query_roles"]["required"]))
        source = json.loads((self.campaign / "inputs" / PREPARE.SOURCE_QUERIES_NAME).read_bytes())
        originals = {row["id"]: row for row in source["queries"]}
        for mapping, row in zip(second["provenance"]["source_query_ids"], second["queries"]):
            original = originals[mapping["source_id"]]
            expected = copy.deepcopy(original)
            expected["id"] = row["id"]
            expected["max_numerator_rank"] = min(original["max_numerator_rank"], 2)
            expected["power_bounds"]["max_power_difference"] = 5
            self.assertEqual(row, expected)
        self.record(2)
        for rank, difference in ((1, 6), (2, 4), (2, 5)):
            with self.subTest(rank=rank, difference=difference), self.assertRaises(ValueError):
                EXTEND.extend(self.campaign, rank, max_power_difference=difference)
        raised = EXTEND.extend(self.campaign, 2, max_power_difference=None)
        self.assertIsNone(raised["max_power_difference"])

    def test_pending_stage_and_tampered_chain_fail_closed(self):
        EXTEND.extend(self.campaign, 1)
        with self.assertRaisesRegex(ValueError, "pending"):
            EXTEND.extend(self.campaign, 2)
        self.record(1)
        path = self.campaign / "amendments/amendment-0001.json"
        value = json.loads(path.read_bytes())
        value["queries"][0]["upper"][0] += 1
        path.write_bytes(EXTEND.encoded(value))
        with self.assertRaisesRegex(ValueError, "geometry"):
            EXTEND.extend(self.campaign, 2)

    def test_tampered_attachment_and_incomplete_admission_refused(self):
        self.metadata["initial_admission"] = "in_progress"
        self.save()
        with self.assertRaisesRegex(ValueError, "admission"):
            EXTEND.extend(self.campaign, 1)
        self.metadata["initial_admission"] = "complete"
        self.save()
        path = self.campaign / "inputs" / PREPARE.SOURCE_QUERIES_NAME
        path.chmod(0o644)  # Only this disposable test fixture is intentionally corrupted.
        path.write_bytes(path.read_bytes() + b" ")
        with self.assertRaises(ValueError):
            EXTEND.extend(self.campaign, 1)

    def test_live_campaign_and_locked_checkpoint_refused(self):
        with patch.object(EXTEND.PRODUCTION, "campaign_run_liveness", return_value=["native alive"]):
            with self.assertRaisesRegex(ValueError, "still running"):
                EXTEND.extend(self.campaign, 1)
        with EXTEND.PRODUCTION.checkpoint_lock(self.checkpoint):
            with self.assertRaisesRegex(ValueError, "in use"):
                EXTEND.extend(self.campaign, 1)
        self.assertFalse((self.campaign / "amendments").exists())

    def test_cli_launch_requires_explicit_start(self):
        arguments = ["--campaign-directory", str(self.campaign), "--extend-rank", "1"]
        with redirect_stdout(io.StringIO()), patch.object(EXTEND.subprocess, "call", return_value=4) as run:
            self.assertEqual(EXTEND.main(arguments), 0)
            run.assert_not_called()
            self.assertEqual(EXTEND.main([*arguments, "--start"]), 4)
            self.assertIn("--resume", run.call_args.args[0])
            self.assertEqual(run.call_args.args[0][-1], "--start")

    def test_cli_preview_does_not_offer_unprepared_resume_as_ready(self):
        with redirect_stdout(io.StringIO()) as output, patch.object(EXTEND.subprocess, "call") as run:
            self.assertEqual(EXTEND.main(["--campaign-directory", str(self.campaign),
                                         "--extend-rank", "1", "--dry-run"]), 0)
            run.assert_not_called()
        self.assertIn("no amendment was written", output.getvalue())
        self.assertNotIn("Resume when ready:", output.getvalue())
        self.assertFalse((self.campaign / "amendments").exists())

    def test_interrupted_atomic_publish_leaves_no_partial_amendment(self):
        with patch.object(EXTEND.os, "link", side_effect=KeyboardInterrupt):
            with self.assertRaises(KeyboardInterrupt):
                EXTEND.extend(self.campaign, 1)
        self.assertEqual(list((self.campaign / "amendments").iterdir()), [])
        receipt = EXTEND.extend(self.campaign, 1)
        self.assertTrue(Path(receipt["amendment"]).is_file())

    def test_unsnapshotted_python_uses_sibling_launcher_and_preserves_frozen_policy(self):
        (self.campaign / "steering/production_saved_owner_campaign.py").unlink()
        before = snapshot(self.campaign / "bin")
        receipt = EXTEND.extend(self.campaign, 1)
        self.assertEqual(receipt["resume_command"][2], str(Path(EXTEND.PRODUCTION.__file__).resolve()))
        self.assertEqual(snapshot(self.campaign / "bin"), before)

    def test_phase_two_opt_in_uses_new_launcher_without_rewriting_frozen_snapshot(self):
        (self.campaign / "master-reduction").mkdir()
        (self.campaign / "master-reduction/policy.json").write_text('{"enabled":true}')
        before = snapshot(self.campaign / "steering")
        receipt = EXTEND.extend(self.campaign, 1)
        self.assertEqual(receipt["resume_command"][2], str(Path(EXTEND.PRODUCTION.__file__).resolve()))
        self.assertEqual(snapshot(self.campaign / "steering"), before)

    def test_phase_dispatcher_lock_prevents_extending_inflight_master_scope(self):
        import fcntl
        directory = self.campaign / "master-reduction"
        directory.mkdir()
        with (directory / "dispatcher.lock").open("wb") as stream:
            fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
            with self.assertRaisesRegex(ValueError, "dispatcher is still running"):
                EXTEND.extend(self.campaign, 1)
        self.assertFalse((self.campaign / "amendments").exists())

    def test_two_cap_disjoint_query_restored_without_rewriting_base(self):
        # A second independent fresh fixture has one fixed-D5 required query.
        source = (self.campaign / "inputs" / PREPARE.SOURCE_QUERIES_NAME).read_bytes()
        value = json.loads(source)
        value["queries"][1]["power_bounds"].update(min_power_difference=5, max_power_difference=5)
        data = EXTEND.encoded(value)
        scoped, receipt = PREPARE.plan_rank_queries(data, 0, max_power_difference=4)
        self.assertEqual(receipt["omitted_disjoint_power_difference_query_ids"], ["required-second"])
        self.assertEqual(len(json.loads(scoped)["queries"]), 1)
        extension = EXTEND.stage_document(data, 0, 1, "a" * 64, "b" * 64, 0, 5, 4)
        self.assertEqual(len(extension["queries"]), 2)
        self.assertEqual(extension["queries"][0]["power_bounds"]["min_power_difference"], 5)
        self.assertEqual(extension["queries"][0]["power_bounds"]["max_power_difference"], 5)
        EXTEND.validate_extension(0, 0, 4, 5)
        with self.assertRaisesRegex(ValueError, "lower"):
            EXTEND.validate_extension(0, 1, None, 5)


if __name__ == "__main__":
    unittest.main()
