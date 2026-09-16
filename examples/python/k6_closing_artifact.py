"""Generate, inspect and apply the canonical autonomous K=6 artifact.

Generation uses the generic family_close API with an external TOML input.
It is an explicit one-off operation, not repeated during ordinary evaluation.

Typical workflow::

    python examples/python/k6_closing_artifact.py k6.rr --generate --workers 6
    python examples/python/k6_closing_artifact.py k6.rr

The second command consumes the supplied artifact without generating anything.
Inspection and reduction use the public cold-loading APIs. Neither command
invokes FORM, Mathematica, a specialized generator, or external rule hints.
"""

from __future__ import annotations

import argparse
import os
import pathlib
import tomllib

import rustred


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=pathlib.Path, help="K=6 .rr artifact path")
    parser.add_argument(
        "--generate", action="store_true", help="generate first; refuse an existing path"
    )
    parser.add_argument("--workers", type=int, default=1, help="generation workers (default: 1)")
    args = parser.parse_args()
    if args.workers < 1:
        parser.error("--workers must be a positive integer")

    if args.generate:
        if args.artifact.exists() or args.artifact.is_symlink():
            parser.error(f"refusing to overwrite existing artifact: {args.artifact}")
        if not args.artifact.parent.is_dir():
            parser.error("the artifact's parent directory must already exist")
        source = pathlib.Path(__file__).resolve().parents[1] / "input" / "three_loop_k6.toml"
        generated = rustred.family_close(
            source.read_text(encoding="utf-8"), input_format="toml", n_cores=args.workers
        )
        assert generated.status == "generated-durable"
        # Exclusive creation repeats the no-clobber check after generation,
        # so a concurrently created artifact is never overwritten.
        with args.artifact.open("xb") as output:
            output.write(generated.artifact)
            output.flush()
            os.fsync(output.fileno())
        print("[generation]")
        print(generated.to_toml(), end="")

    artifact = args.artifact.read_bytes()
    inspection = rustred.inspect_closing_artifact(artifact)
    metadata = tomllib.loads(inspection.to_toml())
    assert inspection.status == "inspected"
    assert metadata["artifact"]["arity"] == 6
    assert len(metadata["artifact"]["masters"]) == 38

    reduction = rustred.reduce_with_closing_artifact(artifact, [2, 1, 1, 1, 1, 1])
    assert reduction.status == "reduced"
    assert reduction.target_powers == [2, 1, 1, 1, 1, 1]
    assert len(reduction.terms) == 30

    print("[inspection]")
    print(inspection.to_toml(), end="")
    print("[reduction]")
    print(reduction.to_toml(), end="")


if __name__ == "__main__":
    main()
