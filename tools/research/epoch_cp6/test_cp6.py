"""Synthetic receipts and mocked launch only; zero native/process authority."""
import copy
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import collect
from contract import CONTRACT, FORMAT, TIMING, accept, expected_schedule
from gate import acceptance, freeze_receipt, prepare_verification
from receipts import ROOT, commands, read, sha, validate_plan, write_new


def documents(checkpoint):
    roles = {"total": 2, "required": 1, "auxiliary": 1}
    metrics = {"exit_code": 4, "stop_reason": None, "runner_error": None, "censored": False,
               "killed_after_grace": False, "whole_command_timing_scope": TIMING,
               "whole_command_seconds": 2.5, "resource_guard": {"child_started": True,
               "stop_reason": None, "process_group": 123, "launcher_wait_seconds": 2.4,
               "owned_group_drain_seconds": 0.1}}
    summary = {"status": "incomplete", "finalization": "not_evaluated", "recursive_worklist_exhausted": True,
               "admission_complete": True, "full_result_in_output_document": False, "full_state_in_checkpoint": True,
               "all_scheduled_domains_resolved": False, "observer_failed": False, "family_closure_claim": False,
               "stop_reason": None, "operational_stop": None, "admission_failure": None, "frontiers": 0,
               "input_frontiers_count": 0, "failed_nodes": 0, "queued_nodes": 0, "scheduled_nodes": 3,
               "native_processed_nodes": 3, "parallel": {"workers_joined": True, "active_workers": 0},
               "epoch": {"engine_certification_void": False, "inspector_lookup_mode": "snapshot",
                         "schedule": {"kind": "lockstep", "depth": 1, "b": 16}},
               "query_admission": {"requested": 2, "admitted": 2, "unadmitted": 0,
                       "required": 1, "auxiliary": 1, "admitted_required": 1, "admitted_auxiliary": 1},
               "checkpoint": {"format": FORMAT, "schema": 1, "state": "saved", "resumable": True,
                       "saved_this_invocation": True, "directory": checkpoint, "generation": 2,
                       "paused": False, "stop_reason": None, "manifest": str(Path(checkpoint) / "latest.json"),
                       "manifest_blake3": "aa" * 32}}
    cold = {"schema": "rustred.walk-verify-closure.v1", "verdict": "PASS", "family_closure_claim": False,
            "mutation": None, "reference": {"native_levers": "Off"}, "result_binding": None,
            "violations": [], "violations_suppressed": 0, "roots_total": 2, "roots_independently_verified": 2,
            "reinspection": {"mode": "All", "complete": True, "candidates": 3, "selected": 3,
                             "tally": {"inspected": 3}},
            "checkpoint": {"directory": checkpoint, "generation": 2, "publication_policy": "epoch",
                           "walk_semantics_version": 3, "request_binding_matches": True, "owner_digests_match": True},
            "queries": {"blake3": "bb" * 32, "count": 2, "admitted_prefix": 2, "unadmitted": 0,
                        "unresolved_input_frontiers": 0, "unadmitted_ids": []},
            "certification": {"classes": {k: {"total": 1, "independently_verified": 1,
                               "consistent_closed": 1, "oracle_closed": 1} for k in ("physics", "helper")}},
            "counts": {"domains": 3, "oracle_closed": 3}}
    audit = {"audit": "INCOMPLETE", "violations": [], "violations_suppressed": 0,
             "all_local_obligations_discharged": False,
             "incomplete_reason": "checkpoint-only output has no full record proof; native --no-result needed"}
    expected = {"contract": CONTRACT, "checkpoint": checkpoint, "mode": "snapshot", "b": 16,
                "roles": roles, "queries_blake3": "bb" * 32}
    return [metrics, summary, cold, audit, {"exit_code": 0, "reason": None},
            {"exit_code": 1, "reason": None}, expected]


class PredicateTests(unittest.TestCase):
    def test_explicit_rolling_dispatch_union_and_current_schema_keep_cold_all_authority(self):
        for dispatch in ("fifo", "adaptive"):
            values = documents("/unused/checkpoint")
            values[6].update(b=20, checkpoint_schema=2, g2="union", schedule={
                "kind": "rolling", "depth": 2, "b": 20, "window": 20, "cut_size": 16,
                "publication_order": "oldest_sequence_prefix", "dispatch": dispatch})
            values[0]["g2"] = "union"
            values[1]["epoch"]["schedule"] = copy.deepcopy(values[6]["schedule"])
            values[1]["checkpoint"]["schema"] = 2
            values[1]["g2_residual_anchors"] = {"mode": "union"}
            result = accept(*values)
            self.assertEqual(result["schedule"], values[6]["schedule"])
            self.assertEqual(result["g2"], "union")
            self.assertEqual(result["checkpoint_schema"], 2)
            for index, path, bad in [
                (0, ["g2"], "off"), (1, ["g2_residual_anchors"], None),
                (1, ["epoch", "schedule", "dispatch"], "fifo" if dispatch == "adaptive" else "adaptive"),
                (1, ["epoch", "schedule", "window"], 21),
                (1, ["epoch", "schedule", "depth"], True),
                (1, ["checkpoint", "schema"], 1),
                (2, ["reference", "native_levers"], "AsRun"),
                (2, ["reinspection", "mode"], "PhysicsQueries"),
                (2, ["counts", "oracle_closed"], 2),
            ]:
                mutated = copy.deepcopy(values)
                target = mutated[index]
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = bad
                with self.subTest(dispatch=dispatch, path=path), self.assertRaises(ValueError):
                    accept(*mutated)

    def test_schedule_plan_is_bounded_consistent_and_legacy_shape_stays_exact(self):
        plan = {"b": 20, "schedule": {"kind": "rolling", "depth": 2, "b": 20,
                "window": 20, "cut_size": 16, "publication_order": "oldest_sequence_prefix",
                "dispatch": "adaptive"}}
        self.assertEqual(expected_schedule(plan), plan["schedule"])
        for key, bad in [("depth", 1), ("b", True), ("window", 21), ("cut_size", 0),
                         ("cut_size", 21), ("publication_order", "arrival"),
                         ("kind", "lockstep"), ("dispatch", "random")]:
            mutated = copy.deepcopy(plan)
            mutated["schedule"][key] = bad
            with self.subTest(key=key), self.assertRaises(ValueError):
                expected_schedule(mutated)
        values = documents("/unused/checkpoint")
        values[1]["epoch"]["schedule"]["window"] = 16
        with self.assertRaises(ValueError):
            accept(*values)  # No implicit widening of old receipt semantics.

    def test_only_raw_cold_all_not_python_pass_is_authority(self):
        result = accept(*documents("/unused/checkpoint"))
        self.assertTrue(result["accepted"])
        self.assertEqual(result["authority"], "raw_cp6_cold_all")
        self.assertEqual(result["native_exit_code"], 4)
        self.assertEqual(result["python_audit"], "INCOMPLETE")
        self.assertFalse(result["full_result_proof"])

    def test_fail_closed_mutation_matrix(self):
        cases = [(0, ["exit_code"], 0), (0, ["censored"], True), (0, ["stop_reason"], "time_limit"),
                 (0, ["runner_error"], "failed"), (0, ["killed_after_grace"], True),
                 (0, ["resource_guard", "owned_group_drain_seconds"], None),
                 (0, ["whole_command_seconds"], float("nan")),
                 (1, ["recursive_worklist_exhausted"], False), (1, ["observer_failed"], True),
                 (1, ["operational_stop"], "ram_guard"), (1, ["frontiers"], 1), (1, ["queued_nodes"], 1),
                 (1, ["native_processed_nodes"], True), (1, ["query_admission", "unadmitted"], 1),
                 (1, ["epoch", "inspector_lookup_mode"], "all-miss"),
                 (1, ["checkpoint", "generation"], 0), (1, ["checkpoint", "resumable"], False),
                 (2, ["verdict"], "INCOMPLETE"), (2, ["reinspection", "mode"], "Sample"),
                 (2, ["reinspection", "complete"], False), (2, ["reinspection", "selected"], 2),
                 (2, ["reinspection", "tally", "inspected"], 2), (2, ["roots_total"], True),
                 (2, ["roots_independently_verified"], 1), (2, ["reference", "native_levers"], "AsRun"),
                 (2, ["result_binding"], {"kind": "checkpoint_only_summary"}),
                 (2, ["checkpoint", "generation"], 1), (2, ["checkpoint", "request_binding_matches"], False),
                 (2, ["queries", "blake3"], "cc" * 32), (2, ["queries", "count"], 1),
                 (2, ["certification", "classes", "physics", "independently_verified"], 0),
                 (2, ["certification", "classes", "helper", "total"], 2),
                 (2, ["counts", "oracle_closed"], 2), (2, ["violations"], ["bad"]),
                 (3, ["audit"], "PASS"), (3, ["audit"], "FAIL"),
                 (3, ["incomplete_reason"], "other incomplete"), (4, ["reason"], "timeout"),
                 (5, ["exit_code"], 0), (5, ["reason"], "ram_guard")]
        for index, path, value in cases:
            with self.subTest(path=path, value=value):
                values = documents("/unused/checkpoint")
                target = values[index]
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = value
                with self.assertRaises((ValueError, KeyError, TypeError)):
                    accept(*values)

    def test_missing_receipt_fields_never_default_to_success(self):
        for index in range(7):
            values = documents("/unused/checkpoint")
            for key in list(values[index]):
                bad = copy.deepcopy(values)
                del bad[index][key]
                with self.subTest(index=index, key=key):
                    # Extra schema is handled by the reused oracle gate.
                    if index == 2 and key == "schema":
                        continue
                    with self.assertRaises((ValueError, KeyError, TypeError)):
                        accept(*bad)


class FileReceiptTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        run, cp = self.base / "run", self.base / "checkpoint"
        run.mkdir(); cp.mkdir()
        query, binary, guard = [self.base / name for name in ("queries.json", "binary", "guard.py")]
        write_new(query, {"queries": [{"id": "q"}, {"id": "helper"}],
                          "query_roles": {"required": ["q"], "auxiliary": ["helper"]}})
        binary.write_bytes(b"fake executable; never launched")
        guard.write_bytes(b"fake guard; never launched")
        native = [str(binary), "owner-domain-match", "--queries", str(query), "--checkpoint", str(cp),
                  "--output", str(run / "result.json"), "--publication-policy", "epoch",
                  "--epoch-inspector-lookup", "snapshot", "--follow-successors"]
        self.plan = {"contract": CONTRACT, "run": str(run), "checkpoint": str(cp), "queries": str(query),
                     "native_cwd": str(ROOT), "verification_cwd": str(ROOT),
                     "queries_sha256": sha(query), "queries_blake3": "bb" * 32,
                     "binary": str(binary), "binary_sha256": sha(binary), "mode": "snapshot", "b": 16,
                     "native_argv": native, "launcher_argv": ["nice", "-n", "5", "nix", "develop", str(ROOT), "--command"] + native,
                     "verify_threads": 1, "verification_timeout": 60, "python": "/fake/python",
                     "verification_guard": str(guard), "verification_guard_sha256": sha(guard),
                     "minimum_start_bytes": 250 << 30, "minimum_run_bytes": 150 << 30,
                     "verification_resources": {stage: {"cpus": [32], "locks": ["/fake/lock"]}
                                                for stage in ("cold-verifier", "python-audit")},
                     "runner_sha256": "cc" * 32}
        self.values = documents(str(cp))
        for name, value in zip(("metrics.json", "result.json", "cold-verify.json", "cold-audit.json"), self.values):
            write_new(run / name, value)
        write_new(run / "command.json", native)
        write_new(run / "cp6-collection.json", {"contract": CONTRACT, "status": "COLLECTED_UNACCEPTED",
                    "native_exit_code": 4, "legacy_runner_exit_code": 1, "actual_launcher_argv": self.plan["launcher_argv"],
                    "runner_sha256": self.plan["runner_sha256"], "metrics_sha256": sha(run / "metrics.json")})
        write_new(cp / "latest.json", {"manifest": {"format": FORMAT, "schema": 1, "generation": 2,
                    "walk_semantics_version": 3, "resumable": True}, "blake3": [170] * 32})
        for name in ("previous.json", "epoch-session.bin", "checkpoint.lock", "epoch-payload.part"):
            (cp / name).write_bytes(b"unchanged fake authority/payload")
        for index, (stage, command) in enumerate(commands(self.plan).items()):
            (run / stage).mkdir()
            write_new(run / stage / "request.json", dict(command=command, cwd=str(ROOT), minimum_start_bytes=250 << 30,
                       stop_below_bytes=150 << 30, **self.plan["verification_resources"][stage]))
            write_new(run / stage / "result.json", self.values[4 + index])

    def test_frozen_actual_roles_and_read_only_gate(self):
        frozen = freeze_receipt(self.plan)
        self.assertEqual(frozen["expected"]["roles"], {"total": 2, "required": 1, "auxiliary": 1})
        self.assertIn("--no-result", frozen["commands"]["cold-verifier"])
        self.assertNotIn("--result", frozen["commands"]["cold-verifier"])
        self.assertTrue(acceptance(self.plan, frozen)["checkpoint_read_only"])

    def test_plan_requires_exact_rolling_dispatch_and_union_flags(self):
        plan = copy.deepcopy(self.plan)
        plan.update(b=20, checkpoint_schema=2, g2="union", schedule={
            "kind": "rolling", "depth": 2, "b": 20, "window": 20, "cut_size": 16,
            "publication_order": "oldest_sequence_prefix", "dispatch": "adaptive"})
        plan["native_argv"] += ["--epoch-rolling", "--epoch-dispatch", "adaptive",
                                "--g2-residual-anchors", "union"]
        self.assertEqual(validate_plan(plan)["g2"], "union")
        for flag in ("--epoch-rolling", "--epoch-dispatch", "--g2-residual-anchors"):
            for kind in ("missing", "duplicate", "equals"):
                mutated = copy.deepcopy(plan)
                argv = mutated["native_argv"]
                position = argv.index(flag)
                if kind == "missing":
                    del argv[position]
                elif kind == "duplicate":
                    argv.append(flag)
                else:
                    argv[position] = flag + "=unexpected"
                with self.subTest(flag=flag, kind=kind), self.assertRaises(ValueError):
                    validate_plan(mutated)
        for field, value in (("g2", "off"), ("checkpoint_schema", 3)):
            mutated = copy.deepcopy(plan)
            mutated[field] = value
            with self.assertRaises(ValueError):
                validate_plan(mutated)

    def test_pointer_session_lock_payload_or_new_file_change_refuses(self):
        for name in ("latest.json", "previous.json", "epoch-session.bin", "checkpoint.lock", "epoch-payload.part", "new.part"):
            with self.subTest(name=name):
                frozen = freeze_receipt(self.plan)
                path = Path(self.plan["checkpoint"]) / name
                old = path.read_bytes() if path.exists() else None
                path.write_bytes(b"changed")
                with self.assertRaises((ValueError, KeyError, TypeError)):
                    acceptance(self.plan, frozen)
                if old is None:
                    path.unlink()
                else:
                    path.write_bytes(old)

    def test_changed_guard_command_or_query_bytes_refuses(self):
        frozen = freeze_receipt(self.plan)
        path = Path(self.plan["run"]) / "cold-verifier/request.json"
        value = read(path)
        value["command"].remove("--no-result")
        path.write_text(__import__("json").dumps(value))
        with self.assertRaises(ValueError):
            acceptance(self.plan, frozen)
        Path(self.plan["queries"]).write_text("{}")
        with self.assertRaises(ValueError):
            validate_plan(self.plan)

    def test_verification_requires_fresh_paths_and_registered_working_directory(self):
        with self.assertRaises(ValueError):
            prepare_verification(self.plan)
        frozen = freeze_receipt(self.plan)
        path = Path(self.plan["run"]) / "cold-verifier/request.json"
        value = read(path)
        value["cwd"] = "/different"
        path.write_text(__import__("json").dumps(value))
        with self.assertRaises(ValueError):
            acceptance(self.plan, frozen)

    def test_same_original_guard_hook_restores_after_success_and_failure(self):
        plan = dict(self.plan, cpus=[32], locks=["/fake/lock"], time_limit=60, grace=5,
                    resource_environment={"RAYON_NUM_THREADS": "1"})
        called = []
        class Guard:
            def launch(self, command, **kwargs):
                called.append((self, command, kwargs))
                return "original sentinel"
        guard = Guard()
        guard.stop_file = Path(plan["run"]) / "stop-request.json"
        guard.paths = [Path("/fake/lock")]
        guard.minimum_start, guard.minimum_run = 250 << 30, 150 << 30
        guard.time_limit, guard.grace = 60, 5
        runner = SimpleNamespace(ArmGuard=Guard)
        original = Guard.launch
        kwargs = {"env": {"RAYON_NUM_THREADS": "1"}, "stdout": object(), "preexec_fn": object(), "cwd": ROOT}
        with patch.object(collect.os, "sched_getaffinity", return_value={32}):
            with collect.checked_launch(runner, plan):
                self.assertEqual(guard.launch(plan["launcher_argv"], **kwargs), "original sentinel")
            self.assertIs(Guard.launch, original)
            with self.assertRaises(ValueError), collect.checked_launch(runner, plan):
                guard.launch(["different"], **kwargs)
            self.assertIs(Guard.launch, original)
        self.assertIs(called[0][0], guard)
        self.assertIs(called[0][2]["preexec_fn"], kwargs["preexec_fn"])
        self.assertEqual(len(called), 1)

    def test_collection_transports_runner_one_without_accepting_and_keeps_hook_on_errors(self):
        plan = dict(self.plan, cpus=[32], locks=["/fake/lock"], time_limit=60, grace=5,
                    resource_environment={"RAYON_NUM_THREADS": "1"}, runner_argv=[
                        "--policy", "epoch", "--g2", "off", "--heavy-lock", "/fake/lock",
                        "--out-root", str(self.base.parent), "--label", self.base.name,
                        "--family", "run"])
        launches = []
        class Guard:
            def launch(self, command, **kwargs):
                launches.append((self, command, kwargs))
                return None  # Fake only; no child/process/session created.
        guard = Guard()
        guard.stop_file, guard.paths = Path(plan["run"]) / "stop-request.json", [Path("/fake/lock")]
        guard.minimum_start, guard.minimum_run = 250 << 30, 150 << 30
        guard.time_limit, guard.grace = 60, 5
        runner_code = [1]
        def main():
            guard.launch(plan["launcher_argv"], cwd=ROOT, env={"RAYON_NUM_THREADS": "1"})
            return runner_code[0]
        runner = SimpleNamespace(ArmGuard=Guard, main=main)
        original, argv = Guard.launch, collect.sys.argv
        # Existing synthetic files stand in for runner outputs; only the
        # pre-launch existence check is mocked. No runner or child is loaded.
        with patch.object(collect.os, "sched_getaffinity", return_value={32}), \
                patch.object(Path, "exists", return_value=False), \
                patch.object(collect, "write_new") as write:
            result = collect.collect(plan, runner)
            self.assertEqual(result["status"], "COLLECTED_UNACCEPTED")
            self.assertEqual(result["native_exit_code"], 4)
            self.assertEqual(result["legacy_runner_exit_code"], 1)
            self.assertNotIn("accepted", result)
            self.assertIs(Guard.launch, original)
            self.assertIs(collect.sys.argv, argv)
            write.reset_mock()
            runner_code[0] = 0
            with self.assertRaises(ValueError):
                collect.collect(plan, runner)
            write.assert_not_called()
            runner_code[0] = 1
            write.side_effect = OSError("receipt disk failure")
            with self.assertRaises(OSError):
                collect.collect(plan, runner)
            self.assertIs(Guard.launch, original)
            self.assertIs(collect.sys.argv, argv)
        self.assertEqual(len(launches), 3)


if __name__ == "__main__":
    unittest.main()
