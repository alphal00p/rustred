from __future__ import annotations

import sys
import tarfile
from pathlib import Path
import zipfile


FORBIDDEN_COMPONENT = "FOR_REFERENCE_ONLY_DO_NOT_PUSH"
FORBIDDEN_DOCUMENTS = {"GOAL.md", "HANDOFF.md"}
REQUIRED_SDIST_SUFFIXES = (
    "Cargo.toml",
    "crates/rustred-core/Cargo.toml",
    "crates/rustred-app/Cargo.toml",
    "crates/rustred-python/Cargo.toml",
    "vendor/symbolica/Cargo.toml",
    "vendor/symbolica/build.rs",
    "vendor/symbolica/src/lib.rs",
    "vendor/symbolica/lib/graphica/Cargo.toml",
    "vendor/symbolica/lib/numerica/Cargo.toml",
)
PRIVATE_CLASSIFIER = "Classifier: Private :: Do Not Upload"
RELEASE_PROHIBITION = "Do not publish the current sdist."


def archive_names(path: Path) -> list[str]:
    if path.suffix == ".whl" or zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            return archive.namelist()
    if path.name.endswith((".tar.gz", ".tar.bz2", ".tar.xz")):
        with tarfile.open(path) as archive:
            return archive.getnames()
    raise ValueError(f"unsupported distribution archive: {path}")


def metadata_text(path: Path) -> str:
    if path.suffix == ".whl" or zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            matches = [
                name for name in archive.namelist() if name.endswith(".dist-info/METADATA")
            ]
            if len(matches) != 1:
                raise ValueError(f"{path}: expected exactly one wheel METADATA file")
            return archive.read(matches[0]).decode("utf-8")
    if path.name.endswith((".tar.gz", ".tar.bz2", ".tar.xz")):
        with tarfile.open(path) as archive:
            matches = [
                member
                for member in archive.getmembers()
                if member.name.endswith("/PKG-INFO") and member.isfile()
            ]
            if len(matches) != 1:
                raise ValueError(f"{path}: expected exactly one sdist PKG-INFO file")
            extracted = archive.extractfile(matches[0])
            if extracted is None:
                raise ValueError(f"{path}: cannot read sdist PKG-INFO")
            return extracted.read().decode("utf-8")
    raise ValueError(f"unsupported distribution archive: {path}")


def main(arguments: list[str]) -> int:
    if not arguments:
        raise SystemExit("usage: assert_distribution_contents.py ARCHIVE [...]")
    for raw_path in arguments:
        path = Path(raw_path)
        names = archive_names(path)
        metadata = metadata_text(path)
        if PRIVATE_CLASSIFIER not in metadata:
            print(f"{path}: distribution metadata omits private classifier")
            return 1
        if RELEASE_PROHIBITION not in metadata:
            print(f"{path}: distribution metadata omits release prohibition")
            return 1
        forbidden = [
            name
            for name in names
            if FORBIDDEN_COMPONENT in Path(name).parts
            or Path(name).name in FORBIDDEN_DOCUMENTS
            or "docs/research" in Path(name).as_posix()
        ]
        if forbidden:
            print(f"{path}: forbidden reference-only payloads: {forbidden[:10]}")
            return 1
        if path.suffix == ".whl" and not any(
            name.endswith("rustred/py.typed") for name in names
        ):
            print(f"{path}: wheel omits rustred/py.typed")
            return 1
        if path.name.endswith((".tar.gz", ".tar.bz2", ".tar.xz")):
            root_build_scripts = [
                name
                for name in names
                if len(Path(name).parts) == 2 and Path(name).name == "build.rs"
            ]
            if root_build_scripts:
                print(f"{path}: sdist contains a forbidden root build.rs")
                return 1
            missing = [
                suffix
                for suffix in REQUIRED_SDIST_SUFFIXES
                if not any(name.endswith(suffix) for name in names)
            ]
            if missing:
                print(f"{path}: sdist omits required build inputs: {missing}")
                return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
