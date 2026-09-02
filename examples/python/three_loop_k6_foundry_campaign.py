"""Run either pure-RustRed K=6 foundry lane from a release Python build."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import sys
import tempfile
import tomllib

import rustred


EXAMPLES = Path(__file__).resolve().parents[1]
CONFIGS = {
    "external-hints": EXAMPLES / "k6_external_search_hints.toml",
    "autonomous": EXAMPLES / "k6_autonomous_campaign.toml",
}


def persist_artifact(path: Path, artifact: bytes) -> None:
    """Atomically install durable bytes without replacing an existing path."""
    descriptor, staging_name = tempfile.mkstemp(
        dir=path.parent,
        prefix=f".{path.name}.rustred-tmp-",
    )
    staging = Path(staging_name)
    try:
        with os.fdopen(descriptor, "wb") as destination:
            written = destination.write(artifact)
            if written != len(artifact):
                raise OSError(
                    f"short artifact write: wrote {written} of {len(artifact)} bytes"
                )
            destination.flush()
            os.fsync(destination.fileno())

        # A same-directory hard link is an atomic create-if-absent install. It
        # cannot replace a destination which appears while staging is written.
        os.link(staging, path)
        staging.unlink()
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        try:
            staging.unlink()
        except FileNotFoundError:
            pass


def preflight_outputs(paths: list[Path | None]) -> None:
    """Reject ambiguous or non-fresh destinations before expensive work."""
    requested = [path for path in paths if path is not None]
    resolved = [path.resolve(strict=False) for path in requested]
    if len(set(resolved)) != len(resolved):
        raise ValueError("output, measurement, and artifact paths must be distinct")
    for path in requested:
        if path.exists():
            raise FileExistsError(f"output path already exists: {path}")
        if not path.parent.is_dir():
            raise FileNotFoundError(f"output parent directory does not exist: {path.parent}")


def persist_text(path: Path, document: str) -> None:
    """Atomically install one UTF-8 TOML document without replacing a path."""
    persist_artifact(path, document.encode("utf-8"))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode",
        choices=tuple(CONFIGS),
        default="external-hints",
        help="choose the reviewed-hint lane or the strictly hint-free lane",
    )
    parser.add_argument("--n-cores", type=int, default=1)
    parser.add_argument(
        "--output",
        type=Path,
        help="write the deterministic semantic report as one TOML document",
    )
    parser.add_argument(
        "--measurements-output",
        type=Path,
        help="write the nonsemantic wall-clock sidecar as a separate TOML document",
    )
    parser.add_argument(
        "--artifact-output",
        type=Path,
        help=(
            "write canonical durable artifact bytes if and only if the exact "
            "K=6 campaign closes"
        ),
    )
    arguments = parser.parse_args()
    if arguments.n_cores < 1:
        parser.error("--n-cores must be positive")
    preflight_outputs(
        [arguments.output, arguments.measurements_output, arguments.artifact_output]
    )

    config = CONFIGS[arguments.mode].read_text(encoding="utf-8")
    document = tomllib.loads(config)
    if arguments.mode == "external-hints":
        assert document["mode"] == "external-hints-only"
        assert len(document["hints"]["domains"]) == 55
    else:
        assert document["mode"] == "autonomous"
        assert "hints" not in document

    result = rustred.run_foundry_wave_campaign(
        config,
        n_cores=arguments.n_cores,
    )
    persisted_artifact_size = None
    if arguments.artifact_output is not None:
        artifact = result.artifact_bytes
        if artifact is None:
            print(
                "campaign is incomplete; no artifact file was written",
                file=sys.stderr,
            )
        else:
            persist_artifact(arguments.artifact_output, artifact)
            persisted_artifact_size = len(artifact)

    # A report advertising durable publication is exposed only after any
    # requested artifact destination has completed its no-overwrite write,
    # flush, file/directory fsync, and close path. The semantic report and
    # nonsemantic measurements remain separate parseable TOML documents.
    if arguments.measurements_output is not None:
        persist_text(arguments.measurements_output, result.measurements_to_toml())
    report = result.to_toml()
    if arguments.output is None:
        print(report, end="")
    else:
        persist_text(arguments.output, report)
    if persisted_artifact_size is not None:
        print(
            f"wrote {persisted_artifact_size} canonical artifact bytes to "
            f"{arguments.artifact_output}",
            file=sys.stderr,
        )


if __name__ == "__main__":
    main()
