"""Steering-only fixtures; fake bytes/status are not native closure evidence."""
from contextlib import contextmanager, redirect_stdout
import copy
import io
import json
import os
from pathlib import Path
import tempfile
import signal
import sys
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import finite_saved_campaign as FIN
import extend_rank_campaign as EXTEND
import prepare_rank_campaign as PREPARE
import test_prepare_rank_campaign as FIXTURE
import saved_campaign as PUBLIC


class FiniteCampaignTests(unittest.TestCase):
    def setUp(self):
        PREPARE.WORKSPACE_TEMP.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=PREPARE.WORKSPACE_TEMP)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        source = FIXTURE.RankCampaignStagingTests().fixture(self.root)
        self.campaign = self.root / "finite"
        PREPARE.prepare(source, self.campaign, 0, max_power_difference=4)
        self.binary = self.root / "rustred"
        self.binary.write_text("#!/bin/sh\nexit 99\n")
        self.binary.chmod(0o700)
        self.policy = {"workers": 1, "windows": {"max_seed_points": 2, "max_seed_bytes": 65536,
            "max_positive_layers": 64, "max_partition_checks": 1000}, "census": {}, "export": {},
            "storage": {}, "max_total_rows": 100, "max_source_bytes": 1048576,
            "control": {"max_bytes": 1048576, "max_files": 100}}
        self.policy_path = self.root / "policy.json"
        self.policy_path.write_bytes(FIN.encoded(self.policy))
        self.args = SimpleNamespace(action="run", campaign=self.campaign, executable=self.binary,
            engine="finite-demand", finite_policy=self.policy_path, cpus=str(min(os.sched_getaffinity(0))),
            max_memory_bytes=32_000_000_000, host_memory_reserve_bytes=0, ram_guard_margin_percent=5,
            stop_grace_seconds=120, resume=False, dry_run=False, deep_verification=None)
        self.native = {"schema": "rustred.finite-demand-status.v1", "engine": FIN.ENGINE,
            "generation": {"number": 1, "digest": "b"*64},
            "committed": {"rows": 2, "required_rows": 2, "auxiliary_rows": 0,
                "required_complete": False, "all_rows_exhausted": False},
            "source": {"status": "bound", "request": "a"*64, "total_queries": 2, "amendments": []}}

    def plan(self, apply=True):
        return FIN.launch_plan(self.args, self.campaign, self.binary, apply=apply)

    def namespace(self):
        path = self.campaign / "finite-demand"
        path.mkdir(exist_ok=True)
        (path / "control.lock").touch()
        self.bind_sources(path)
        return path

    def bind_sources(self, path):
        (path / "sources").mkdir(exist_ok=True)
        originals = [("selection", self.campaign / "inputs/selection.json"),
                     ("queries", self.campaign / "inputs/queries.json")]
        originals += [("amendment", item) for item in sorted((self.campaign / "amendments").glob("amendment-*.json"))
                      ][:len(self.native["source"]["amendments"])]
        files = []
        for kind, original in originals:
            data = original.read_bytes()
            # Fake native-inspect transport only; no hash authority is inferred.
            name = f"sources/{kind}-{FIN.digest(data)}.json"
            (path / name).write_bytes(data)
            files.append({"path": name, "bytes": len(data), "blake3": FIN.digest(data)})
        self.native["source"]["files"] = files

    def test_engine_marker_prevents_legacy_fallback_or_import(self):
        self.assertIsNone(FIN.engine(self.campaign))
        self.assertEqual(FIN.engine(self.campaign, "finite-demand"), FIN.ENGINE)
        path = self.campaign / "bin/steering.json"
        path.parent.mkdir()
        path.write_text('{}')
        with self.assertRaisesRegex(ValueError, "fresh"):
            FIN.engine(self.campaign, "finite-demand")
        path.write_text(json.dumps({"engine": FIN.ENGINE}))
        with self.assertRaisesRegex(ValueError, "legacy engine"):
            FIN.engine(self.campaign, "legacy")
        (self.campaign / "checkpoints/main").mkdir(parents=True)
        with self.assertRaisesRegex(ValueError, "does not import"):
            FIN.engine(self.campaign)

    def test_dry_plan_never_freezes_or_creates_namespace(self):
        before = FIXTURE.snapshot(self.campaign)
        plan = self.plan(apply=False)
        self.assertEqual(FIXTURE.snapshot(self.campaign), before)
        self.assertFalse((self.campaign / "finite-demand").exists())
        self.assertIn("finite-demand", plan["native_command"])
        self.assertNotIn("--resume", plan["native_command"])
        self.assertNotIn("checkpoint_manifest_sha256", plan)
        for value in (0, float("nan"), float("inf")):
            self.args.stop_grace_seconds = value
            with self.assertRaisesRegex(ValueError, "finite and positive"):
                self.plan(apply=False)
        self.assertEqual(FIXTURE.snapshot(self.campaign), before)

    def test_finite_default_honors_requested_ram_even_when_host_available_is_lower(self):
        self.args.max_memory_bytes = 600_000_000_000
        self.args.host_memory_reserve_bytes = 150_000_000_000
        with patch.object(FIN.SUPERVISOR, "host_memory", return_value={
                "available_bytes": 100_000_000_000, "host_available_bytes": 100_000_000_000}):
            plan = self.plan(apply=False)
            self.assertEqual(plan["supervisor_ram_policy"]["memory_policy"], "process-limit")
            self.assertEqual(plan["supervisor_ram_policy"]["max_memory_bytes"], 600_000_000_000)
            self.args.memory_policy = "host-aware"
            with self.assertRaisesRegex(ValueError, "headroom"):
                self.plan(apply=False)

    def test_saved_memory_mode_and_budget_survive_resume_and_postprocess_launches(self):
        host = {"available_bytes": 1_000_000_000_000, "host_available_bytes": 1_000_000_000_000}
        with patch.object(FIN.SUPERVISOR, "host_memory", return_value=host):
            for mode in ("process-limit", "host-aware"):
                with self.subTest(mode=mode):
                    self.args.action = "run"
                    self.args.memory_policy = mode
                    self.args.cpus = str(min(os.sched_getaffinity(0)))
                    self.args.finite_policy = self.policy_path
                    self.args.max_memory_bytes = 600_000_000_000
                    self.args.host_memory_reserve_bytes = 150_000_000_000
                    self.args.ram_guard_margin_percent = 5
                    first = self.plan()
                    self.namespace()
                    saved = FIN.read(self.campaign / "bin/steering.json")["resources"]
                    self.assertEqual(saved, first["supervisor_ram_policy"])
                    for name in ("memory_policy", "cpus", "finite_policy", "max_memory_bytes",
                                 "host_memory_reserve_bytes", "ram_guard_margin_percent", "stop_grace_seconds"):
                        setattr(self.args, name, None)
                    self.args.resume = True
                    resumed = self.plan()
                    self.assertIn("--resume", resumed["native_command"])
                    self.assertEqual(resumed["supervisor_ram_policy"], saved)
                    for action in ("extend", "publish", "refine", "finalize-refinement"):
                        self.args.action = action
                        plan = self.plan(apply=False)
                        self.assertEqual(plan["supervisor_ram_policy"], saved)
                        self.assertEqual(plan["supervisor_ram_policy"]["memory_policy"], mode)
                        self.assertEqual(plan["supervisor_ram_policy"]["max_memory_bytes"], 600_000_000_000)
                        self.assertEqual(plan["supervisor_ram_policy"]["swap_growth_stop_bytes_per_second"], 0)

    def test_optional_resident_policy_is_explicit_nonnegative_and_not_a_scope_change(self):
        self.assertNotIn("max_resident_work_bytes", FIN.require_policy(self.policy))
        windows = copy.deepcopy(self.policy["windows"])
        for grant in (0, 1, 300_000_000_000):
            value = {**self.policy, "max_resident_work_bytes": grant}
            self.assertEqual(FIN.require_policy(value)["max_resident_work_bytes"], grant)
            self.assertEqual(value["windows"], windows)
        for grant in (-1, True, 1.5, 2**64):
            with self.assertRaisesRegex(ValueError, "resident Work allowance"):
                FIN.require_policy({**self.policy, "max_resident_work_bytes": grant})

    def test_worker_override_is_saved_without_mutating_input_or_window_policy(self):
        original = self.policy_path.read_bytes()
        self.args.workers = 150
        self.args.cpus = "0-149"
        with patch.object(FIN.os, "sched_getaffinity", return_value=set(range(150))):
            plan = self.plan()
            self.assertEqual(plan["requested_workers"], 150)
            self.assertEqual(plan["supervisor_ram_policy"]["workers"], 150)
            frozen = FIN.read(plan["finite_policy"])
            self.assertEqual(frozen, {**self.policy, "workers": 150})
            self.assertEqual(self.policy_path.read_bytes(), original)
            self.namespace()
            self.args.workers = self.args.cpus = self.args.finite_policy = None
            self.args.resume = True
            resumed = self.plan(apply=False)
            self.assertEqual(resumed["requested_workers"], 150)
            self.assertEqual(resumed["cpus"], "0-149")
            self.assertEqual(FIN.read(resumed["finite_policy"]), frozen)

    def test_worker_override_keeps_positive_and_affinity_checks(self):
        before = FIXTURE.snapshot(self.campaign)
        for bad in (0, -1, True, 1.5):
            self.args.workers = bad
            with self.assertRaisesRegex(ValueError, "positive workers"):
                self.plan(apply=False)
        self.args.workers = 150
        self.args.cpus = "0-148"
        with patch.object(FIN.os, "sched_getaffinity", return_value=set(range(150))):
            with self.assertRaisesRegex(ValueError, "reserved permitted CPU count"):
                self.plan(apply=False)
        self.assertEqual(FIXTURE.snapshot(self.campaign), before)

    def test_uncapped_operational_policy_is_frozen_without_changing_windows(self):
        self.plan()
        self.namespace()
        modified = copy.deepcopy(self.policy)
        modified.update(max_total_rows=None, max_source_bytes=None)
        modified["storage"] = {"max_runs": None, "max_pages": None}
        modified["control"] = {"max_bytes": None, "max_files": None}
        self.policy_path.write_bytes(FIN.encoded(modified))
        plan = self.plan()
        frozen = FIN.read(plan["finite_policy"])
        self.assertEqual(frozen, modified)
        self.assertEqual(frozen["windows"], self.policy["windows"])
        self.assertIn("--resume", plan["native_command"])

    def test_inspection_forwards_explicit_policy_and_leaves_saved_policy_unchanged(self):
        self.plan()
        override = self.root / "override.json"
        override.write_bytes(FIN.encoded({**self.policy, "max_total_rows": None}))
        before = FIXTURE.snapshot(self.campaign)
        response = SimpleNamespace(returncode=0, stdout=json.dumps(self.native), stderr="")
        with redirect_stdout(io.StringIO()), patch.object(FIN.subprocess, "run", return_value=response) as native:
            self.assertEqual(PUBLIC.main(["inspect", "--campaign", str(self.campaign),
                "--executable", str(self.binary), "--format", "json",
                "--finite-policy", str(override)]), 0)
        command = native.call_args.args[0]
        self.assertEqual(command[command.index("--policy") + 1], str(override))
        self.assertEqual(FIXTURE.snapshot(self.campaign), before)
        with self.assertRaises(SystemExit) as error:
            PUBLIC.main(["inspect", "--campaign", str(self.campaign),
                "--executable", str(self.binary), "--cpus", "0"])
        self.assertEqual(error.exception.code, 2)

    def test_steering_json_allowance_is_caller_selected(self):
        path = self.root / "large-metadata.json"
        path.write_text('{"padding":"' + "x" * (16 * 1024 * 1024) + '"}')
        self.assertEqual(len(FIN.read(path)["padding"]), 16 * 1024 * 1024)
        self.assertEqual(len(FIN.PHASES.read_json(path)["padding"]), 16 * 1024 * 1024)
        for reader in (FIN.read, FIN.PHASES.read_json):
            with self.assertRaisesRegex(ValueError, "allowance"):
                reader(path, max_bytes=100)
            for bad in (-1, True):
                with self.assertRaisesRegex(ValueError, "allowance"):
                    reader(path, max_bytes=bad)

    def test_public_finite_dry_run_dispatch_is_read_only_and_never_legacy(self):
        before = FIXTURE.snapshot(self.campaign)
        command = ["run", "--campaign", str(self.campaign), "--executable", str(self.binary),
            "--engine", "finite-demand", "--finite-policy", str(self.policy_path),
            "--cpus", self.args.cpus, "--max-memory-bytes", str(self.args.max_memory_bytes),
            "--host-memory-reserve-bytes", "0", "--dry-run"]
        with redirect_stdout(io.StringIO()), \
             patch.object(PUBLIC, "native_executable", side_effect=AssertionError("legacy preflight")), \
             patch.object(PUBLIC.os, "execv", side_effect=AssertionError("legacy launch")):
            self.assertEqual(PUBLIC.main(command), 0)
        self.assertEqual(FIXTURE.snapshot(self.campaign), before)

    def test_public_inspect_uses_persisted_engine_not_artifact_inspect(self):
        self.plan()
        response = SimpleNamespace(returncode=0, stdout=json.dumps(self.native), stderr="")
        with redirect_stdout(io.StringIO()), patch.object(FIN.subprocess, "run", return_value=response) as native, \
             patch.object(PUBLIC, "native_executable", side_effect=AssertionError("legacy preflight")):
            self.assertEqual(PUBLIC.main(["inspect", "--campaign", str(self.campaign),
                "--executable", str(self.binary), "--format", "json"]), 0)
        self.assertEqual(native.call_args.args[0][1:3], ["finite-demand", "inspect"])

    def test_source_carrier_and_policy_are_immutable_and_scope_ordered(self):
        plan = self.plan()
        source = FIN.read(plan["source_command"])["command"]
        self.assertEqual(source[:2], ["rustred", "owner-domain-match"])
        self.assertEqual(source[source.index("--manifest")+1], "../inputs/selection.json")
        self.assertIn("--checkpoint", source)
        self.assertFalse((self.campaign / "finite-source-unused").exists())
        self.assertFalse((self.campaign / "finite-demand").exists())
        self.namespace()
        resumed = self.plan()
        self.assertIn("--resume", resumed["native_command"])
        self.assertEqual(plan["source_command"], resumed["source_command"])
        changed = copy.deepcopy(self.policy)
        changed["windows"]["max_seed_points"] += 1
        self.policy_path.write_bytes(FIN.encoded(changed))
        before = FIXTURE.snapshot(self.campaign)
        with self.assertRaisesRegex(ValueError, "cannot change"):
            self.plan()
        self.assertEqual(FIXTURE.snapshot(self.campaign), before)

    def test_public_inspect_adds_native_publication_and_never_legacy_fallback(self):
        self.plan()
        output = io.StringIO()
        with redirect_stdout(output), patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.subprocess, "run") as command:
            FIN.public_inspect(self.campaign, self.binary, "json")
        command.assert_not_called()
        pending = json.loads(output.getvalue())
        self.assertIsNone(pending["publication"])
        self.assertEqual(pending["publication_status"], "pending")
        artifacts = self.campaign / "artifacts"
        artifacts.mkdir()
        (artifacts / "latest.json").write_text('{}')
        published = {"published_rule_records": 9982, "observed_cover_rules": 2165,
                     "remaining_masters": 62, "published_scope_current": False,
                     "scope_selection": "earlier publication; new request not published"}
        response = SimpleNamespace(returncode=0, stdout=json.dumps(published), stderr="")
        output = io.StringIO()
        with redirect_stdout(output), patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.PHASES, "completed_artifact", return_value={}), \
             patch.object(FIN.subprocess, "run", return_value=response) as command:
            FIN.public_inspect(self.campaign, self.binary, "json")
        report = json.loads(output.getvalue())
        self.assertEqual(report["publication"], published)
        self.assertFalse(report["publication"]["published_scope_current"])
        self.assertEqual(report["committed"], self.native["committed"])
        self.assertEqual(command.call_args.args[0][1:3], ["artifact-inspect", "--campaign-directory"])
        with patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.PHASES, "completed_artifact", side_effect=ValueError("corrupt pointer")), \
             patch.object(FIN.subprocess, "run") as command:
            with self.assertRaisesRegex(ValueError, "corrupt pointer"):
                FIN.public_inspect(self.campaign, self.binary, "json")
        command.assert_not_called()

    def test_public_inspect_table_inherits_native_tty_and_errors_propagate(self):
        self.plan()
        (self.campaign / "artifacts").mkdir()
        (self.campaign / "artifacts/latest.json").write_text('{}')
        with patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.PHASES, "completed_artifact", return_value={}), \
             patch.object(FIN.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as command:
            FIN.public_inspect(self.campaign, self.binary, "table")
        self.assertEqual(len(command.call_args_list), 2)
        self.assertTrue(all(not call.kwargs for call in command.call_args_list))
        self.assertEqual(command.call_args.args[0][-2:], ["--format", "table"])
        with patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.PHASES, "completed_artifact", return_value={}), \
             patch.object(FIN.sys.stdout, "isatty", return_value=True), \
             patch.object(FIN.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as command:
            FIN.public_inspect(self.campaign, self.binary, "auto")
        self.assertEqual(len(command.call_args_list), 2)
        self.assertTrue(all(not call.kwargs for call in command.call_args_list))
        output = io.StringIO()
        with redirect_stdout(output), patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.PHASES, "completed_artifact", return_value={}), \
             patch.object(FIN.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout='{}', stderr='')) as command:
            FIN.public_inspect(self.campaign, self.binary, "auto")
        self.assertEqual(len(command.call_args_list), 1)
        self.assertTrue(command.call_args.kwargs["capture_output"])
        self.assertEqual(json.loads(output.getvalue())["publication"], {})
        with patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.PHASES, "completed_artifact", return_value={}), \
             patch.object(FIN.subprocess, "run", return_value=SimpleNamespace(returncode=4, stderr="bad artifact")):
            with self.assertRaisesRegex(ValueError, "bad artifact"):
                FIN.public_inspect(self.campaign, self.binary, "json")

    def test_auxiliary_and_unknown_policy_refuse_before_freeze(self):
        source = self.root / "source"
        # Reuse the original staged-source fixture, including its helper.
        (self.root / "aux-fixture").mkdir()
        staged = FIXTURE.RankCampaignStagingTests().fixture(self.root / "aux-fixture")
        PREPARE.prepare(staged, source, 0, include_auxiliary=True)
        args = copy.copy(self.args)
        args.campaign = source
        with patch.object(FIN.PRODUCTION, "copy_executable") as freeze:
            with self.assertRaisesRegex(ValueError, "auxiliary"):
                FIN.launch_plan(args, source, self.binary, apply=True)
            freeze.assert_not_called()
        with self.assertRaisesRegex(ValueError, "diagnostic"):
            FIN.require_policy({**self.policy, "mode": "shared_windows"})
        FIN.require_policy(self.policy)  # Native's omitted aggregate defaults remain uncapped.

    def test_inspect_failure_is_not_missing_or_fresh(self):
        result = SimpleNamespace(returncode=4, stdout="", stderr="corrupt native CURRENT")
        with patch.object(FIN.subprocess, "run", return_value=result), self.assertRaisesRegex(ValueError, "inspect failed"):
            FIN.inspect(self.campaign, self.binary)

    def test_native_scope_and_pending_amendment_stay_separate(self):
        plan = self.plan()
        self.namespace()
        self.native["committed"].update(required_complete=True, all_rows_exhausted=True)
        with patch.object(FIN, "inspect", return_value=self.native):
            binding = FIN.finite_scope_binding(plan)
            self.assertEqual(FIN.finite_completed_request(plan, binding), Path(plan["source_command"]))
            amendments = self.campaign / "amendments"
            amendments.mkdir()
            (amendments / "amendment-0001.json").write_text("{}")
            pending = FIN.finite_scope_binding(plan)
            self.assertNotEqual(binding["key"], pending["key"])
            self.assertIsNone(FIN.finite_completed_request(plan, pending))
            self.assertFalse(FIN.PHASES.same_input_scope(binding, pending))
            self.assertFalse(FIN.PHASES.same_input_scope(binding, {"components": {}}))

    def test_shared_stage_builder_is_exact_and_pending_is_idempotent(self):
        self.plan()
        self.namespace()
        before = FIXTURE.snapshot(self.campaign)
        with patch.object(FIN, "inspect", return_value=self.native):
            preview = FIN.extension(self.campaign, self.binary, 1, EXTEND.PRESERVE_DIFFERENCE, True)
            self.assertEqual(FIXTURE.snapshot(self.campaign), before)
            first = FIN.extension(self.campaign, self.binary, 1, EXTEND.PRESERVE_DIFFERENCE, False)
            again = FIN.extension(self.campaign, self.binary, 1, EXTEND.PRESERVE_DIFFERENCE, False)
        self.assertEqual(first, again)
        self.assertEqual(first["amendment_sha256"], preview["amendment_sha256"])
        source = (self.campaign / "inputs" / PREPARE.SOURCE_QUERIES_NAME).read_bytes()
        expected = EXTEND.stage_document(source, 1, 1, "a"*64,
            FIN.PHASES.digest(self.campaign / "inputs/selection.json"), 0, 4, 4)
        self.assertEqual(FIN.read(first["amendment"]), expected)
        self.assertNotIn("original_root_prefix", first)
        self.assertFalse(first["closure_claim"])
        self.assertEqual({name: FIXTURE.snapshot(self.campaign)[name] for name in before}, before)
        with patch.object(FIN, "inspect", return_value=self.native), self.assertRaisesRegex(ValueError, "pending"):
            FIN.extension(self.campaign, self.binary, 2, EXTEND.PRESERVE_DIFFERENCE, False)

    def test_extension_uses_new_policy_before_inspecting_the_old_scope(self):
        self.plan()
        self.namespace()
        before = FIXTURE.snapshot(self.campaign)
        override = self.root / "expanded-policy.json"
        override.write_bytes(FIN.encoded({**self.policy, "max_total_rows": None}))
        with patch.object(FIN, "inspect", return_value=self.native) as inspect:
            FIN.extension(self.campaign, self.binary, 1, EXTEND.PRESERVE_DIFFERENCE,
                          True, policy_path=override)
        self.assertEqual(inspect.call_count, 2)
        self.assertTrue(all(call.kwargs["policy_path"] == override or
                            call.kwargs["policy_path"] == str(override)
                            for call in inspect.call_args_list))
        self.assertEqual(FIXTURE.snapshot(self.campaign), before)

    def test_finite_lock_order_and_no_legacy_checkpoint_calls(self):
        plan = self.plan()
        self.namespace()
        order = []
        @contextmanager
        def dispatcher(path):
            path.mkdir(parents=True, exist_ok=True)
            order.append("dispatcher+")
            yield
            order.append("dispatcher-")
        @contextmanager
        def control(_):
            order.append("control+")
            yield
            order.append("control-")
        policy = FIN.PHASES.configuration(self.campaign)
        def stop(_):
            self.assertEqual(order, ["dispatcher+", "control+", "control-"])
            return 8, False
        with patch.object(FIN.PHASES, "dispatcher_lock", dispatcher), patch.object(FIN, "control_lock", control), \
             patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.PRODUCTION, "checkpoint_lock", side_effect=AssertionError("CP6 touched")), \
             patch.object(FIN.PHASES, "phase_one", side_effect=stop), patch.object(FIN.PHASES, "phase_two") as publish:
            self.assertEqual(FIN.PHASES.run(plan, policy, True, FIN), 8)
            publish.assert_not_called()
        self.assertEqual(order[-1], "dispatcher-")
        self.assertFalse((self.campaign / "artifacts/latest.json").exists())

    def test_append_publication_reuses_exact_retained_refinement(self):
        plan = self.plan()
        self.namespace()
        self.native["committed"].update(required_complete=True, all_rows_exhausted=True)
        previous = {"operation": "refine", "scope_binding": {"components": {"engine": FIN.ENGINE,
            "generation_digest": "old"}}, "resolved_directory": self.campaign / "master-reduction/scopes/old",
            "refinement": {"proved_maps": "native-owned"}}
        with patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.PHASES, "completed_artifact", return_value=previous), \
             patch.object(FIN.PHASES, "phase_one", side_effect=AssertionError("unexpected work")), \
             patch.object(FIN.PHASES, "phase_two", return_value=0) as publish:
            self.assertEqual(FIN.PHASES.run(plan, FIN.PHASES.configuration(self.campaign), True, FIN), 0)
        effective = publish.call_args.args[1]
        self.assertEqual(effective["previous_artifact"], str(previous["resolved_directory"]))
        self.assertEqual(effective["preserved_refinement"], previous["refinement"])

    def test_missing_source_only_allows_explicit_run_repair_not_freshness(self):
        plan = self.plan()
        self.namespace()
        stale = copy.deepcopy(self.native)
        stale["source"] = {"status": "stale"}
        with patch.object(FIN, "inspect", return_value=stale):
            self.assertIsNone(FIN.finite_scope_binding(plan))
            plan["allow_unbound_resume"] = False
            with self.assertRaisesRegex(ValueError, "resupply"):
                FIN.finite_scope_binding(plan)
        self.assertIn("--resume", self.plan()["native_command"])
        preparing = {"status": "preparing", "generation": None, "committed": None,
                     "source": {"status": "unadmitted"}, "repair_preparation_possible": True}
        with patch.object(FIN, "inspect", return_value=preparing):
            with self.assertRaisesRegex(ValueError, "--repair-preparation"):
                FIN.finite_scope_binding(plan)

    def test_guarded_native_exit_needs_typed_completion_and_never_infers_it(self):
        plan = self.plan()
        run = Path(plan["run_directory"])
        def native_guard(command, directory, resources, **options):
            self.assertFalse(any(t.name == "finite-startup-progress" for t in threading.enumerate()))
            self.assertEqual(command, plan["native_command"])
            self.assertEqual(options["cooperative_stop_file"], run / "stop-request.json")
            self.assertIs(options["native_tty_progress"], True)
            directory.mkdir()
            FIN.write_json(directory / "result.json", {"owned_group_drained": True,
                "stop_reason": None, "exit_code": 0})
            FIN.write_json(run / "native-result.json", {"schema": "rustred.finite-demand-run.v1",
                "engine": FIN.ENGINE, "complete": False})
        module = SimpleNamespace(run=native_guard)
        spec = SimpleNamespace(loader=SimpleNamespace(exec_module=lambda _: None))
        with patch.object(FIN.importlib.util, "spec_from_file_location", return_value=spec), \
             patch.object(FIN.importlib.util, "module_from_spec", return_value=module):
            with self.assertRaisesRegex(ValueError, "lacks a complete"):
                FIN.execute(run / "launch.json")
        self.assertFalse((self.campaign / "artifacts/latest.json").exists())

    def test_same_count_modified_amendment_refuses_before_any_planning_write(self):
        plan = self.plan()
        path = self.campaign / "amendments/amendment-0001.json"
        path.parent.mkdir()
        path.write_text('{"queries": []}\n')
        self.native["source"]["amendments"] = [{"sequence": 1}]
        self.namespace()
        with patch.object(FIN, "inspect", return_value=self.native):
            FIN.finite_scope_binding(plan)
            path.write_text('{"queries":{}} \n')
            before = FIXTURE.snapshot(self.campaign)
            with self.assertRaisesRegex(ValueError, "source bytes differ"):
                FIN.finite_scope_binding(plan)
            self.assertEqual(FIXTURE.snapshot(self.campaign), before)

    def test_dispatcher_and_nested_phase_symlinks_never_write_outside_campaign(self):
        directory = self.campaign / "master-reduction"
        directory.mkdir()
        external = self.root / "outside-lock"
        (directory / "dispatcher.lock").symlink_to(external)
        with self.assertRaises(OSError), FIN.PHASES.dispatcher_lock(directory):
            pass
        self.assertFalse(external.exists())
        outside = self.root / "outside"
        outside.mkdir()
        for name in ("scopes", "bin"):
            (directory / name).symlink_to(outside, target_is_directory=True)
            plan = {"engine": FIN.ENGINE, "campaign_directory": str(self.campaign)}
            with self.assertRaisesRegex(ValueError, "symlink"):
                FIN.PHASES.finite_output_path(plan, directory / name / "new" / "output.json")
        self.assertEqual(list(outside.iterdir()), [])

    def test_stop_racing_native_publication_exit0_never_promotes_pointers(self):
        plan = self.plan()
        binary = self.root / "fake-publisher"
        binary.write_text(f"#!{sys.executable}\n" + '''import json, pathlib, sys, time
args=sys.argv
assert '--finite-directory' in args and '--checkpoint' not in args
directory=pathlib.Path(args[args.index('--directory')+1])
stop=pathlib.Path(args[args.index('--stop-file')+1])
while not stop.exists(): time.sleep(.01)
(directory/'artifact.json').write_text(json.dumps({'status':'published_unrefined'}))
''')
        binary.chmod(0o700)
        plan["steering_policy"] = {"command_arguments": ["--executable", str(binary)]}
        parent = self.campaign / "master-reduction"
        parent.mkdir()
        artifacts = self.campaign / "artifacts"
        artifacts.mkdir()
        old_artifact = artifacts / "latest.json"
        old_phase = parent / "completed-phase.json"
        old_artifact.write_text('{"retained":"old-refinement"}\n')
        old_phase.write_text('{"directory":"old","scope_binding":{"components":{}}}\n')
        before = (old_artifact.read_bytes(), old_phase.read_bytes())
        target = parent / "scopes/new"
        target.mkdir(parents=True)
        binding = {"key": "new", "components": {"selection_sha256": "new"}}
        timer = threading.Timer(.3, lambda: os.kill(os.getpid(), signal.SIGINT))
        with redirect_stdout(io.StringIO()), patch.object(FIN.PHASES.sys, "stderr", io.StringIO()), \
             patch.object(FIN.PHASES, "publish_pointer", side_effect=AssertionError("pointer promotion")):
            timer.start()
            try:
                status = FIN.PHASES.phase_two(plan, FIN.PHASES.configuration(self.campaign),
                    Path(plan["source_command"]), binding, target, FIN)
            finally:
                timer.cancel()
                timer.join()
        self.assertEqual(status, 8)
        self.assertTrue((target / "artifact.json").is_file())
        self.assertEqual((old_artifact.read_bytes(), old_phase.read_bytes()), before)
        receipt = FIN.read(next((target / "runs").glob('*/supervisor-result.json')))
        self.assertEqual(receipt["native_exit_status"], 0)
        self.assertEqual(receipt["exit_status"], 8)

    def test_explicit_preparation_repair_defers_eligibility_to_native_and_refusal_cannot_publish(self):
        self.args.repair_preparation = True
        before = FIXTURE.snapshot(self.campaign)
        with self.assertRaisesRegex(ValueError, "existing interrupted"):
            self.plan()
        self.assertEqual(FIXTURE.snapshot(self.campaign), before)
        self.namespace()
        plan = self.plan()
        self.assertIn("--repair-preparation", plan["native_command"])
        self.assertIn("--resume", plan["native_command"])
        self.native["committed"].update(required_complete=True, all_rows_exhausted=True)
        for exit_code, stopped, expected in [(4, False, 4), (8, False, 8), (0, True, 8), (0, False, 0)]:
            with self.subTest(exit_code=exit_code, stopped=stopped), \
                 patch.object(FIN, "inspect", return_value=self.native) as inspect, \
                 patch.object(FIN.PHASES, "phase_one", return_value=(exit_code, stopped)) as work, \
                 patch.object(FIN.PHASES, "phase_two", return_value=0) as publish:
                self.assertEqual(FIN.PHASES.run(plan, FIN.PHASES.configuration(self.campaign), True, FIN), expected)
                work.assert_called_once()
                if expected:
                    inspect.assert_not_called()
                    publish.assert_not_called()
                else:
                    inspect.assert_called_once()
                    publish.assert_called_once()

    def test_extension_startup_reports_phases_then_joins_before_solve(self):
        self.namespace()
        self.args.action = "extend"
        self.args.rank = 1
        self.args.unbounded_power_difference = False
        self.args.max_power_difference = None
        self.args.repair_preparation = False
        receipt = {"status": "amendment-prepared-no-native-work"}
        stdout, stderr = io.StringIO(), io.StringIO()
        progress = FIN.STARTUP.StartupProgress(stream=stderr, log_interval=60)
        self.addCleanup(progress.stop)
        configure = FIN.PHASES.configuration
        freeze = FIN.PHASES.freeze_master_executable

        def configuration(*args, **kwargs):
            self.assertEqual(json.loads(stdout.getvalue()), receipt)
            self.assertTrue(progress._thread.is_alive())
            self.assertIn("checking publication configuration", stderr.getvalue())
            return configure(*args, **kwargs)

        def freeze_executable(*args, **kwargs):
            self.assertTrue(progress._thread.is_alive())
            self.assertIn("freezing publication executable", stderr.getvalue())
            return freeze(*args, **kwargs)

        def solve(_):
            self.assertIsNone(progress._thread)
            self.assertFalse(any(t.name == "finite-startup-progress" for t in threading.enumerate()))
            return 8, False

        with redirect_stdout(stdout), \
             patch.object(FIN.STARTUP, "StartupProgress", return_value=progress), \
             patch.object(FIN, "extension", return_value=receipt), \
             patch.object(FIN, "inspect", return_value=self.native), \
             patch.object(FIN.PHASES, "configuration", side_effect=configuration), \
             patch.object(FIN.PHASES, "freeze_master_executable", side_effect=freeze_executable), \
             patch.object(FIN.PHASES, "phase_one", side_effect=solve):
            self.assertEqual(FIN.public_locked(self.args, self.campaign, self.binary, apply=True), 8)
        self.assertEqual(json.loads(stdout.getvalue()), receipt)
        for phase in ("acquiring finite control lock", "inspecting finite checkpoint",
                      "validating committed input sources", "verifying campaign inputs",
                      "freezing solve executable", "validating current input scope"):
            self.assertIn(phase, stderr.getvalue())

    def test_startup_joins_before_publication_and_finalize_refinement(self):
        self.namespace()
        self.native["committed"].update(required_complete=True, all_rows_exhausted=True)
        plan = self.plan()
        for operation, launcher in (("publish", "phase_two"), ("finalize-refinement", "finalize_refinement")):
            with self.subTest(operation=operation):
                progress = FIN.STARTUP.StartupProgress(stream=io.StringIO(), log_interval=60)
                self.addCleanup(progress.stop)
                policy = {**FIN.PHASES.configuration(self.campaign), "operation": operation}

                def launch(*_):
                    self.assertIsNone(progress._thread)
                    self.assertFalse(any(t.name == "finite-startup-progress" for t in threading.enumerate()))
                    return 0

                with patch.object(FIN, "inspect", return_value=self.native), \
                     patch.object(FIN.PHASES, launcher, side_effect=launch):
                    self.assertEqual(FIN.PHASES.run(plan, policy, True, FIN, startup_progress=progress), 0)

    def test_startup_cleanup_on_planning_failure_and_dry_run_silence(self):
        self.args.repair_preparation = False
        for error in (ValueError("configuration failed"), KeyboardInterrupt()):
            with self.subTest(error=type(error).__name__):
                progress = FIN.STARTUP.StartupProgress(stream=io.StringIO(), log_interval=60)
                self.addCleanup(progress.stop)
                with patch.object(FIN.STARTUP, "StartupProgress", return_value=progress), \
                     patch.object(FIN.PHASES, "configuration", side_effect=error), \
                     self.assertRaises(type(error)):
                    FIN.public_locked(self.args, self.campaign, self.binary, apply=True)
                self.assertIsNone(progress._thread)
                self.assertFalse(any(t.name == "finite-startup-progress" for t in threading.enumerate()))
        self.args.dry_run = True
        stderr, stdout = io.StringIO(), io.StringIO()
        with patch.object(FIN.STARTUP.sys, "stderr", stderr), redirect_stdout(stdout):
            self.assertEqual(FIN.public_locked(self.args, self.campaign, self.binary, apply=False), 0)
        self.assertEqual(stderr.getvalue(), "")
        self.assertIn("No native work launched.", stdout.getvalue())


if __name__ == "__main__":
    unittest.main()
