"""Generate Vakint's one- through four-loop rule programs from native IBPs.

This is Python steering of release executables, not a solver or a cached-rule
converter. Without --execute it only prints a plan. Select --loops explicitly;
four-loop generation can be expensive and is never part of the default action.
Build rustred and the normalize_candidate_terminals Cargo example once, outside
the measured run. See the example README for commands and scope distinctions.

Four-loop terminal normalization is generated from exact native symmetries.
Optional --catalog-directory supplies *precomputed* .rrcat.bin.gz value maps;
their exact family/output-key binding is checked, but their values are not
derived here. No FORM, Mathematica, master evaluation, or artifact installation
is performed. No mathematical work or elapsed-time cutoff is added.
--workers controls native concurrency, not RAM; use the caller's resource
envelope when generating larger families. The example installs no AS limit.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]
INPUTS = ROOT / "examples/input"
# Recipe data, not engine dispatch. Physical coordinates precede auxiliary ISPs.
FOUR = {"h": [9], "fg": [8, 9], "bmw": [8, 9], "x": [9]}
LOWER = {1: (1, "unit-mass-vacuum-k1", "3"),
         2: (3, "unit-mass-vacuum-k3", "2,2,1"),
         3: (6, None, "2,1,1,1,1,1")}
GENERATION_IO = ["--bundle-max-bytes", str(1 << 30), "--bundle-max-entries", "8000000",
                 "--bundle-max-total-coefficient-bytes", str(512 << 20)]


def sha(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path: Path, value) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())


def compressed_copy(source: Path, target: Path) -> None:
    """Deterministic transport of freshly generated rules, never a rule source."""
    with source.open("rb") as incoming, target.open("xb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=9) as out:
            shutil.copyfileobj(incoming, out)
        raw.flush()
        os.fsync(raw.fileno())


def decompressed_copy(source: Path, target: Path, cap: int) -> None:
    # Trusted generated provenance remains required; this bounds transport only.
    with gzip.open(source, "rb") as incoming, target.open("xb") as out:
        count = 0
        while data := incoming.read(1 << 20):
            count += len(data)
            if count > cap:
                raise ValueError(f"decompressed input exceeds {cap} bytes: {source}")
            out.write(data)


def plan(args) -> dict:
    if args.workers < 1:
        raise ValueError("workers must be positive")
    if len(set(args.loops)) != len(args.loops):
        raise ValueError("loop selections must be unique")
    if args.families and 4 not in args.loops:
        raise ValueError("--families requires --loops 4")
    if args.catalog_directory and 4 not in args.loops:
        raise ValueError("--catalog-directory requires --loops 4")
    families = args.families or list(FOUR)
    if len(set(families)) != len(families):
        raise ValueError("family selections must be unique")
    out = args.output_directory.resolve()
    cli = str(args.executable.resolve())
    helper = str(args.normalizer.resolve())
    jobs = []
    for loop in sorted(args.loops):
        if loop in LOWER:
            arity, selector, powers = LOWER[loop]
            name = f"unit_mass_vacuum_k{arity}"
            artifact = out / f"{name}.rrbin"
            source = INPUTS / "three_loop_k6.toml" if loop == 3 else None
            generate = ([cli, "campaign", "generate", "--family", selector] if selector else
                        [cli, "family-close", "--input", str(source), "--input-format", "toml",
                         "--n-cores", str(args.workers), "--progress"])
            jobs.append({"name": name, "loop": loop, "kind": "certified", "arity": arity,
                         "artifact": str(artifact), "source": str(source) if source else None,
                         "commands": [generate + ["--output", str(artifact)],
                             [cli, "campaign", "inspect", "--artifact", str(artifact),
                              "--output", str(out / f"{name}.inspection.toml")],
                             [cli, "campaign", "reduce", "--artifact", str(artifact),
                              "--powers", powers, "--output", str(out / f"{name}.reduction.toml")]]})
        else:
            for family in families:
                directory = out / "four_loop"
                source = INPUTS / "vakint" / f"{family}.toml"
                artifact = directory / f"{family}.candidates.rrbin"
                normalization = directory / f"{family}.rrnorm.bin"
                catalog = directory / f"{family}.rrcat.bin" if args.catalog_directory else None
                jobs.append({"name": family, "loop": 4, "kind": "uncertified-candidates",
                    "arity": 10, "artifact": str(artifact), "source": str(source),
                    "nonpositive_indices": FOUR[family], "max_numerator_rank": None,
                    "normalization": str(normalization), "catalog": str(catalog) if catalog else None,
                    "commands": [[cli, "family-candidates", "--input", str(source),
                        "--input-format", "auto", "--nonpositive-indices", ",".join(map(str, FOUR[family])),
                        "--n-cores", str(args.workers), "--exact-backend", "sparse",
                        "--numerical-depth", "2", "--finite-case-policy", "search", "--progress",
                        *GENERATION_IO, "--output", str(artifact), "--report-output",
                        str(directory / f"{family}.generation.toml")],
                        [helper, "generate", "10", str(artifact), str(normalization),
                         str(directory / f"{family}.normalization.json"), str(catalog) if catalog else "-"],
                        [helper, "verify", "10", str(artifact), str(normalization),
                         str(directory / f"{family}.normalization-replay.json"), str(catalog) if catalog else "-"]]})
    return {"schema": "rustred.vakint-artifact-generation-example.v1", "output_directory": str(out),
            "fresh_ibp_generation": True, "rule_payloads_imported_for_generation": False,
            "four_loop_closure_certified": False, "numerical_master_values_generated": False,
            "ordering": "unchanged generation defaults; no permutation override",
            "entry_rank_or_power_bound": None, "elapsed_deadline_seconds": None,
            "nested_thread_pools": 1,
            "jobs": jobs}


def run_command(command: list[str], log: Path) -> dict:
    started = time.monotonic()
    # Secrets remain solely in the inherited environment, never in receipts.
    with log.with_suffix(".stdout").open("xb") as stdout, log.with_suffix(".stderr").open("xb") as stderr:
        environment = dict(os.environ, RAYON_NUM_THREADS="1", OMP_NUM_THREADS="1",
                           OMP_THREAD_LIMIT="1", OPENBLAS_NUM_THREADS="1",
                           MKL_NUM_THREADS="1", BLIS_NUM_THREADS="1")
        result = subprocess.run(command, stdout=stdout, stderr=stderr, check=False, env=environment)
    receipt = {"command": command, "returncode": result.returncode,
               "whole_command_seconds": time.monotonic() - started}
    write_json(log.with_suffix(".json"), receipt)
    if result.returncode:
        raise RuntimeError(f"native command failed ({result.returncode}); preserved {log}.stderr")
    return receipt


def execute(args, prepared: dict) -> None:
    out = Path(prepared["output_directory"])
    executables = {args.executable.resolve()}
    if 4 in args.loops or args.reference_directory:
        executables.add(args.normalizer.resolve())
    for binary in executables:
        if not binary.is_file() or not os.access(binary, os.X_OK):
            raise ValueError(f"build the release executable first: {binary}")
    inputs = {Path(j["source"]) for j in prepared["jobs"] if j["source"]} | executables
    for job in prepared["jobs"]:
        if job.get("catalog"):
            inputs.add(args.catalog_directory.resolve() / f"{job['name']}.rrcat.bin.gz")
        if args.reference_directory:
            relative = (Path("four_loop") / f"{job['name']}.candidates.rrbin.gz" if job["loop"] == 4 else
                        Path(f"{job['name']}.rrbin"))
            inputs.add(args.reference_directory.resolve() / relative)
    before = {str(path): sha(path) for path in sorted(inputs)}
    # No-clobber: no existing output tree, cached rules, or resume fallback.
    out.mkdir(parents=False, exist_ok=False)
    (out / "logs").mkdir()
    if 4 in args.loops:
        (out / "four_loop").mkdir()
    write_json(out / "plan.json", prepared)
    write_json(out / "inputs.json", before)
    completed = []
    try:
        for job in prepared["jobs"]:
            print(f"Generating {job['name']} ({job['kind']}); logs: {out / 'logs'}", flush=True)
            if job.get("catalog"):
                source = args.catalog_directory.resolve() / f"{job['name']}.rrcat.bin.gz"
                decompressed_copy(source, Path(job["catalog"]), 1 << 30)
            for ordinal, command in enumerate(job["commands"]):
                run_command(command, out / "logs" / f"{job['name']}-{ordinal}")
            if job["loop"] == 4:
                report = json.loads((out / "four_loop" / f"{job['name']}.normalization-replay.json").read_text())
                if (report["max_numerator_rank"] is not None or report["numerical_depth"] != 2
                        or report["finite_case_policy"] != "search"):
                    raise RuntimeError("generated candidate policy differs from the unrestricted recipe")
                compressed_copy(Path(job["artifact"]), Path(job["artifact"] + ".gz"))
                if job.get("catalog"):
                    # Only after exact generated output-key validation. These
                    # values remain externally supplied, not generated results.
                    source = args.catalog_directory.resolve() / f"{job['name']}.rrcat.bin.gz"
                    with source.open("rb") as incoming, Path(job["catalog"] + ".gz").open("xb") as target:
                        shutil.copyfileobj(incoming, target)
            else:
                report = tomllib.loads((out / f"{job['name']}.inspection.toml").read_text())
                if report["status"] != "inspected" or report["artifact"]["arity"] != job["arity"]:
                    raise RuntimeError("cold certified-artifact inspection did not match the recipe")
            if args.reference_directory:
                reference = args.reference_directory.resolve() / ("four_loop" if job["loop"] == 4 else "")
                reference /= (f"{job['name']}.candidates.rrbin.gz" if job["loop"] == 4 else f"{job['name']}.rrbin")
                if job["loop"] == 4:
                    expanded = out / "four_loop" / f"{job['name']}.reference.rrbin"
                    decompressed_copy(reference, expanded, 1 << 30)
                    reference = expanded
                run_command([str(args.normalizer.resolve()), "compare", job["artifact"], str(reference),
                             str(out / f"{job['name']}.reference-comparison.json")],
                            out / "logs" / f"{job['name']}-reference")
            completed.append(job["name"])
        if {path: sha(Path(path)) for path in before} != before:
            raise RuntimeError("an executable or input changed during generation")
        write_json(out / "result.json", {"status": "generated-and-cold-checked", "completed": completed,
            "fresh_ibp_generation": True, "numerical_master_values_generated": False,
            "precomputed_catalog_bound": args.catalog_directory is not None,
            "supplied_reference_semantically_equal": bool(args.reference_directory),
            "four_loop_closure_certified": False,
            "files": {str(path.relative_to(out)): sha(path) for path in sorted(out.rglob("*")) if path.is_file()}})
    except Exception as error:
        write_json(out / "failure.json", {"status": "failed", "completed": completed, "error": str(error)})
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--loops", type=int, choices=(1, 2, 3, 4), nargs="+", required=True)
    parser.add_argument("--families", choices=tuple(FOUR), nargs="+", help="optional four-loop subset; default all four")
    parser.add_argument("--output-directory", type=Path, required=True, help="new directory under an existing parent")
    parser.add_argument("--executable", type=Path, default=ROOT / "target/release/rustred")
    parser.add_argument("--normalizer", type=Path, default=ROOT / "target/release/examples/normalize_candidate_terminals")
    parser.add_argument("--workers", type=int, default=1)
    parser.add_argument("--catalog-directory", type=Path, help="trusted precomputed four-loop .rrcat.bin.gz inputs")
    parser.add_argument("--reference-directory", type=Path, help="optional Vakint data/rustred tree for exact native semantic comparison")
    parser.add_argument("--execute", action="store_true", help="generate; without this flag only print the plan")
    args = parser.parse_args()
    try:
        prepared = plan(args)
        if args.execute:
            execute(args, prepared)
        else:
            print(json.dumps(prepared, indent=2))
    except (OSError, ValueError, RuntimeError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
