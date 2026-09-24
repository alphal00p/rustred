"""Steering/transport tests only; real native generation is a separate gate."""
import argparse
import gzip
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest

SPEC = importlib.util.spec_from_file_location("generate_vakint_artifacts", Path(__file__).with_name("generate_vakint_artifacts.py"))
DRIVER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DRIVER)


def arguments(directory, **changes):
    values = dict(loops=[1, 2], families=None, output_directory=Path(directory) / "new",
                  executable=Path("/missing/release/rustred"), normalizer=Path("/missing/release/normalizer"),
                  workers=1, catalog_directory=None, reference_directory=None, execute=False)
    return argparse.Namespace(**(values | changes))


class GenerationSteeringTests(unittest.TestCase):
    def test_lower_recipes_really_generate_and_cold_load(self):
        planned = DRIVER.plan(arguments("/unused", loops=[1, 2, 3]))
        for job in planned["jobs"]:
            self.assertEqual(job["kind"], "certified")
            self.assertEqual(job["commands"][1][1:3], ["campaign", "inspect"])
            self.assertEqual(job["commands"][2][1:3], ["campaign", "reduce"])
        self.assertEqual(planned["jobs"][0]["commands"][0][1:5],
                         ["campaign", "generate", "--family", "unit-mass-vacuum-k1"])
        self.assertEqual(planned["jobs"][1]["commands"][0][4], "unit-mass-vacuum-k3")
        self.assertEqual(planned["jobs"][2]["commands"][0][1], "family-close")
        self.assertTrue(planned["jobs"][2]["source"].endswith("three_loop_k6.toml"))

    def test_four_recipes_preserve_scope_and_replay_normalization(self):
        planned = DRIVER.plan(arguments("/unused", loops=[4], workers=6))
        self.assertEqual([job["name"] for job in planned["jobs"]], ["h", "fg", "bmw", "x"])
        for job in planned["jobs"]:
            command = job["commands"][0]
            self.assertEqual(command[1], "family-candidates")
            # Historical generic producer stored Auto even for these TOMLs.
            self.assertEqual(command[command.index("--input-format") + 1], "auto")
            self.assertEqual(command[command.index("--finite-case-policy") + 1], "search")
            self.assertEqual(command[command.index("--numerical-depth") + 1], "2")
            self.assertEqual(command[command.index("--exact-backend") + 1], "sparse")
            for forbidden in ("--max-numerator-rank", "--max-total-excess-degree", "--permutation",
                              "--max-positive-power", "--min-power-difference", "--resume"):
                self.assertNotIn(forbidden, command)
            self.assertEqual(job["commands"][1][1], "generate")
            self.assertEqual(job["commands"][2][1], "verify")
            self.assertEqual(job["commands"][1][-1], "-")
        self.assertFalse(planned["four_loop_closure_certified"])
        self.assertIsNone(planned["elapsed_deadline_seconds"])

    def test_four_input_identity_and_physical_roots(self):
        for name, auxiliary in DRIVER.FOUR.items():
            source = tomllib.loads((DRIVER.INPUTS / "vakint" / f"{name}.toml").read_text())
            self.assertNotIn("name", source["family"])
            self.assertEqual(len(source["family"]["loop_momenta"]), 4)
            self.assertEqual(len(source["family"]["denominators"]), 10)
            self.assertEqual(source["target"]["powers"], [int(i not in auxiliary) for i in range(10)])
            self.assertEqual(source["family"]["external_momenta"], [])

    def test_explicit_four_family_subset(self):
        planned = DRIVER.plan(arguments("/unused", loops=[4], families=["fg"]))
        self.assertEqual([job["name"] for job in planned["jobs"]], ["fg"])

    def test_invalid_selections_fail_before_native_work(self):
        for changes in (dict(workers=0), dict(loops=[1, 1]), dict(families=["fg"]),
                        dict(catalog_directory=Path("/unused")),
                        dict(loops=[4], families=["fg", "fg"])):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                DRIVER.plan(arguments("/unused", **changes))

    def test_catalog_is_explicit_input_not_generated_master_claim(self):
        planned = DRIVER.plan(arguments("/unused", loops=[4], families=["fg"],
                                        catalog_directory=Path("/precomputed")))
        self.assertFalse(planned["numerical_master_values_generated"])
        self.assertTrue(planned["jobs"][0]["commands"][1][-1].endswith("fg.rrcat.bin"))

    def test_dry_run_needs_neither_license_binary_nor_output_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "not-created"
            run = subprocess.run([sys.executable, "-B", str(Path(DRIVER.__file__)),
                                  "--loops", "1", "2", "--output-directory", str(output),
                                  "--executable", "/does/not/exist"], capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertFalse(output.exists())
            self.assertTrue(json.loads(run.stdout)["fresh_ibp_generation"])

    def test_compression_is_deterministic_and_no_clobber(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "raw"
            source.write_bytes(b"test transport only" * 200)
            first, second = root / "first.gz", root / "second.gz"
            DRIVER.compressed_copy(source, first)
            DRIVER.compressed_copy(source, second)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertEqual(gzip.decompress(first.read_bytes()), source.read_bytes())
            with self.assertRaises(FileExistsError):
                DRIVER.compressed_copy(source, first)
            with self.assertRaisesRegex(ValueError, "exceeds"):
                DRIVER.decompressed_copy(first, root / "too-small", 10)

    def test_native_error_is_retained_and_never_retried(self):
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "failed"
            with self.assertRaisesRegex(RuntimeError, "failed"):
                DRIVER.run_command([sys.executable, "-c", "raise SystemExit(7)"], log)
            self.assertEqual(json.loads(log.with_suffix(".json").read_text())["returncode"], 7)
            self.assertTrue(log.with_suffix(".stderr").exists())


if __name__ == "__main__":
    unittest.main()
