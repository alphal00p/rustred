"""Saved unsealed formulas are not artifacts; certification is independent."""

from __future__ import annotations

import inspect
from pathlib import Path
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
        for backend in ["semi-numerical", "sparse-factorized"]:
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
        generated = rustred.family_candidates(
            UNIT_MASS_PROJECT_K3, exact_backend="sparse-factorized", n_cores=2
        )
        self.assertEqual(generated.status, "uncertified-candidates")
        self.assertEqual(generated.schema, sparse.schema)
        self.assertEqual(
            tomllib.loads(generated.to_toml())["exact_backend"], "sparse-factorized"
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
