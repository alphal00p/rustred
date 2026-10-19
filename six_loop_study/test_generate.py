"""Steering tests only: native algebra is deliberately not started here."""
import argparse
import contextlib
import io
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import generate


class StudyTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(dir=generate.ROOT / "TMP")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.topology = self.base / "topology"
        self.topology.mkdir()
        self.output = self.base / "run"
        self.exe = self.base / "native"
        self.exe.write_text("#!/bin/sh\nexit 99\n")
        self.exe.chmod(0o755)
        self.family = self.topology / "a.toml"
        self.family.write_text('''schema = "rustred.project.toml.v1"
[family]
name = "test"
loop_momenta = ["k1", "k2"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "D1"
expression = "k1^2-1"
[[family.denominators]]
id = "D2"
expression = "(k1+k2)^2-1"
[[family.denominators]]
id = "D3"
expression = "k2^2-1"
''')
        (self.topology / "a.dot").write_text("graph test {}\n")
        self.manifest = self.topology / "manifest.json"
        generate.write(self.manifest, dict(schema=generate.TOPOLOGY_SCHEMA, entries=[dict(
            id="parent-a", input="a.toml", dot="a.dot", loop_count=2,
            physical_propagators=[0, 2], auxiliary_slots=[1])]))
        self.affinity = self.enterContext(patch.object(generate.scheduler.os, "sched_getaffinity",
                                                      return_value={64,65,66,67}))

    def args(self, **overrides):
        values = dict(directory=self.output, manifest=self.manifest, executable=self.exe,
            workers=4, cpus="64-67", jobs=2, max_memory_bytes=1_000_000_000,
            ram_margin_percent=None, rank=None, numerical_depth=None, finite_case_policy=None,
            exact_backend=None, discovery_strategy=None, full_downsets=False,
            execution_mode="isolated", max_active_sectors=None)
        values.update(overrides)
        return argparse.Namespace(**values)

    def test_prepare_does_not_execute_and_preserves_nontrailing_auxiliary_slots(self):
        with patch.object(generate.scheduler, "run") as native:
            plan = generate.prepare(self.args())
        native.assert_not_called()
        self.assertEqual(plan["rank"], 0)
        self.assertEqual(plan["finite_case_policy"], "search")
        self.assertEqual(plan["allocation"]["workers_per_job"], 2)
        self.assertEqual(plan["resources"]["memory_policy"], "process-limit")
        self.assertEqual(plan["resources"]["swap_growth_stop_bytes_per_second"], 0)
        command = generate.command(self.output, plan, plan["owners"][0])
        self.assertEqual(command[command.index("--nonpositive-indices")+1], "1")
        self.assertNotIn("--selected-sectors", command)
        self.assertIn("--root-zero-census", command)
        self.assertNotIn("--resume", command)
        self.assertEqual(command[command.index("--checkpoint-max-bytes")+1], str(generate.USIZE_MAX))
        self.assertEqual(command[command.index("--bundle-max-polynomial-terms")+1], str(generate.USIZE_MAX))
        self.assertEqual(command[command.index("--bundle-max-term-operations")+1], str(generate.USIZE_MAX))
        self.assertEqual(command[command.index("--bundle-max-bytes")+1], "950000000")
        self.assertNotIn("--finite-max-visited-points", command)

    def test_historical_manifest_remains_readable(self):
        value = generate.read(self.manifest)
        value["schema"] = "six-loop-study.topologies.v1"
        generate.write(self.manifest, value)
        self.assertEqual(len(generate.manifest_entries(self.manifest)[1]), 1)

    def test_unknown_manifest_schema_is_not_accepted(self):
        value = generate.read(self.manifest)
        value["schema"] = "unrecognized"
        generate.write(self.manifest, value)
        with self.assertRaisesRegex(ValueError, "unsupported topology manifest"):
            generate.manifest_entries(self.manifest)

    def test_shared_entrypoint_is_used_in_printed_resume_command(self):
        entrypoint = generate.ROOT / "seven_loops/generate.py"
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            code = generate.main([
                "--directory", str(self.output), "--manifest", str(self.manifest),
                "--executable", str(self.exe), "--workers", "4", "--cpus", "64-67",
                "--max-memory-bytes", "1000000000",
            ], entrypoint=entrypoint)
        self.assertEqual(code, 0)
        self.assertIn(str(entrypoint), json.loads(output.getvalue())["resume_command"])

    def test_shared_pool_uses_entire_budget_without_dividing_it_between_parents(self):
        plan = generate.prepare(self.args(execution_mode="shared", jobs=None))
        self.assertEqual(plan["resources"]["generation_jobs"], 1)
        self.assertTrue(plan["allocation"]["shared_pool"])
        self.assertEqual(plan["resources"]["max_active_sectors"], None)
        command = generate.command(self.output, plan, plan["owners"][0])
        self.assertEqual(command[command.index("--n-cores")+1], "4")

    def test_shared_parent_count_need_not_divide_workers_and_sector_limit_is_explicit(self):
        plan = generate.prepare(self.args(execution_mode="shared", jobs=3, max_active_sectors=2))
        self.assertEqual(plan["allocation"]["jobs"], 3)
        self.assertEqual(plan["allocation"]["max_active_sectors"], 2)
        generate.validate(self.output, plan)

    def test_shared_runtime_caps_can_change_on_resume_without_mutating_frozen_plan(self):
        plan = generate.prepare(self.args(execution_mode="shared", jobs=2, max_active_sectors=2))
        updated = generate.resume_resources(plan, 1, "all")
        self.assertEqual(plan["resources"]["max_active_sectors"], 2)
        self.assertEqual(plan["resources"]["generation_jobs"], 2)
        self.assertIsNone(updated["resources"]["max_active_sectors"])
        self.assertEqual(updated["resources"]["generation_jobs"], 1)
        self.assertEqual(plan["bound_files"], updated["bound_files"])
        generate.validate(self.output, updated)

    def test_resume_accepts_new_workers_cpus_jobs_and_ram_without_touching_math(self):
        plan = generate.prepare(self.args(execution_mode="shared", jobs=2))
        changed = generate.resume_resources(plan, 7, 1, workers=2, cpus="64-65", max_memory_bytes=2_000_000_000)
        self.assertEqual(changed["resources"]["workers"], 2)
        self.assertEqual(changed["allocation"]["jobs"], 7)
        self.assertEqual(changed["allocation"]["slots"], [[64,65]])
        self.assertEqual(changed["resources"]["max_memory_bytes"], 2_000_000_000)
        self.assertEqual(changed["payload_bytes"], plan["payload_bytes"], "native byte permissions are frozen, not a RAM reservation")
        self.assertEqual(changed["bound_files"], plan["bound_files"])
        self.assertEqual(plan["resources"]["workers"], 4)
        command = generate.command(self.output, changed, changed["owners"][0])
        self.assertEqual(command[command.index("--n-cores")+1], "2")
        generate.validate(self.output, changed)

    def test_resource_preview_handles_old_cpu_affinity_without_mutating_plan(self):
        generate.prepare(self.args(execution_mode="shared"))
        before = (self.output / "generation.json").read_bytes()
        output = io.StringIO()
        with patch.object(generate.scheduler.os, "sched_getaffinity", return_value={64,65}), \
                contextlib.redirect_stdout(output):
            result = generate.main(["--directory", str(self.output), "--resume", "--workers", "2", "--cpus", "64-65", "--jobs", "3"])
        self.assertEqual(result, 0)
        self.assertEqual(json.loads(output.getvalue())["allocation"]["total_workers"], 2)
        self.assertIn("--workers 2 --cpus 64-65 --jobs 3", json.loads(output.getvalue())["resume_command"])
        self.assertEqual((self.output / "generation.json").read_bytes(), before)

    def test_started_resource_choices_persist_and_inputs_stay_frozen(self):
        plan = generate.prepare(self.args())
        changed = generate.resume_resources(plan, 1, workers=2, cpus="64-65")
        def run(jobs, attempt, resources, completed, **kwargs):
            for job in jobs:
                self.fake_outputs(job)
                completed(job)
        with patch.object(generate.scheduler, "run", side_effect=run), contextlib.redirect_stdout(io.StringIO()):
            generate.execute(self.output, changed, dashboard=False)
        stored = generate.read(self.output / "generation.json")
        self.assertEqual(stored["resources"], changed["resources"])
        self.assertEqual(stored["bound_files"], plan["bound_files"])
        with self.assertRaisesRegex(ValueError, "frozen generation inputs"):
            generate.execute(self.output, {**stored, "rank":1}, dashboard=False)

    def test_active_sector_limit_accepts_explicit_all_and_positive_counts(self):
        self.assertEqual(generate.active_sector_limit("all"), "all")
        self.assertEqual(generate.active_sector_limit("32"), 32)
        for value in ["0", "-1", "no"]:
            with self.assertRaises(argparse.ArgumentTypeError): generate.active_sector_limit(value)

    def test_abandoned_metadata_temporary_does_not_block_atomic_retry(self):
        stale = self.base / ".receipt.json.old.part"
        stale.write_text("interrupted")
        destination = self.base / "receipt.json"
        generate.write(destination, {"status": "complete"})
        self.assertEqual(generate.read(destination), {"status": "complete"})
        self.assertTrue(stale.exists(), "never delete an unowned interrupted temporary")

    def test_portable_frozen_inputs_and_binary_reject_changes(self):
        plan = generate.prepare(self.args())
        moved = self.base / "moved"
        self.output.rename(moved)
        self.exe.unlink()
        shutil.rmtree(self.topology)
        generate.validate(moved, plan)
        command = generate.command(moved, plan, plan["owners"][0])
        self.assertEqual(command[0], str(moved / "bin/rustred"))
        (moved / plan["owners"][0]["input"]).write_text("changed")
        with self.assertRaisesRegex(ValueError, "frozen input"):
            generate.validate(moved, plan)

    def test_resume_uses_checkpoint_without_changing_rank_or_resources(self):
        plan = generate.prepare(self.args())
        sectors = self.output / "generation/parent-a/sectors"
        sectors.mkdir()
        with self.assertRaisesRegex(ValueError, "incomplete checkpoint"):
            generate.command(self.output, plan, plan["owners"][0])
        (sectors / "checkpoint.toml").write_text("# native owns validation\n")
        command = generate.command(self.output, plan, plan["owners"][0])
        self.assertEqual(command[-2:], ["--resume", "--force"])
        with contextlib.redirect_stderr(io.StringIO()), patch.object(generate.scheduler, "run") as native:
            code = generate.main(["--directory", str(self.output), "--resume", "--start", "--rank", "1"])
        self.assertEqual(code, 1)
        native.assert_not_called()

    def fake_outputs(self, job, *, wrong_rank=False):
        run = Path(job.command[job.command.index("--output")+1]).parent
        (run / "sectors").mkdir(exist_ok=True)
        source = Path(job.command[job.command.index("--input")+1]).read_text()
        (run / "sectors/checkpoint.toml").write_text(
            'root_zero_census = true\nroot_sector = [true, false, true]\n'
            'family_fingerprint = "test-family"\nfamily_source = ' + json.dumps(source) + '\n')
        (run / "candidates.rrbin").write_bytes(b"test-only native stub")
        (run / "generation.toml").write_text('''schema = "rustred.family-candidates-output.toml.v1"
status = "uncertified-candidates"
family_fingerprint = "test-family"
generation_scope = "root-downset"
root_sector = [true, false, true]
arity = 3
max_numerator_rank = %d
numerical_depth = 0
exact_backend = "sparse-factorized"
finite_case_policy = "search"
generated_rules = 9
finite_residuals = 2
solved_sectors = 3
total_us = 1000000
[discovery_strategy]
version = 1
[discovery_strategy.sectors]
kind = "active-first"
[discovery_strategy.rows]
kind = "features"
[[discovery_strategy.rows.priorities]]
descending = false
[discovery_strategy.rows.priorities.feature]
kind = "terms"
[[discovery_strategy.rows.priorities]]
descending = false
[discovery_strategy.rows.priorities.feature]
kind = "coefficient-monomials"
''' % (1 if wrong_rank else 0))

    def test_completed_outputs_checked_and_skip_resume_without_native(self):
        plan = generate.prepare(self.args())
        def run(jobs, attempt, resources, completed, **kwargs):
            for job in jobs:
                self.assertIsInstance(job.metadata["parent"], int)
                self.assertEqual(job.metadata["root"], "101")
                self.assertIsInstance(job.metadata["selected_sectors"], list)
                self.fake_outputs(job)
                completed(job)
        with patch.object(generate.scheduler, "run", side_effect=run) as native, \
             contextlib.redirect_stdout(io.StringIO()):
            summary = generate.execute(self.output, plan, dashboard=False)
            self.assertEqual(summary["generated_rules"], 9)
            self.assertFalse(summary["family_closure_claim"])
            generate.execute(self.output, plan, dashboard=False)
            self.assertEqual(native.call_count, 1)
        (self.output / "generation/parent-a/candidates.rrbin").write_bytes(b"altered")
        with self.assertRaisesRegex(ValueError, "completed output changed"):
            generate.completed(self.output, plan, plan["owners"][0])

    def test_changed_plan_or_incomplete_receipt_cannot_skip_native_work(self):
        plan = generate.prepare(self.args())
        owner = plan["owners"][0]
        job = generate.scheduler.Job(owner["id"], tuple(generate.command(self.output, plan, owner)), owner)
        self.fake_outputs(job)
        generate.record_completed(self.output, plan, owner)
        with self.assertRaisesRegex(ValueError, "report differs"):
            generate.completed(self.output, {**plan, "rank": 1}, owner)
        receipt = self.output / "generation/parent-a/completed.json"
        value = generate.read(receipt)
        del value["outputs"]["candidates.rrbin"]
        generate.write(receipt, value)
        with self.assertRaisesRegex(ValueError, "invalid completed-output receipt"):
            generate.completed(self.output, plan, owner)

    def test_bad_report_not_marked_completed(self):
        plan = generate.prepare(self.args())
        owner = plan["owners"][0]
        job = generate.scheduler.Job(owner["id"], tuple(generate.command(self.output, plan, owner)), owner)
        self.fake_outputs(job, wrong_rank=True)
        with self.assertRaisesRegex(ValueError, "report differs"):
            generate.record_completed(self.output, plan, owner)
        self.assertFalse((self.output / "generation/parent-a/completed.json").exists())

    def test_invalid_coordinate_partition_and_path_refused(self):
        value = generate.read(self.manifest)
        value["entries"][0]["auxiliary_slots"] = [2]
        generate.write(self.manifest, value)
        with self.assertRaisesRegex(ValueError, "partition"):
            generate.prepare(self.args())
        value["entries"][0]["auxiliary_slots"] = [1]
        value["entries"][0]["input"] = "../native"
        generate.write(self.manifest, value)
        with self.assertRaisesRegex(ValueError, "within its directory"):
            generate.prepare(self.args())

    def test_retention_is_explicit_with_unlimited_counts(self):
        plan = generate.prepare(self.args(finite_case_policy="retain-rank-finite"))
        command = generate.command(self.output, plan, plan["owners"][0])
        self.assertEqual(command[command.index("--finite-max-visited-points")+1], str(generate.USIZE_MAX))

    def test_selected_representatives_default_and_full_downsets_override(self):
        value = generate.read(self.manifest)
        value["entries"][0]["selected_sectors"] = ["101", "100"]
        generate.write(self.manifest, value)
        plan = generate.prepare(self.args())
        command = generate.command(self.output, plan, plan["owners"][0])
        self.assertEqual(command[command.index("--selected-sectors")+1], "101,100")
        full_directory = self.base / "full"
        full = generate.prepare(self.args(directory=full_directory, full_downsets=True))
        self.assertNotIn("--selected-sectors", generate.command(full_directory, full, full["owners"][0]))
        value["entries"][0]["selected_sectors"] = ["111"]
        generate.write(self.manifest, value)
        with self.assertRaisesRegex(ValueError, "inside the physical root"):
            generate.manifest_entries(self.manifest)

    def test_changed_selected_scope_refuses_completed_report(self):
        plan = generate.prepare(self.args())
        owner = plan["owners"][0]
        job = generate.scheduler.Job(owner["id"], tuple(generate.command(self.output, plan, owner)), owner)
        self.fake_outputs(job)
        generate.record_completed(self.output, plan, owner)
        with self.assertRaisesRegex(ValueError, "report differs"):
            generate.completed(self.output, plan, {**owner, "selected_sectors": ["101"]})

    def test_selected_report_and_checkpoint_must_both_match(self):
        value = generate.read(self.manifest)
        value["entries"][0]["selected_sectors"] = ["101"]
        generate.write(self.manifest, value)
        plan = generate.prepare(self.args())
        owner = plan["owners"][0]
        job = generate.scheduler.Job(owner["id"], tuple(generate.command(self.output, plan, owner)), owner)
        self.fake_outputs(job)
        run = self.output / "generation/parent-a"
        report = run / "generation.toml"
        report.write_text(report.read_text().replace('generation_scope = "root-downset"',
            'generation_scope = "selected-sectors"\nselected_sectors = [[true, false, true]]'))
        checkpoint = run / "sectors/checkpoint.toml"
        checkpoint.write_text(checkpoint.read_text() + 'selected_sectors = [[true, false, true]]\n')
        generate.record_completed(self.output, plan, owner)
        self.assertEqual(generate.completed(self.output, plan, owner)["generated_rules"], 9)
        checkpoint.write_text(checkpoint.read_text().replace(
            'selected_sectors = [[true, false, true]]', 'selected_sectors = [[true, false, false]]'))
        with self.assertRaisesRegex(ValueError, "checkpoint provenance"):
            generate.record_completed(self.output, plan, owner)


if __name__ == "__main__":
    unittest.main()
