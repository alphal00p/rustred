#!/bin/sh
set -eu

# Demonstrate the already implemented, autonomous K=6 artifact workflow.  The
# generator and application are ordinary RustRed binaries: once they have been
# built, rerunning this example does not recompile anything.  Set
# RUSTRED_K6_GENERATOR and RUSTRED_BIN to point at existing release binaries
# when experimenting repeatedly.
repository_root=$(CDPATH= cd "$(dirname "$0")/../.." && pwd)
artifact=${1:-"$repository_root/k6.rr"}
workers=${2:-${RUSTRED_K6_WORKERS:-6}}
generator=${RUSTRED_K6_GENERATOR:-"$repository_root/target/release/examples/spired-generate-k6"}
rustred_bin=${RUSTRED_BIN:-"$repository_root/target/release/rustred"}

case "$artifact" in
  /*) : ;;
  *) artifact="$repository_root/$artifact" ;;
esac

if [ -e "$artifact" ]; then
  echo "refusing to overwrite existing artifact: $artifact" >&2
  exit 2
fi
mkdir -p "$(dirname "$artifact")"

if [ -x "$generator" ]; then
  "$generator" "$artifact" "$workers"
else
  (cd "$repository_root" && cargo run --release --locked -p rustred \
    --example spired-generate-k6 -- "$artifact" "$workers")
fi

if [ -x "$rustred_bin" ]; then
  app="$rustred_bin"
else
  app="cargo run --release --locked -p rustred-app --bin rustred --"
fi

if [ -x "$rustred_bin" ]; then
  "$app" campaign inspect --artifact "$artifact" --output -
  "$app" campaign reduce --artifact "$artifact" \
    --powers 2,1,1,1,1,1 --output -
else
  (cd "$repository_root" && $app campaign inspect --artifact "$artifact" --output -)
  (cd "$repository_root" && $app campaign reduce --artifact "$artifact" \
    --powers 2,1,1,1,1,1 --output -)
fi

echo "K=6 artifact written to $artifact"
