"""Saved unsealed formulas are not artifacts; certification is independent."""

from __future__ import annotations

import inspect
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
