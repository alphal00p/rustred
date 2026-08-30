from __future__ import annotations

import concurrent.futures
import contextlib
import importlib.util
import io
import inspect
import os
from pathlib import Path
import pickle
import socket
import subprocess
import sys
import threading
import tomllib
import unittest

import rustred


ONE_LOOP = r"""
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1),
  numerator(sp(k,k))
)
"""

HYBRID = '''schema = "rustred.project.toml.v1"
parameters = ["d", "m2"]
integral = """
I(
  name(tadpole),
  loops(k),
  externals(),
  dimension(d),
  prop(D1,k^2-m2,1)
)
"""
'''

EXPLICIT = '''schema = "rustred.project.toml.v1"
parameters = ["d", "m2"]

[family]
name = "tadpole"
loop_momenta = ["k"]
external_momenta = []
dimension = "d"

[[family.denominators]]
id = "D1"
expression = "k^2-m2"

[target]
powers = [1]
'''

PROFILE = '''schema = "rustred.campaign-execution-resource-profile.v1"
estimator_revision = 19
enclosing_memory_limit = "1024B"

[fixed_memory]
process_runtime_and_shared_catalogs = "20B"
coordinator_stack_tls_workspace = "10B"
per_worker_stack_tls_workspace = "10B"
explicitly_admitted_inner_threads = "5B"
hydrated_retained_lanes = "0B"
staged_results = "10B"
checkpoint_and_output_buffers = "20B"
safety_reserve = "35B"

[minimum_runnable_task.retained_output]
visible_logical = "60B"
opaque_native_reserve = "0B"

[minimum_runnable_task.transient_excluding_output]
visible_logical = "40B"
opaque_native_reserve = "0B"
'''

CAMPAIGN = f'''schema = "rustred.campaign-input.toml.v1"

[[roots]]
id = "tadpole"
integral = """{ONE_LOOP}"""
'''

NON_VACUUM = r"""
I(
  name(one_loop_two_externals),
  loops(k),
  externals(p,q),
  dimension(d),
  prop(D1,k^2,1),
  prop(D2,(k+p)^2,1),
  prop(D3,(k+q)^2,1),
  gram(p,p,s),
  gram(p,q,t),
  gram(q,q,u)
)
"""


def rustred_cli() -> Path | None:
    configured = os.environ.get("RUSTRED_CLI")
    if configured:
        path = Path(configured)
    else:
        path = Path(__file__).resolve().parents[3] / "target" / "debug" / "rustred"
    return path if path.is_file() else None


def with_durable_schema(artifact: bytes, schema: int) -> bytes:
    crafted = bytearray(artifact)
    crafted[8:12] = schema.to_bytes(4, "little")
    return bytes(crafted)


def with_durable_arity(artifact: bytes, arity: int) -> bytes:
    crafted = bytearray(artifact)
    metadata_section_offset = 16
    assert int.from_bytes(
        crafted[metadata_section_offset : metadata_section_offset + 2],
        "little",
    ) == 1
    metadata_payload_offset = metadata_section_offset + 2 + 8
    algorithm_bytes = int.from_bytes(
        crafted[metadata_payload_offset : metadata_payload_offset + 8],
        "little",
    )
    arity_offset = metadata_payload_offset + 8 + algorithm_bytes
    crafted[arity_offset : arity_offset + 8] = arity.to_bytes(8, "little")
    return bytes(crafted)


def fresh_local_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


def cli_toml(arguments: list[str], source: str) -> str:
    cli = rustred_cli()
    if cli is None:
        raise unittest.SkipTest("set RUSTRED_CLI to run Python/CLI parity tests")
    completed = subprocess.run(
        [str(cli), *arguments],
        input=source,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        env={**os.environ, "SYMBOLICA_HIDE_BANNER": "1"},
    )
    if completed.returncode != 0:
        raise AssertionError(f"RustRed CLI failed: {completed.stderr}")
    if completed.stderr:
        raise AssertionError(f"successful CLI emitted diagnostics: {completed.stderr}")
    return completed.stdout


def cli_bytes(arguments: list[str], source: bytes = b"") -> bytes:
    cli = rustred_cli()
    if cli is None:
        raise unittest.SkipTest("set RUSTRED_CLI to run Python/CLI parity tests")
    completed = subprocess.run(
        [str(cli), *arguments],
        input=source,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        env={**os.environ, "SYMBOLICA_HIDE_BANNER": "1"},
    )
    if completed.returncode != 0:
        raise AssertionError(
            f"RustRed CLI failed: {completed.stderr.decode(errors='replace')}"
        )
    if completed.stderr:
        raise AssertionError(
            "successful CLI emitted diagnostics: "
            + completed.stderr.decode(errors="replace")
        )
    return completed.stdout


class PythonApiTests(unittest.TestCase):
    def test_repository_two_loop_example_generates_and_applies_closed_artifact(
        self,
    ) -> None:
        example = (
            Path(__file__).resolve().parents[3]
            / "examples"
            / "python"
            / "two_loop_single_mass_vacuum.py"
        )
        if not example.is_file():
            repository_root = Path(__file__).resolve().parents[3]
            if (repository_root / ".git").exists():
                self.fail("Git checkout is missing the documented Python example")
            self.skipTest("repository examples are not included in source distributions")
        specification = importlib.util.spec_from_file_location(
            "rustred_two_loop_single_mass_vacuum_example",
            example,
        )
        self.assertIsNotNone(specification)
        self.assertIsNotNone(specification.loader)
        module = importlib.util.module_from_spec(specification)
        specification.loader.exec_module(module)

        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            module.main()
        payload = output.getvalue()
        reduction_marker = (
            'schema = "rustred.closing-artifact-reduce-output.toml.v1"'
        )
        reduction_offset = payload.index(reduction_marker)
        generation = tomllib.loads(payload[:reduction_offset])
        reduction = tomllib.loads(payload[reduction_offset:])

        self.assertEqual(generation["family_selector"], "unit-mass-vacuum-k3")
        self.assertEqual(generation["validation"]["source_rows"], 4)
        self.assertEqual(generation["validation"]["guarded_rules"], 5)
        self.assertEqual(len(generation["rules"]), 5)
        self.assertEqual(reduction["target"]["powers"], [2, 2, 1])
        self.assertEqual(
            {
                tuple(term["master"]["powers"]): term["common_mass_squared_power"]
                for term in reduction["terms"]
            },
            {(0, 1, 1): "-3", (1, 1, 1): "-2"},
        )

    def test_all_derive_input_modes_return_canonical_toml(self) -> None:
        cases = [
            (ONE_LOOP, "symbolica", "raw_symbolica"),
            (HYBRID, "toml", "hybrid_toml"),
            (EXPLICIT, "toml", "explicit_toml"),
        ]
        for source, input_format, detected in cases:
            with self.subTest(input_format=detected):
                result = rustred.derive(
                    source,
                    input_format=input_format,
                    relations="ordinary",
                    n_cores=1,
                )
                payload = result.to_toml()
                self.assertEqual(result.schema, "rustred.derive-output.toml.v1")
                self.assertEqual(result.status, "ok")
                self.assertTrue(payload.endswith("\n"))
                self.assertIn(f'detected_input_form = "{detected}"', payload)
                self.assertLess(len(repr(result)), 160)

    def test_python_cli_bytes_match_every_input_mode_and_filter(self) -> None:
        cases = [
            (ONE_LOOP, rustred.InputFormat.SYMBOLICA),
            (ONE_LOOP, rustred.InputFormat.AUTO),
            (HYBRID, rustred.InputFormat.TOML),
            (HYBRID, rustred.InputFormat.AUTO),
            (EXPLICIT, rustred.InputFormat.TOML),
            (EXPLICIT, rustred.InputFormat.AUTO),
        ]
        selections = [
            rustred.RelationSelection.ALL,
            rustred.RelationSelection.ORDINARY,
            rustred.RelationSelection.LORENTZ_INVARIANCE,
        ]
        for source, input_format in cases:
            for relations in selections:
                with self.subTest(input_format=input_format, relations=relations):
                    python_toml = rustred.derive(
                        source,
                        input_format=input_format,
                        relations=relations,
                        n_cores=1,
                    ).to_toml()
                    command_toml = cli_toml(
                        [
                            "derive",
                            "--input",
                            "-",
                            "--input-format",
                            str(input_format),
                            "--relations",
                            str(relations),
                            "--n-cores",
                            "1",
                        ],
                        source,
                    )
                    self.assertEqual(python_toml.encode(), command_toml.encode())

    def test_generic_non_vacuum_external_kinematics(self) -> None:
        result = rustred.derive(
            NON_VACUUM,
            input_format=rustred.InputFormat.SYMBOLICA,
            relations=rustred.RelationSelection.ALL,
        )
        payload = result.to_toml()
        self.assertIn('name = "one_loop_two_externals"', payload)
        self.assertIn('left = "p"', payload)
        self.assertIn('right = "q"', payload)
        self.assertIn('kind = "lorentz_invariance"', payload)

    def test_campaign_operations_and_raw_root_mode(self) -> None:
        planned = rustred.campaign_plan(
            ONE_LOOP,
            input_format="symbolica",
            root_id="tadpole",
        )
        self.assertEqual(planned.schema, "rustred.campaign-plan-output.toml.v1")
        self.assertIn('id = "tadpole"', planned.to_toml())

        preflight = rustred.campaign_preflight(
            PROFILE,
            n_cores=4,
            max_memory_bytes=900,
        )
        self.assertEqual(
            preflight.schema,
            "rustred.campaign-execution-preflight-output.toml.v1",
        )
        self.assertEqual(preflight.status, "ready")
        self.assertTrue(preflight.to_toml().endswith("\n"))
        self.assertEqual(
            planned.to_toml(),
            cli_toml(
                [
                    "campaign",
                    "plan",
                    "--input",
                    "-",
                    "--input-format",
                    "symbolica",
                    "--root-id",
                    "tadpole",
                ],
                ONE_LOOP,
            ),
        )
        self.assertEqual(
            preflight.to_toml(),
            cli_toml(
                [
                    "campaign",
                    "preflight",
                    "--profile",
                    "-",
                    "--n-cores",
                    "4",
                    "--max-memory",
                    "900B",
                ],
                PROFILE,
            ),
        )
        for input_format in (
            rustred.InputFormat.TOML,
            rustred.InputFormat.AUTO,
        ):
            with self.subTest(campaign_input_format=input_format):
                toml_plan = rustred.campaign_plan(
                    CAMPAIGN,
                    input_format=input_format,
                )
                self.assertEqual(
                    toml_plan.to_toml(),
                    cli_toml(
                        [
                            "campaign",
                            "plan",
                            "--input",
                            "-",
                            "--input-format",
                            str(input_format),
                        ],
                        CAMPAIGN,
                    ),
                )

    def test_closing_artifact_generation_inspection_and_reduction(self) -> None:
        generated = rustred.generate_closing_artifact(
            family=rustred.ClosingFamily.UNIT_MASS_VACUUM_K1,
        )
        self.assertEqual(
            generated.schema,
            "rustred.closing-artifact-generate-output.toml.v1",
        )
        self.assertEqual(generated.status, "generated-durable")
        self.assertIsInstance(generated.artifact, bytes)
        self.assertTrue(generated.artifact)
        generated_document = tomllib.loads(generated.to_toml())
        self.assertTrue(generated_document["lifecycle"]["durable"])
        self.assertEqual(
            generated_document["payload"]["bytes"],
            len(generated.artifact),
        )
        self.assertEqual(generated_document["validation"]["source_rows"], 1)
        self.assertEqual(generated_document["validation"]["guarded_rules"], 1)

        inspected = rustred.inspect_closing_artifact(generated.artifact)
        self.assertEqual(inspected.status, "inspected")
        self.assertIn(
            "decoded-authenticated-durable-bytes",
            inspected.to_toml(),
        )

        reduced = rustred.reduce_with_closing_artifact(
            generated.artifact,
            [3],
        )
        self.assertEqual(reduced.status, "reduced")
        self.assertEqual(reduced.target_powers, [3])
        self.assertEqual(len(reduced.terms), 1)
        term = reduced.terms[0]
        self.assertEqual(term.master_powers, [1])
        self.assertEqual(
            term.unit_mass_coefficient,
            "(-6*rustred::{}::d+8+rustred::{}::d^2)*1/8",
        )
        self.assertEqual(term.common_mass_squared_power, -2)
        reduced_document = tomllib.loads(reduced.to_toml())
        self.assertEqual(reduced_document["terms"][0]["master"]["powers"], [1])
        self.assertEqual(
            reduced_document["terms"][0]["unit_mass_coefficient"],
            term.unit_mass_coefficient,
        )
        self.assertEqual(
            reduced_document["terms"][0]["common_mass_squared_power"],
            "-2",
        )
        self.assertEqual(
            reduced_document["terms"][0]["common_mass_squared_factor"],
            "mass_squared^(-2)",
        )

    def test_closing_artifact_python_cli_parity(self) -> None:
        selector = "unit-mass-vacuum-k1"
        generated = rustred.generate_closing_artifact(family=selector)
        self.assertEqual(
            generated.artifact,
            cli_bytes(["campaign", "generate", "--family", selector]),
        )
        inspected = rustred.inspect_closing_artifact(generated.artifact)
        self.assertEqual(
            inspected.to_toml(),
            cli_bytes(
                ["campaign", "inspect", "--artifact", "-"],
                generated.artifact,
            ).decode(),
        )
        reduced = rustred.reduce_with_closing_artifact(generated.artifact, [3])
        self.assertEqual(
            reduced.to_toml(),
            cli_bytes(
                [
                    "campaign",
                    "reduce",
                    "--artifact",
                    "-",
                    "--powers",
                    "3",
                ],
                generated.artifact,
            ).decode(),
        )

    def test_two_loop_closing_artifact_is_available_from_import_rustred(self) -> None:
        selector = rustred.ClosingFamily.UNIT_MASS_VACUUM_K3
        self.assertEqual(selector, "unit-mass-vacuum-k3")
        generated = rustred.generate_closing_artifact(family=selector)
        document = tomllib.loads(generated.to_toml())
        self.assertEqual(document["artifact"]["schema"], "rustred.closing-artifact.v2")
        self.assertEqual(document["artifact"]["schema_version"], 2)
        self.assertEqual(document["family_selector"], "unit-mass-vacuum-k3")
        self.assertEqual(document["validation"]["source_rows"], 4)
        self.assertEqual(document["validation"]["guarded_rules"], 5)

        reduced = rustred.reduce_with_closing_artifact(
            generated.artifact,
            [2, 2, 1],
        )
        self.assertEqual(reduced.target_powers, [2, 2, 1])
        self.assertEqual(len(reduced.terms), 2)
        powers = {
            tuple(term.master_powers): term.common_mass_squared_power
            for term in reduced.terms
        }
        self.assertEqual(powers[(1, 1, 1)], -2)
        self.assertEqual(powers[(0, 1, 1)], -3)

    def test_closing_artifact_errors_and_resource_ceiling_are_typed(self) -> None:
        with self.assertRaises(rustred.RustRedInputError):
            rustred.generate_closing_artifact(family="I1L")
        generated = rustred.generate_closing_artifact()
        with self.assertRaises(rustred.RustRedInputError):
            rustred.inspect_closing_artifact(b"invalid artifact")
        with self.assertRaises(rustred.RustRedSchemaError):
            rustred.inspect_closing_artifact(
                with_durable_schema(generated.artifact, 3)
            )
        with self.assertRaises(rustred.RustRedSchemaError):
            rustred.inspect_closing_artifact(
                with_durable_schema(generated.artifact, 1)
            )
        with self.assertRaises(rustred.RustRedLimitError):
            rustred.inspect_closing_artifact(
                with_durable_arity(generated.artifact, (1 << 64) - 1)
            )
        with self.assertRaises(TypeError):
            rustred.inspect_closing_artifact("not bytes")
        with self.assertRaises(rustred.RustRedInputError):
            rustred.reduce_with_closing_artifact(generated.artifact, [])
        with self.assertRaises(rustred.RustRedInputError):
            rustred.reduce_with_closing_artifact(generated.artifact, [True])
        with self.assertRaises(rustred.RustRedInputError):
            rustred.reduce_with_closing_artifact(generated.artifact, [1 << 100])
        with self.assertRaises(rustred.RustRedInputError):
            rustred.reduce_with_closing_artifact(
                generated.artifact,
                [3],
                max_rule_applications=-1,
            )
        with self.assertRaises(rustred.RustRedLimitError):
            rustred.reduce_with_closing_artifact(
                generated.artifact,
                [1],
                max_rule_applications=1_000_001,
            )
        with self.assertRaises(rustred.RustRedLimitError) as exhausted:
            rustred.reduce_with_closing_artifact(
                generated.artifact,
                [3],
                max_rule_applications=1,
            )
        self.assertIn("configured limit 1", str(exhausted.exception))

    def test_python_errors_are_typed(self) -> None:
        with self.assertRaises(rustred.RustRedInputError):
            rustred.derive(ONE_LOOP, input_format="json")
        with self.assertRaises(rustred.RustRedInputError):
            rustred.derive(ONE_LOOP, relations="laporta")
        with self.assertRaises(rustred.RustRedInputError):
            rustred.derive(ONE_LOOP, n_cores=0)
        with self.assertRaises(rustred.RustRedInputError):
            rustred.derive(ONE_LOOP, n_cores=True)
        with self.assertRaises(rustred.RustRedInputError):
            rustred.derive(ONE_LOOP, n_cores=1 << 200)
        with self.assertRaises(rustred.RustRedInputError):
            rustred.campaign_preflight(
                PROFILE,
                n_cores=1,
                max_memory_bytes=0,
            )
        with self.assertRaises(rustred.RustRedInputError):
            rustred.campaign_preflight(
                PROFILE,
                n_cores=True,
                max_memory_bytes=1,
            )
        with self.assertRaises(rustred.RustRedInputError):
            rustred.campaign_preflight(
                PROFILE,
                n_cores=1,
                max_memory_bytes=True,
            )
        with self.assertRaises(rustred.RustRedInputError):
            rustred.campaign_preflight(
                PROFILE,
                n_cores=1,
                max_memory_bytes=1 << 200,
            )

    def test_schema_lowering_and_limit_failures_match_the_cli(self) -> None:
        wrong_schema = HYBRID.replace(
            "rustred.project.toml.v1",
            "rustred.project.toml.unsupported",
            1,
        )
        nonlinear_denominator = ONE_LOOP.replace("k^2-m2", "(k^2)^2", 1)
        oversized_metadata = (
            HYBRID
            + "\n[metadata]\ndescription = \""
            + "x" * (64 * 1024 + 1)
            + "\"\n"
        )
        cases = (
            (rustred.RustRedSchemaError, wrong_schema, "toml"),
            (rustred.RustRedLoweringError, nonlinear_denominator, "symbolica"),
            (rustred.RustRedLimitError, oversized_metadata, "toml"),
        )
        cli = rustred_cli()
        for exception_type, source, input_format in cases:
            with self.subTest(exception_type=exception_type.__name__):
                with self.assertRaises(exception_type) as python_failure:
                    rustred.derive(source, input_format=input_format)
                if cli is None:
                    continue
                completed = subprocess.run(
                    [
                        str(cli),
                        "derive",
                        "--input",
                        "-",
                        "--input-format",
                        input_format,
                        "--relations",
                        "all",
                        "--n-cores",
                        "1",
                    ],
                    input=source,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    check=False,
                    env={**os.environ, "SYMBOLICA_HIDE_BANNER": "1"},
                )
                self.assertEqual(completed.returncode, 4)
                self.assertEqual(completed.stdout, "")
                self.assertEqual(
                    completed.stderr,
                    f"rustred: input: {python_failure.exception}\n",
                )

    def test_root_id_validation_is_owned_by_the_application(self) -> None:
        boundary_root_id = "r" * (4 * 1024 + 1)
        with self.assertRaises(rustred.RustRedInputError) as python_failure:
            rustred.campaign_plan(
                ONE_LOOP,
                input_format="symbolica",
                root_id=boundary_root_id,
            )
        self.assertNotIsInstance(python_failure.exception, rustred.RustRedLimitError)

        cli = rustred_cli()
        if cli is not None:
            completed = subprocess.run(
                [
                    str(cli),
                    "campaign",
                    "plan",
                    "--input",
                    "-",
                    "--input-format",
                    "symbolica",
                    "--root-id",
                    boundary_root_id,
                ],
                input=ONE_LOOP,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
                env={**os.environ, "SYMBOLICA_HIDE_BANNER": "1"},
            )
            self.assertEqual(completed.returncode, 4)
            self.assertEqual(
                completed.stderr,
                f"rustred: input: {python_failure.exception}\n",
            )

        larger_than_source_limit = "r" * (16 * 1024 * 1024 + 1)
        with self.assertRaises(rustred.RustRedInputError) as large_failure:
            rustred.campaign_plan(
                ONE_LOOP,
                input_format="symbolica",
                root_id=larger_than_source_limit,
            )
        self.assertNotIsInstance(large_failure.exception, rustred.RustRedLimitError)
        self.assertIn("campaign root identifier bytes", str(large_failure.exception))

    def test_exception_module_identity_and_pickle_round_trip(self) -> None:
        result_types = (
            rustred.DeriveResult,
            rustred.CampaignPlanResult,
            rustred.CampaignPreflightResult,
        )
        exception_types = (
            rustred.RustRedError,
            rustred.RustRedInputError,
            rustred.RustRedSchemaError,
            rustred.RustRedLimitError,
            rustred.RustRedLoweringError,
            rustred.RustRedDerivationError,
            rustred.RustRedExecutionError,
            rustred.RustRedLicenseError,
            rustred.RustRedSerializationError,
            rustred.RustRedOutputLimitError,
            rustred.RustRedInternalError,
            rustred.RustRedCoordinatorPoisonedError,
        )
        for result_type in result_types:
            with self.subTest(result_type=result_type.__name__):
                self.assertEqual(result_type.__module__, "rustred")
        for exception_type in exception_types:
            with self.subTest(exception_type=exception_type.__name__):
                self.assertEqual(exception_type.__module__, "rustred")
                original = exception_type("round-trip marker")
                restored = pickle.loads(pickle.dumps(original))
                self.assertIs(type(restored), exception_type)
                self.assertEqual(restored.args, original.args)

        self.assertIsNone(importlib.util.find_spec("_rustred"))
        top_level_import = subprocess.run(
            [sys.executable, "-c", "import _rustred"],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        self.assertNotEqual(top_level_import.returncode, 0)

    def test_public_signatures_and_omitted_defaults(self) -> None:
        self.assertEqual(
            str(inspect.signature(rustred.derive)),
            "(source, *, input_format='auto', relations='all', n_cores=1)",
        )
        self.assertEqual(
            str(inspect.signature(rustred.campaign_plan)),
            "(source, *, input_format='auto', root_id=None)",
        )
        self.assertEqual(
            str(inspect.signature(rustred.campaign_preflight)),
            "(profile, *, n_cores=1, max_memory_bytes)",
        )

        self.assertEqual(
            rustred.derive(ONE_LOOP, input_format="symbolica").to_toml(),
            rustred.derive(
                ONE_LOOP,
                input_format="symbolica",
                relations="all",
                n_cores=1,
            ).to_toml(),
        )
        self.assertEqual(
            rustred.campaign_plan(CAMPAIGN).to_toml(),
            rustred.campaign_plan(
                CAMPAIGN,
                input_format="auto",
                root_id=None,
            ).to_toml(),
        )
        self.assertEqual(
            rustred.campaign_preflight(
                PROFILE,
                max_memory_bytes=900,
            ).to_toml(),
            rustred.campaign_preflight(
                PROFILE,
                n_cores=1,
                max_memory_bytes=900,
            ).to_toml(),
        )

    def test_released_gil_allows_another_python_thread_to_run(self) -> None:
        running = True
        started = threading.Event()
        ticks = 0

        def ticker() -> None:
            nonlocal ticks
            started.set()
            while running:
                ticks += 1

        thread = threading.Thread(target=ticker)
        thread.start()
        started.wait()
        before = ticks
        try:
            # The application call is routed to the Rust coordinator while
            # this calling thread is detached from the interpreter.
            rustred.derive(
                ONE_LOOP,
                input_format="symbolica",
                relations="all",
                n_cores=1,
            )
        finally:
            running = False
            thread.join()
        self.assertGreater(ticks, before)

    def test_concurrent_python_callers_receive_deterministic_serialized_results(self) -> None:
        def derive_once(_: int) -> str:
            return rustred.derive(
                ONE_LOOP,
                input_format="symbolica",
                relations="all",
                n_cores=1,
            ).to_toml()

        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            outputs = list(executor.map(derive_once, range(8)))
        self.assertTrue(outputs)
        self.assertTrue(all(output == outputs[0] for output in outputs))

    @unittest.skipUnless(
        os.environ.get("SYMBOLICA_LICENSE") and (os.cpu_count() or 0) >= 4,
        "licensed four-core test",
    )
    def test_licensed_core_widths_are_byte_identical(self) -> None:
        outputs = [
            rustred.derive(
                NON_VACUUM,
                input_format="symbolica",
                relations="all",
                n_cores=n_cores,
            ).to_toml()
            for n_cores in (1, 2, 4)
        ]
        self.assertEqual(outputs[0], outputs[1])
        self.assertEqual(outputs[0], outputs[2])

    def test_unlicensed_multicore_failure_uses_typed_license_error(self) -> None:
        script = f'''import rustred
source = {ONE_LOOP!r}
try:
    rustred.derive(source, input_format="symbolica", n_cores=2)
except rustred.RustRedLicenseError as error:
    print(error)
    raise SystemExit(0)
except BaseException as error:
    print(type(error).__name__, error)
    raise SystemExit(2)
raise SystemExit(3)
'''
        environment = dict(os.environ)
        environment.pop("SYMBOLICA_LICENSE", None)
        environment["SYMBOLICA_HIDE_BANNER"] = "1"
        # The restricted manager may bind its port during parsing/lowering,
        # before RustRed reports that multicore execution needs a license.
        # Give this fresh probe a private best-effort port instead of inheriting
        # the machine-wide Symbolica default.
        environment["SYMBOLICA_PORT"] = str(fresh_local_port())
        completed = subprocess.run(
            [sys.executable, "-c", script],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=environment,
            timeout=60,
            check=False,
        )
        self.assertEqual(
            completed.returncode,
            0,
            f"stdout={completed.stdout!r} stderr={completed.stderr!r}",
        )
        self.assertEqual(completed.stderr, "")
        python_message = completed.stdout.removesuffix("\n")
        self.assertTrue(python_message)

        cli = rustred_cli()
        if cli is not None:
            cli_environment = dict(environment)
            cli_environment["SYMBOLICA_PORT"] = str(fresh_local_port())
            cli_failure = subprocess.run(
                [
                    str(cli),
                    "derive",
                    "--input",
                    "-",
                    "--input-format",
                    "symbolica",
                    "--n-cores",
                    "2",
                ],
                input=ONE_LOOP,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env=cli_environment,
                timeout=60,
                check=False,
            )
            self.assertEqual(cli_failure.returncode, 8)
            self.assertEqual(cli_failure.stdout, "")
            self.assertEqual(
                cli_failure.stderr,
                f"rustred: execution: {python_message}\n",
            )

    @unittest.skipUnless(hasattr(os, "fork"), "requires os.fork")
    def test_post_fork_calls_fail_closed_instead_of_hanging(self) -> None:
        script = f'''import os
import rustred
profile = {PROFILE!r}
child = os.fork()
if child == 0:
    try:
        rustred.campaign_preflight(profile, max_memory_bytes=900)
    except rustred.RustRedCoordinatorPoisonedError:
        os._exit(0)
    except BaseException:
        os._exit(2)
    os._exit(3)
_, status = os.waitpid(child, 0)
raise SystemExit(os.waitstatus_to_exitcode(status))
'''
        completed = subprocess.run(
            [sys.executable, "-c", script],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env={**os.environ, "SYMBOLICA_HIDE_BANNER": "1"},
            timeout=30,
            check=False,
        )
        self.assertEqual(
            completed.returncode,
            0,
            f"stdout={completed.stdout!r} stderr={completed.stderr!r}",
        )


if __name__ == "__main__":
    unittest.main()
