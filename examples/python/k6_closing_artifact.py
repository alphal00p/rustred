"""Inspect and apply a completed autonomous K=6 artifact.

This is intentionally an artifact-consumer example: generation is a one-off
release operation and is not repeated when a user evaluates an integral.

Typical workflow::

    cargo run --release --locked -p rustred --example spired-generate-k6 -- k6.rr 6
    python examples/python/k6_closing_artifact.py k6.rr

The Python process only authenticates the supplied bytes once and then applies
RustRed's generic strict-descent reducer.  It does not invoke FORM, Mathematica,
or a topology-specific implementation.
"""

from __future__ import annotations

import argparse
import pathlib
import tomllib

import rustred


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=pathlib.Path, help="completed K=6 .rr artifact")
    args = parser.parse_args()

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
