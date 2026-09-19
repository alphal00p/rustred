"""Saved unsealed formulas are not artifacts; certification is independent."""

from __future__ import annotations

import inspect
import tomllib
import unittest

import rustred

from test_python_api import GeneratedProgramAssertions, UNIT_MASS_PROJECT_K1, cli_bytes


class CandidateApiTests(GeneratedProgramAssertions):
    def test_exact_backend_selection_and_cli_parity(self) -> None:
        sparse = rustred.family_candidates(UNIT_MASS_PROJECT_K1)
        reconstructed = rustred.family_candidates(
            UNIT_MASS_PROJECT_K1, exact_backend="semi-numerical", n_cores=2
        )
        # Native Symbolica IDs may depend on process history. Compare exact
        # certified semantics, not dirty-process native dump bytes.
        self.assertProgramEqual(
            rustred.certify_candidates(sparse.bundle).artifact,
            rustred.certify_candidates(reconstructed.bundle).artifact,
        )
        self.assertEqual(tomllib.loads(reconstructed.to_toml())["exact_backend"], "semi-numerical")
        self.assertProgramEqual(
            rustred.certify_candidates(reconstructed.bundle).artifact,
            rustred.certify_candidates(cli_bytes(
                ["family-candidates", "--exact-backend", "semi-numerical"],
                UNIT_MASS_PROJECT_K1.encode(),
            )).artifact,
        )
        with self.assertRaises(rustred.RustRedInputError):
            rustred.family_candidates("not parsed", exact_backend="invalid")

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
