"""Saved unsealed formulas are not artifacts; certification is independent."""

from __future__ import annotations

import inspect
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest

import rustred

from test_python_api import (
    GeneratedProgramAssertions,
    UNIT_MASS_PROJECT_K1,
    UNIT_MASS_PROJECT_K3,
    cli_bytes,
)


class CandidateApiTests(GeneratedProgramAssertions):
    def test_bundle_output_limits_are_strict_optional_and_fail_closed(self) -> None:
        signature = inspect.signature(rustred.family_candidates)
        keywords = ["bundle_max_bytes", "bundle_max_entries", "bundle_max_coefficient_bytes",
                    "bundle_max_total_coefficient_bytes"]
        for key in keywords:
            self.assertIsNone(signature.parameters[key].default)
            self.assertEqual(signature.parameters[key].kind, inspect.Parameter.KEYWORD_ONLY)
            for value in [True, False, 0, -1, 0.5, "2", 1 << 128]:
                with self.subTest(key=key, value=value), self.assertRaises(rustred.RustRedInputError):
                    rustred.family_candidates("not parsed", **{key: value})
            with self.subTest(tiny=key), self.assertRaisesRegex(rustred.RustRedError, "limit"):
                rustred.family_candidates(UNIT_MASS_PROJECT_K1, **{key: 1})
        with self.assertRaisesRegex(rustred.RustRedInputError, "hard 1073741824-byte"):
            rustred.family_candidates("not parsed", bundle_max_bytes=(1 << 30) + 1)
        ordinary = rustred.family_candidates(UNIT_MASS_PROJECT_K1)
        explicit_none = rustred.family_candidates(UNIT_MASS_PROJECT_K1, **dict.fromkeys(keywords))
        self.assertProgramEqual(ordinary.bundle, explicit_none.bundle)

    def test_larger_bundle_budgets_preserve_k3_finite_output_and_checkpoint_identity(self) -> None:
        finite = {"max_numerator_rank": 2, "finite_case_policy": "retain-rank-finite"}
        enlarged = {"bundle_max_bytes": 1 << 30, "bundle_max_entries": 32_000_000,
                    "bundle_max_coefficient_bytes": 32 << 20,
                    "bundle_max_total_coefficient_bytes": 512 << 20}
        ordinary = rustred.family_candidates(UNIT_MASS_PROJECT_K3, **finite)
        fresh_enlarged = rustred.family_candidates(UNIT_MASS_PROJECT_K3, **finite, **enlarged)
        self.assertProgramEqual(ordinary.bundle, fresh_enlarged.bundle)
        scratch = Path(__file__).resolve().parents[3] / "TMP"
        scratch.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch, prefix="python-candidate-budgets-") as tmp:
            checkpoint = Path(tmp) / "sectors"
            initial = rustred.family_candidates(UNIT_MASS_PROJECT_K3, checkpoint_dir=checkpoint, **finite)
            before = {path.name: path.read_bytes() for path in checkpoint.iterdir()}
            resumed = rustred.family_candidates(UNIT_MASS_PROJECT_K3, checkpoint_dir=checkpoint,
                resume=True, **finite, **enlarged)
            self.assertProgramEqual(ordinary.bundle, initial.bundle)
            self.assertProgramEqual(ordinary.bundle, resumed.bundle)
            report = tomllib.loads(resumed.to_toml())["checkpoint"]
            self.assertEqual(report["newly_solved_sectors"], 0)
            self.assertEqual(report["reused_sectors"], 4)
            arguments = ["family-candidates", "--max-numerator-rank", "2",
                "--finite-case-policy", "retain-rank-finite", "--checkpoint-dir", str(checkpoint), "--resume"]
            for key, value in enlarged.items():
                arguments.extend(["--" + key.replace("_", "-"), str(value)])
            cli_resumed = cli_bytes(arguments, UNIT_MASS_PROJECT_K3.encode())
            self.assertProgramEqual(ordinary.bundle, cli_resumed)
            self.assertEqual(before, {path.name: path.read_bytes() for path in checkpoint.iterdir()})

    def test_finite_retention_is_explicit_rank_scoped_and_matches_cli(self) -> None:
        signature = inspect.signature(rustred.family_candidates)
        self.assertEqual(signature.parameters["finite_case_policy"].default, "search")
        self.assertIsNone(signature.parameters["finite_max_visited_points"].default)
        for kwargs in [
            {"finite_case_policy": "unknown"},
            {"finite_case_policy": "retain-rank-finite"},
            {"finite_max_visited_points": 10},
            {"finite_max_retained_terminals": 10},
        ]:
            with self.subTest(kwargs=kwargs), self.assertRaises(rustred.RustRedInputError):
                rustred.family_candidates("not parsed", **kwargs)
        for key in ["finite_max_visited_points", "finite_max_retained_terminals"]:
            for value in [True, False, 0, -1, 0.5, "2", 1 << 128]:
                with self.subTest(key=key, value=value), self.assertRaises(rustred.RustRedInputError):
                    rustred.family_candidates("not parsed", max_numerator_rank=10,
                        finite_case_policy="retain-rank-finite", **{key: value})
        generated = rustred.family_candidates(UNIT_MASS_PROJECT_K3, max_numerator_rank=2,
            finite_case_policy="retain-rank-finite", finite_max_visited_points=10000,
            finite_max_retained_terminals=10000)
        report = tomllib.loads(generated.to_toml())
        self.assertEqual(report["finite_case_policy"], "retain-rank-finite")
        self.assertGreater(report["finite_residuals"], 0)
        via_cli = cli_bytes(["family-candidates", "--max-numerator-rank", "2",
            "--finite-case-policy", "retain-rank-finite", "--finite-max-visited-points", "10000",
            "--finite-max-retained-terminals", "10000"], UNIT_MASS_PROJECT_K3.encode())
        self.assertProgramEqual(generated.bundle, via_cli)
        with self.assertRaisesRegex(rustred.RustRedError, "rank-scoped candidates cannot be certified"):
            rustred.certify_candidates(generated.bundle)

    def test_generation_numerator_rank_is_optional_strict_and_not_certification(self) -> None:
        parameter = inspect.signature(rustred.family_candidates).parameters["max_numerator_rank"]
        self.assertIsNone(parameter.default)
        self.assertEqual(parameter.kind, inspect.Parameter.KEYWORD_ONLY)
        self.assertNotIn("max_numerator_rank", inspect.signature(rustred.certify_candidates).parameters)
        for value in [True, False, -1, 0.5, "2", 1 << 32, 1 << 128]:
            with self.subTest(value=value), self.assertRaises(rustred.RustRedInputError):
                rustred.family_candidates("not parsed", max_numerator_rank=value)
        ordinary = rustred.family_candidates(UNIT_MASS_PROJECT_K1)
        explicit_none = rustred.family_candidates(UNIT_MASS_PROJECT_K1, max_numerator_rank=None)
        self.assertProgramEqual(ordinary.bundle, explicit_none.bundle)
        self.assertNotIn("max_numerator_rank", tomllib.loads(ordinary.to_toml()))
        for rank in [0, 10, 20, (1 << 32) - 1]:
            with self.subTest(rank=rank):
                bounded = rustred.family_candidates(UNIT_MASS_PROJECT_K1, max_numerator_rank=rank)
                self.assertEqual(tomllib.loads(bounded.to_toml())["max_numerator_rank"], rank)
                via_cli = cli_bytes(["family-candidates", "--max-numerator-rank", str(rank)], UNIT_MASS_PROJECT_K1.encode())
                self.assertProgramEqual(bounded.bundle, via_cli)
                for total_excess in [None, 0, 30]:
                    with self.assertRaisesRegex(rustred.RustRedError, "rank-scoped candidates cannot be certified"):
                        rustred.certify_candidates(bounded.bundle, max_total_excess_degree=total_excess)

    def test_total_excess_validation_is_strict_and_distinct_from_numerator_rank(self) -> None:
        signature = inspect.signature(rustred.certify_candidates)
        parameter = signature.parameters["max_total_excess_degree"]
        self.assertIsNone(parameter.default)
        self.assertEqual(parameter.kind, inspect.Parameter.KEYWORD_ONLY)
        self.assertNotIn("max_total_excess_degree", inspect.signature(rustred.family_candidates).parameters)
        for value in [True, False, -1, 0.5, "2", 1 << 64, 1 << 128]:
            with self.subTest(value=value), self.assertRaises(rustred.RustRedInputError):
                rustred.certify_candidates(b"not decoded", max_total_excess_degree=value)
        with self.assertRaisesRegex(rustred.RustRedInputError, "mutually exclusive"):
            rustred.certify_candidates(
                b"not decoded", max_total_excess_degree=2, max_negative_index_degree=1,
            )
        # Valid u64 conversion reaches native-input rejection; the old rank cap
        # is not an argument cap for the new total-excess scope.
        with self.assertRaises(rustred.RustRedError) as rejected:
            rustred.certify_candidates(b"not decoded", max_total_excess_degree=(1 << 64) - 1)
        self.assertNotIn("max_total_excess_degree must", str(rejected.exception))
        self.assertNotIn("rank-scoped limit", str(rejected.exception))
        with self.assertRaisesRegex(rustred.RustRedError, "refusing an unbounded certification fallback"):
            rustred.certify_candidates(b"not decoded", max_negative_index_degree=30)
        with self.assertRaisesRegex(rustred.RustRedInputError, "supported rank-scoped limit"):
            rustred.certify_candidates(b"not decoded", max_negative_index_degree=31)

    def test_total_excess_artifacts_match_cli_and_cold_cross_frontend_application(self) -> None:
        for source, targets, outside in [
            (UNIT_MASS_PROJECT_K1, [[3]], [4]),
            (UNIT_MASS_PROJECT_K3, [[3, 1, 1], [-1, 2, 1]], [4, 1, 1]),
        ]:
            with self.subTest(source=source):
                bundle = rustred.family_candidates(source).bundle
                bounded = rustred.certify_candidates(bundle, max_total_excess_degree=2)
                control = rustred.certify_candidates(bundle)
                explicit_none = rustred.certify_candidates(bundle, max_total_excess_degree=None)
                self.assertProgramEqual(control.artifact, explicit_none.artifact)
                self.assertNotIn("max_total_excess_degree", tomllib.loads(control.to_toml()))
                self.assertNotIn("total_excess_scope", tomllib.loads(
                    rustred.inspect_closing_artifact(control.artifact).to_toml(),
                )["artifact"])
                report = tomllib.loads(bounded.to_toml())
                self.assertEqual(report["max_total_excess_degree"], 2)
                self.assertGreater(report["successor_sector_count"], 0)
                self.assertGreaterEqual(report["max_successor_total_excess_degree"], 2)
                cli_artifact = cli_bytes(
                    ["certify-candidates", "--max-total-excess-degree", "2"], bundle,
                )
                self.assertProgramEqual(bounded.artifact, cli_artifact)
                # Python-produced bytes are inspected and applied by fresh CLI
                # processes. The scope summary is from their verified owner.
                inspected = tomllib.loads(cli_bytes(
                    ["campaign", "inspect", "--artifact", "-"], bounded.artifact,
                ).decode())
                scope = inspected["artifact"]["total_excess_scope"]
                self.assertEqual(scope["max_entry_total_excess_degree"], 2)
                self.assertEqual(scope["successor_sector_count"], report["successor_sector_count"])
                self.assertEqual(scope["max_successor_total_excess_degree"], report["max_successor_total_excess_degree"])
                for powers in targets:
                    reference = tomllib.loads(rustred.reduce_with_closing_artifact(
                        control.artifact, powers,
                    ).to_toml())
                    cold_cli = tomllib.loads(cli_bytes([
                        "campaign", "reduce", "--artifact", "-", "--powers", ",".join(map(str, powers)),
                    ], bounded.artifact).decode())
                    # Inverse direction: a fresh Python interpreter cold-loads
                    # CLI-produced bytes, not the existing process's owner.
                    script = (
                        "import sys, rustred\n"
                        "artifact = sys.stdin.buffer.read()\n"
                        "powers = [int(value) for value in sys.argv[1].split(',')]\n"
                        "sys.stdout.write(rustred.reduce_with_closing_artifact(artifact, powers).to_toml())\n"
                    )
                    child = subprocess.run(
                        [sys.executable, "-c", script, ",".join(map(str, powers))],
                        input=cli_artifact, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                        env={**os.environ, "SYMBOLICA_HIDE_BANNER": "1"}, check=False,
                    )
                    self.assertEqual(child.returncode, 0, child.stderr.decode(errors="replace"))
                    self.assertEqual(child.stderr, b"")
                    cold_python = tomllib.loads(child.stdout.decode())
                    for result in [cold_cli, cold_python]:
                        for key in ["status", "target", "family_fingerprint", "common_mass_squared_symbol", "terms"]:
                            self.assertEqual(result[key], reference[key])
                with self.assertRaisesRegex(rustred.RustRedError, "certified entry maximum 2"):
                    rustred.reduce_with_closing_artifact(cli_artifact, outside)
                with self.assertRaises(rustred.RustRedError):
                    rustred.certify_candidates(
                        bundle, max_total_excess_degree=2, max_domain_bound_endpoint_cells=0,
                    )

    def test_total_excess_zero_and_above_old_rank_cap(self) -> None:
        bundle = rustred.family_candidates(UNIT_MASS_PROJECT_K1).bundle
        for degree in [0, 31]:
            bounded = rustred.certify_candidates(bundle, max_total_excess_degree=degree)
            self.assertEqual(tomllib.loads(bounded.to_toml())["max_total_excess_degree"], degree)
            self.assertEqual(
                rustred.reduce_with_closing_artifact(bounded.artifact, [degree + 1]).status,
                "reduced",
            )
            with self.assertRaisesRegex(rustred.RustRedError, f"certified entry maximum {degree}"):
                rustred.reduce_with_closing_artifact(bounded.artifact, [degree + 2])

    def test_native_checkpoints_reuse_across_frontends_and_worker_counts(self) -> None:
        signature = inspect.signature(rustred.family_candidates)
        self.assertIsNone(signature.parameters["checkpoint_dir"].default)
        self.assertFalse(signature.parameters["resume"].default)
        self.assertIsNone(signature.parameters["checkpoint_max_bytes"].default)
        # All generated test files stay in this checkout, independent of TMPDIR.
        scratch = Path(__file__).resolve().parents[3] / "TMP"
        scratch.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch, prefix="python-candidate-checkpoint-") as tmp:
            for label, source in [("k1", UNIT_MASS_PROJECT_K1), ("k3", UNIT_MASS_PROJECT_K3)]:
                checkpoint = Path(tmp) / label
                ordinary = rustred.family_candidates(source)
                initial = rustred.family_candidates(source, checkpoint_dir=checkpoint, checkpoint_max_bytes=1 << 24)
                # Fresh generation, unlike complete resume below, exercises
                # the multi-worker solver and concurrent native shard writers.
                parallel = rustred.family_candidates(
                    source, checkpoint_dir=Path(tmp) / f"{label}-parallel",
                    checkpoint_max_bytes=1 << 24, n_cores=6,
                )
                self.assertProgramEqual(ordinary.bundle, parallel.bundle)
                self.assertProgramEqual(initial.bundle, parallel.bundle)
                parallel_report = tomllib.loads(parallel.to_toml())["checkpoint"]
                self.assertEqual(parallel_report["reused_sectors"], 0)
                self.assertGreater(parallel_report["newly_solved_sectors"], 0)
                before = {path.name: path.read_bytes() for path in checkpoint.iterdir()}
                resumed = rustred.family_candidates(source, checkpoint_dir=str(checkpoint), resume=True, n_cores=6)
                self.assertNotIn("checkpoint", tomllib.loads(ordinary.to_toml()))
                initial_report = tomllib.loads(initial.to_toml())["checkpoint"]
                resumed_report = tomllib.loads(resumed.to_toml())["checkpoint"]
                self.assertEqual(initial_report["reused_sectors"], 0)
                self.assertGreater(initial_report["newly_solved_sectors"], 0)
                self.assertEqual(resumed_report["newly_solved_sectors"], 0)
                self.assertEqual(resumed_report["reused_sectors"], initial_report["newly_solved_sectors"])
                for field in ["disk_bytes", "resume_validation_us", "assembly_us"]:
                    self.assertIn(field, resumed_report)
                self.assertProgramEqual(ordinary.bundle, initial.bundle)
                self.assertProgramEqual(ordinary.bundle, resumed.bundle)
                cli_resumed = cli_bytes([
                    "family-candidates", "--checkpoint-dir", str(checkpoint), "--resume", "--n-cores", "1",
                ], source.encode())
                self.assertProgramEqual(ordinary.bundle, cli_resumed)
                # Both certification and application are fresh subprocesses,
                # not merely a comparison of in-process native dictionaries.
                cold_artifact = cli_bytes(["certify-candidates"], cli_resumed)
                self.assertProgramEqual(
                    rustred.certify_candidates(ordinary.bundle).artifact,
                    cold_artifact,
                )
                powers = "3" if label == "k1" else "2,1,1"
                cold_reduction = tomllib.loads(cli_bytes([
                    "campaign", "reduce", "--artifact", "-", "--powers", powers,
                ], cold_artifact).decode())
                self.assertEqual(cold_reduction["status"], "reduced")
                self.assertEqual(before, {path.name: path.read_bytes() for path in checkpoint.iterdir()})
                with self.assertRaises(rustred.RustRedInputError):
                    rustred.family_candidates(source, checkpoint_dir=checkpoint, resume=True, numerical_depth=0)

    def test_checkpoint_options_reject_invalid_policy_before_source_work(self) -> None:
        for kwargs in [{"resume": True}, {"checkpoint_max_bytes": 1}, {"checkpoint_dir": ""}]:
            with self.subTest(kwargs=kwargs), self.assertRaises(rustred.RustRedInputError):
                rustred.family_candidates("not parsed", **kwargs)
        for value in [True, False, 0, -1, 1 << 64, 1 << 128, 0.5, "1"]:
            with self.subTest(value=value), self.assertRaises(rustred.RustRedInputError):
                rustred.family_candidates("not parsed", checkpoint_dir="unused", checkpoint_max_bytes=value)
        self.assertNotIn("checkpoint_dir", inspect.signature(rustred.certify_candidates).parameters)

    def test_numerical_depth_is_generic_persisted_and_matches_cli(self) -> None:
        signature = inspect.signature(rustred.family_candidates)
        self.assertEqual(signature.parameters["numerical_depth"].default, 2)
        default = rustred.family_candidates(UNIT_MASS_PROJECT_K1)
        explicit = rustred.family_candidates(UNIT_MASS_PROJECT_K1, numerical_depth=2)
        self.assertProgramEqual(default.bundle, explicit.bundle)
        for depth in [0, 1]:
            generated = rustred.family_candidates(UNIT_MASS_PROJECT_K1, numerical_depth=depth)
            self.assertEqual(tomllib.loads(generated.to_toml())["numerical_depth"], depth)
            self.assertProgramEqual(generated.bundle, cli_bytes(
                ["family-candidates", "--numerical-depth", str(depth)],
                UNIT_MASS_PROJECT_K1.encode(),
            ))
            closed = rustred.certify_candidates(generated.bundle)
            self.assertEqual(rustred.reduce_with_closing_artifact(closed.artifact, [3]).status, "reduced")
        for value in [True, -1, 1 << 32, 1 << 128, 0.5, "0"]:
            with self.subTest(value=value), self.assertRaises(rustred.RustRedInputError):
                rustred.family_candidates("not parsed", numerical_depth=value)

    def test_exact_backend_selection_and_cli_parity(self) -> None:
        sparse = rustred.family_candidates(UNIT_MASS_PROJECT_K1)
        self.assertEqual(tomllib.loads(sparse.to_toml())["exact_backend"], "sparse")
        baseline = rustred.certify_candidates(sparse.bundle).artifact
        for backend in ["semi-numerical", "sparse-factorized", "sparse-target-factorized"]:
            with self.subTest(backend=backend):
                generated = rustred.family_candidates(
                    UNIT_MASS_PROJECT_K1, exact_backend=backend, n_cores=2
                )
                # Native Symbolica IDs may depend on process history. Compare
                # exact certified semantics, not native dump bytes.
                artifact = rustred.certify_candidates(generated.bundle).artifact
                self.assertProgramEqual(baseline, artifact)
                self.assertEqual(
                    tomllib.loads(generated.to_toml())["exact_backend"], backend
                )
                self.assertProgramEqual(
                    artifact,
                    rustred.certify_candidates(cli_bytes(
                        ["family-candidates", "--exact-backend", backend],
                        UNIT_MASS_PROJECT_K1.encode(),
                    )).artifact,
                )
        with self.assertRaises(rustred.RustRedInputError):
            rustred.family_candidates("not parsed", exact_backend="invalid")

    def test_factorized_exact_backend_preserves_complete_sunset_generation(self) -> None:
        self.assertEqual(
            inspect.signature(rustred.family_candidates).parameters["exact_backend"].default,
            "sparse",
        )
        sparse = rustred.family_candidates(UNIT_MASS_PROJECT_K3)
        for backend in ["sparse-factorized", "sparse-target-factorized"]:
            with self.subTest(backend=backend):
                generated = rustred.family_candidates(
                    UNIT_MASS_PROJECT_K3, exact_backend=backend, n_cores=2
                )
                self.assertEqual(generated.status, "uncertified-candidates")
                self.assertEqual(generated.schema, sparse.schema)
                self.assertEqual(
                    tomllib.loads(generated.to_toml())["exact_backend"], backend
                )
                self.assertProgramEqual(
                    rustred.certify_candidates(sparse.bundle).artifact,
                    rustred.certify_candidates(generated.bundle).artifact,
                )

    def test_saved_candidates_and_independent_certification(self) -> None:
        generated = rustred.family_candidates(UNIT_MASS_PROJECT_K1)
        self.assertEqual(generated.status, "uncertified-candidates")
        self.assertEqual(generated.schema, "rustred.family-candidates-output.toml.v1")
        self.assertIsInstance(generated.bundle, bytes)
        self.assertIn("uncertified-candidates", repr(generated))
        self.assertTrue(generated.bundle.startswith(b"RRPBIN\r\n"))
        with self.assertRaises(rustred.RustRedError):
            rustred.inspect_closing_artifact(generated.bundle)
        certified = rustred.certify_candidates(generated.bundle)
        self.assertEqual(certified.status, "generated-durable")
        self.assertEqual(
            certified.schema, "rustred.candidate-certification-output.toml.v1"
        )
        self.assertProgramEqual(certified.artifact, rustred.family_close(UNIT_MASS_PROJECT_K1).artifact)
        reduced = rustred.reduce_with_closing_artifact(certified.artifact, [3])
        self.assertEqual(reduced.status, "reduced")

    def test_cli_python_bundle_and_certification_parity(self) -> None:
        generated = rustred.family_candidates(UNIT_MASS_PROJECT_K1, input_format="toml")
        self.assertProgramEqual(
            rustred.certify_candidates(generated.bundle).artifact,
            rustred.certify_candidates(cli_bytes(
                ["family-candidates", "--input-format", "toml"],
                UNIT_MASS_PROJECT_K1.encode(),
            )).artifact,
        )
        certified = rustred.certify_candidates(generated.bundle, max_predicate_atoms=64)
        self.assertProgramEqual(
            certified.artifact,
            cli_bytes(["certify-candidates", "--max-predicate-atoms", "64"], generated.bundle),
        )

    def test_resource_controls_are_separate_and_validate_before_work(self) -> None:
        self.assertNotIn("max_predicate_atoms", inspect.signature(rustred.family_candidates).parameters)
        self.assertNotIn("n_cores", inspect.signature(rustred.certify_candidates).parameters)
        for value in [True, -1, 257, 1 << 128]:
            with self.subTest(value=value), self.assertRaises(rustred.RustRedInputError):
                rustred.certify_candidates(b"not decoded", max_predicate_atoms=value)
        for keyword in ["permutation", "nonpositive_indices"]:
            with self.subTest(keyword=keyword), self.assertRaises(rustred.RustRedInputError):
                rustred.family_candidates(UNIT_MASS_PROJECT_K1, **{keyword: [True]})
        generated = rustred.family_candidates(UNIT_MASS_PROJECT_K1)
        with self.assertRaises(rustred.RustRedError):
            rustred.certify_candidates(generated.bundle, max_domain_bound_endpoint_cells=0)
        # Changing a container's kind cannot turn candidates into a certificate.
        changed = bytearray(generated.bundle)
        self.assertEqual(changed[12], 1)
        changed[12] = 2
        changed = bytes(changed)
        self.assertNotEqual(changed, generated.bundle)
        with self.assertRaises(rustred.RustRedError):
            rustred.certify_candidates(changed)


if __name__ == "__main__":
    unittest.main()
