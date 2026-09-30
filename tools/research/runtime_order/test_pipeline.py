"""Metadata/guard integration only: synthetic payloads, no native IBPs."""
from contextlib import redirect_stderr, redirect_stdout
import copy
import io
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import types
import unittest
from unittest.mock import MagicMock, patch

import generation_guard as guard
import pipeline
import prepare_selected as prep
from test_stage import bits, write_toml


class PipelineTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=pipeline.ROOT / "TMP")
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.source = self.base / "original"
        self.source.mkdir()
        self.directory = self.base / "new"
        self.cli = self.base / "native"
        self.cli.write_text("#!/bin/sh\nexit 99\n")
        self.cli.chmod(0o700)
        self.family = self.base / "family.toml"
        self.family.write_text("# synthetic non-CAS input\n")
        self.queries = dict(schema="rustred.owner-domain-queries.json.v2", queries=[
            dict(id="physical", owner="10", lower=[0,0], upper=[None,None], max_numerator_rank=1, power_bounds={}),
            dict(id="auxiliary", owner="11", lower=[0,0], upper=[None,None], max_numerator_rank=None, power_bounds={})],
            query_roles=dict(required=["physical"], auxiliary=["auxiliary"]))
        prep.write(self.source / "queries.json", self.queries)
        self.selection = dict(family_fingerprint="synthetic-family", owner_count=2,
            owners=[dict(mask="10", parent=2, path="OLD-NOT-AVAILABLE", ordinal=9, bytes=5, sha256="old"),
                    dict(mask="11", saved_root=[True,True], path="OLD-NOT-AVAILABLE", ordinal=17)],
            initial_frontier_routes=[dict(source_mask="01", owner_mask="10", permutation=[1,0]),
                dict(source_mask="10", owner_mask="10", permutation=[0,1]),
                dict(source_mask="11", owner_mask="11", permutation=[0,1])],
            load_limits=dict(max_total_input_bytes=4096), receipts={"obsolete": True})
        prep.write(self.source / "selection.json", self.selection)
        self.recipe = dict(schema=prep.SCHEMA, source_selection_sha256=prep.sha(self.source / "selection.json"),
            family=dict(path="family.toml", sha256=prep.sha(self.family)),
            queries=dict(path="original/queries.json", sha256=prep.sha(self.source / "queries.json")),
            expected_counts=dict(owners=2, routes=3, required=1, auxiliary=1),
            missing_parent_assignments={"11":3},
            groups=[dict(parent=2, root="10", selected_sectors=["10"]),
                    dict(parent=3, root="11", selected_sectors=["11"])],
            discovery_strategy=None, integral_order="rustred.spired-uncut-sector-order.v1",
            exact_backend="sparse", solver_policy="synthetic-policy", generation_options=[],
            walk_options=["--publication-policy","epoch","--epoch-inspector-lookup","snapshot",
                "--epoch-rolling","--epoch-window","76","--epoch-cut-size","16",
                "--epoch-result-escrow-jobs","1024","--epoch-result-escrow-bytes","268435456",
                "--epoch-preparation-workers","0","--frontier-policy","stop","--no-auto-rescue"])
        prep.write(self.base / "recipe.json", self.recipe)
        cpu = min(os.sched_getaffinity(0))
        self.resources = dict(workers=1, cpus=str(cpu), max_memory_bytes=1_000_000_000,
            host_memory_reserve_bytes=500_000_000, ram_guard_margin_percent=5.0, checkpoint_max_bytes=100000)

    def prepare(self):
        return prep.prepare(self.base / "recipe.json", self.source / "selection.json", self.directory,
                            self.cli, self.cli, self.resources)

    def generated(self, command, attempt, resources):
        if "family-candidates" not in command:
            attempt.mkdir(parents=True)
            prep.write(attempt / "stdout", dict(status="NATIVE_OWNER_BINDING_ADMITTED", owner_count=2))
            return
        run = Path(command[command.index("--output")+1]).parent
        parent = int(run.name.split("-")[-1])
        group = next(g for g in self.recipe["groups"] if g["parent"] == parent)
        sectors = [bits(mask) for mask in sorted(group["selected_sectors"])]
        checkpoint = run / "sectors"
        checkpoint.mkdir(exist_ok=True)
        manifest = dict(version=4, recipe=pipeline.candidate_stage.CHECKPOINT_RECIPE,
            family_source=self.family.read_text(), input_format="toml", family_fingerprint="synthetic-family",
            root_sector=bits(group["root"]), integral_order=self.recipe["integral_order"],
            solver_policy="synthetic-policy", exact_backend="sparse", sectors=sectors, selected_sectors=sectors)
        report = dict(schema="rustred.family-candidates-output.toml.v1", status="uncertified-candidates",
            integral_order=self.recipe["integral_order"], exact_backend="sparse", family_fingerprint="synthetic-family",
            arity=2, root_sector=bits(group["root"]), solved_sectors=len(sectors),
            generation_scope="selected-sectors", selected_sectors=sectors)
        write_toml(checkpoint / "checkpoint.toml", manifest)
        write_toml(run / "generation.toml", report)
        (run / "candidates.rrbin").write_bytes(b"FAKE-NATIVE-BUNDLE")
        for number in range(len(sectors)):
            (checkpoint / f"sector-{number}.rrbin").write_bytes(b"FAKE-SECTOR")

    def test_preparation_scope_sanitation_and_no_payload_fallback(self):
        before = {path: prep.sha(path) for path in self.source.iterdir()}
        plan = self.prepare()
        selection = prep.read(self.directory / "shared/selection.json")
        self.assertEqual(selection["initial_frontier_routes"], self.selection["initial_frontier_routes"])
        self.assertEqual(selection["load_limits"], self.selection["load_limits"])
        self.assertNotIn("receipts", selection)
        for row in selection["owners"]:
            self.assertTrue(row["path"].startswith("UNGENERATED/"))
            self.assertNotIn("ordinal", row)
            self.assertNotIn("sha256", row)
        self.assertEqual(plan["counts"], self.recipe["expected_counts"])
        self.assertEqual((self.directory / "shared/queries.json").read_bytes(), (self.source / "queries.json").read_bytes())
        self.assertEqual({path: prep.sha(path) for path in self.source.iterdir()}, before)
        self.assertFalse((self.directory / "inputs").exists())
        pipeline.validate(self.directory, plan)

    def test_complete_mock_pipeline_resumes_without_replacing_generated_inputs(self):
        plan = self.prepare()
        with patch.object(pipeline, "guarded_run", side_effect=self.generated) as native, \
                patch.object(pipeline.os, "execv") as launch, redirect_stdout(io.StringIO()):
            pipeline.execute(self.directory, plan)
            self.assertEqual(native.call_count, 3)
            self.assertEqual(launch.call_count, 1)
            self.assertNotIn("--resume", launch.call_args.args[1])
            self.assertEqual(prep.read(self.directory / "inputs/selection.json")["initial_frontier_routes"],
                             self.selection["initial_frontier_routes"])
            self.assertEqual((self.directory / "inputs/queries.json").read_bytes(), (self.source / "queries.json").read_bytes())
            frozen = (self.directory / "bin/steering.json").read_bytes()
            checkpoint = self.directory / "checkpoints/main"
            checkpoint.mkdir(parents=True)
            (checkpoint / "latest.json").write_text("{}")
            pipeline.execute(self.directory, plan)
            self.assertEqual(native.call_count, 3)
            self.assertIn("--resume", launch.call_args.args[1])
            self.assertEqual((self.directory / "bin/steering.json").read_bytes(), frozen)

    def test_native_failure_stops_before_staging_and_uses_same_checkpoint_on_resume(self):
        plan = self.prepare()
        calls = []
        def fail(command, *unused):
            calls.append(command)
            run = Path(command[command.index("--checkpoint-dir")+1])
            run.mkdir(exist_ok=True)
            (run / "checkpoint.toml").write_text("# incomplete synthetic metadata")
            raise RuntimeError("bounded failure")
        with patch.object(pipeline, "guarded_run", side_effect=fail), patch.object(pipeline.os, "execv") as launch:
            for _ in range(2):
                with self.assertRaisesRegex(RuntimeError, "bounded failure"):
                    pipeline.execute(self.directory, plan)
            launch.assert_not_called()
        self.assertNotIn("--resume", calls[0])
        self.assertEqual(calls[1][-2:], ["--resume", "--force"])
        self.assertFalse((self.directory / "generated-inputs").exists())

    def test_missing_owner_changed_role_or_bad_parent_is_refused(self):
        for change in (lambda r: r["groups"].pop(), lambda r: r["expected_counts"].update(required=0),
                       lambda r: r["missing_parent_assignments"].update({"11":2}),
                       lambda r: r["generation_options"].extend(["--selected-sectors","10"])):
            bad = copy.deepcopy(self.recipe)
            change(bad)
            (self.base / "recipe.json").write_text(json.dumps(bad))
            with self.assertRaises((ValueError, KeyError)):
                self.prepare()
            self.assertFalse(self.directory.exists())

    def test_guard_has_no_timeout_and_reports_noncooperative_ram_stop(self):
        child = MagicMock(pid=123456789)
        child.poll.side_effect = [None, 0, 0]
        child.wait.return_value = -signal.SIGTERM
        collector = MagicMock()
        collector.sample.return_value = ({1: dict(rss_bytes=999_000_000, cpu_seconds=0.0, start=1, ppid=0)}, {})
        with patch.object(guard.subprocess, "Popen", return_value=child), \
                patch.object(guard, "group_running", return_value=False), \
                patch.object(guard.os, "killpg") as signal_group, \
                patch.object(guard.SUPERVISOR, "ProcessTreeCollector", return_value=collector), \
                patch.object(guard.SUPERVISOR, "host_memory", return_value={"available_bytes":10_000_000_000}), \
                patch.object(guard.SUPERVISOR, "tree_swap_bytes", return_value=0), \
                redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(RuntimeError, "unfinished sector must restart"):
                guard.run(["fake"], self.base / "guard", self.resources, poll_seconds=0)
        signal_group.assert_any_call(child.pid, signal.SIGTERM)
        result = prep.read(self.base / "guard/result.json")
        self.assertEqual(result["stop_reason"], "aggregate_rss_soft_limit")
        self.assertFalse(result["in_sector_resume"])
        self.assertIsNone(prep.read(self.base / "guard/request.json")["hard_timeout"])

    def test_walk_override_or_invalid_escrow_refused_before_generation(self):
        plan = self.prepare()
        for extra in (["--workers=200"], ["--max-memory-bytes", "999999999999"],
                      ["--start"], ["--manifest", "another"],
                      ["--epoch-result-escrow-jobs", "4096"]):
            altered = copy.deepcopy(plan)
            altered["recipe"]["walk_options"] += extra
            with patch.object(pipeline, "guarded_run") as native, redirect_stderr(io.StringIO()), self.assertRaises(ValueError):
                pipeline.execute(self.directory, altered)
            native.assert_not_called()

    def test_interrupted_staging_is_recoverable_without_regeneration(self):
        plan = self.prepare()
        def fail(_, output):
            output.mkdir()
            (output / "partial-owner").write_text("preserved evidence")
            raise OSError("simulated staging interruption")
        with patch.object(pipeline, "guarded_run", side_effect=self.generated) as native, \
                patch.object(pipeline.os, "execv"), redirect_stdout(io.StringIO()):
            with patch.object(pipeline.candidate_stage, "stage", side_effect=fail), self.assertRaises(OSError):
                pipeline.execute(self.directory, plan)
            self.assertEqual(native.call_count, 2)
            self.assertFalse((self.directory / "generated-inputs").exists())
            self.assertEqual(len(list((self.directory / "attempts").glob("*/partial-owner"))), 1)
            pipeline.execute(self.directory, plan)
            self.assertEqual(native.call_count, 3)  # Only native admission; completed generations reused.
            self.assertTrue((self.directory / "inputs/input-receipt.json").is_file())

    def test_resume_ram_override_is_ephemeral_and_forwarded_to_walk(self):
        plan = self.prepare()
        before = (self.directory / "pipeline.json").read_bytes()
        with patch.object(pipeline, "execute") as execute, redirect_stdout(io.StringIO()):
            pipeline.main(["--directory", str(self.directory), "--resume", "--start",
                           "--max-memory-bytes", "750000000000", "--ram-guard-margin-percent", "7"])
        updated = execute.call_args.args[1]
        self.assertEqual(updated["resources"]["max_memory_bytes"], 750_000_000_000)
        self.assertEqual(updated["resources"]["ram_guard_margin_percent"], 7)
        self.assertEqual((self.directory / "pipeline.json").read_bytes(), before)
        with patch.object(pipeline, "guarded_run", side_effect=self.generated), \
                patch.object(pipeline.os, "execv") as launch, redirect_stdout(io.StringIO()):
            pipeline.execute(self.directory, plan)
            checkpoint = self.directory / "checkpoints/main"
            checkpoint.mkdir(parents=True)
            (checkpoint / "latest.json").write_text("{}")
            pipeline.execute(self.directory, updated)
            command = launch.call_args.args[1]
            self.assertEqual(command[command.index("--max-memory-bytes")+1], "750000000000")
            self.assertIn("--resume", command)
            pipeline.execute(self.directory, plan)
            command = launch.call_args.args[1]
            self.assertEqual(command[command.index("--max-memory-bytes")+1], str(self.resources["max_memory_bytes"]))

    def test_supervisor_exception_is_recorded_without_starting_orphan_work(self):
        with patch.object(guard.subprocess, "Popen", side_effect=OSError("synthetic exec failure")), \
                patch.object(guard.SUPERVISOR, "host_memory", return_value={"available_bytes":10_000_000_000}), \
                self.assertRaisesRegex(RuntimeError, "supervisor failed"):
            guard.run(["missing"], self.base / "failed-guard", self.resources)
        result = prep.read(self.base / "failed-guard/result.json")
        self.assertFalse(result["child_started"])
        self.assertTrue(result["owned_group_drained"])
        self.assertIn("synthetic exec failure", result["stop_reason"])

    def test_collector_registration_failure_restores_signal_handlers(self):
        previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
        collector = MagicMock()
        collector.register.side_effect = OSError("synthetic proc registration failure")
        with patch.object(guard.SUPERVISOR, "ProcessTreeCollector", return_value=collector), \
                patch.object(guard.SUPERVISOR, "host_memory", return_value={"available_bytes":10_000_000_000}), \
                patch.object(guard.subprocess, "Popen") as native, self.assertRaises(RuntimeError):
            guard.run(["unused"], self.base / "failed-registration", self.resources)
        native.assert_not_called()
        self.assertEqual(previous, {sig: signal.getsignal(sig) for sig in previous})
        result = prep.read(self.base / "failed-registration/result.json")
        self.assertFalse(result["child_started"])
        self.assertTrue(result["owned_group_drained"])
        self.assertIn("synthetic proc registration failure", result["stop_reason"])

    def test_parallel_generation_freezes_inner_budget_and_keeps_staging_scope(self):
        self.resources.update(workers=4, cpus="64-67", generation_jobs=2)
        with patch.object(pipeline.os, "sched_getaffinity", return_value={64,65,66,67}):
            plan = self.prepare()
            self.assertEqual(plan["generation_schedule"]["slots"], [[64,65],[66,67]])
            for group in self.recipe["groups"]:
                command = prep.read(self.directory / f"commands/parent-{group['parent']}.json")
                self.assertEqual(command[command.index("--n-cores")+1], "2")
                self.assertIn("--progress-json", command)
            def generate(jobs, attempt, resources, completed, **unused):
                # Deliberately reverse completion order; stage remains in the
                # original frozen owner/query/routing order.
                for job in reversed(jobs):
                    self.generated(list(job.command), attempt / job.id, resources)
                    completed(job)
            with patch.object(pipeline.generation_scheduler, "run", side_effect=generate) as parallel, \
                    patch.object(pipeline.subprocess, "run", return_value=types.SimpleNamespace(
                        returncode=0, stdout="--progress-json PATH", stderr="")), \
                    patch.object(pipeline, "guarded_run", side_effect=self.generated) as admit, \
                    patch.object(pipeline.os, "execv"), redirect_stdout(io.StringIO()):
                pipeline.execute(self.directory, plan)
                self.assertEqual(parallel.call_count, 1)
                self.assertEqual(admit.call_count, 1)
                self.assertEqual(prep.read(self.directory / "inputs/selection.json")["initial_frontier_routes"],
                                 self.selection["initial_frontier_routes"])
                self.assertEqual((self.directory / "inputs/queries.json").read_bytes(),
                                 (self.source / "queries.json").read_bytes())
                pipeline.execute(self.directory, plan)
                self.assertEqual(parallel.call_count, 1)
                self.assertEqual(admit.call_count, 1)

    def test_parallel_generation_requires_capability_and_never_stages_after_failure(self):
        self.resources.update(workers=4, cpus="64-67", generation_jobs=2)
        with patch.object(pipeline.os, "sched_getaffinity", return_value={64,65,66,67}):
            plan = self.prepare()
            with patch.object(pipeline.generation_scheduler, "run") as parallel, \
                    patch.object(pipeline.subprocess, "run", return_value=types.SimpleNamespace(
                        returncode=0, stdout="old engine", stderr="")), \
                    patch.object(pipeline.candidate_stage, "stage") as stage, \
                    self.assertRaisesRegex(ValueError, "advertising --progress-json"):
                pipeline.execute(self.directory, plan)
            parallel.assert_not_called()
            stage.assert_not_called()
            with patch.object(pipeline.generation_scheduler, "run", side_effect=RuntimeError("siblings drained")), \
                    patch.object(pipeline.subprocess, "run", return_value=types.SimpleNamespace(
                        returncode=0, stdout="--progress-json PATH", stderr="")), \
                    patch.object(pipeline.candidate_stage, "stage") as stage, self.assertRaises(RuntimeError):
                pipeline.execute(self.directory, plan)
            stage.assert_not_called()

    def test_parallel_generation_schedule_cannot_change_on_resume(self):
        plan = self.prepare()
        plan["resources"]["generation_jobs"] = 2
        with self.assertRaises(ValueError):
            pipeline.validate(self.directory, plan)
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            pipeline.main(["--directory", str(self.directory), "--resume", "--generation-jobs", "2"])


if __name__ == "__main__":
    unittest.main()
