#!/usr/bin/env bash
# Run RustRed's suite with a licensed Symbolica instance. cargo-nextest runs
# tests from different binaries concurrently; rustdoc tests follow separately.

set -euo pipefail

script_dir=$(cd -- "${BASH_SOURCE[0]%/*}" && pwd)
repo_dir=$(cd -- "${script_dir}/.." && pwd)
cd -- "${repo_dir}"

if [[ -z "${SYMBOLICA_LICENSE:-}" ]]; then
    echo "SYMBOLICA_LICENSE must be set for the parallel Symbolica test suite." >&2
    exit 2
fi

# Symbolica-heavy integration tests can each retain substantial exact-algebra
# state. Keep the default parallel, but bounded independently of the host CPU
# count; callers may still override it explicitly.
test_jobs=${RUSTRED_TEST_JOBS:-4}

cargo run --quiet --example symbolica_license_check

if command -v cargo-nextest >/dev/null 2>&1; then
    cargo nextest run --workspace --all-targets --test-threads "${test_jobs}"
else
    echo "cargo-nextest is unavailable; Cargo will still parallelize tests within each binary." >&2
    echo "Use 'nix develop' to obtain target-level parallel execution." >&2
    cargo test --workspace --all-targets -- --test-threads "${test_jobs}"
fi

cargo test --workspace --doc -- --test-threads "${test_jobs}"
