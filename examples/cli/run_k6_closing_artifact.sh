#!/bin/sh
set -eu

# Use one existing generic release CLI for generation, fresh-process inspection
# and reduction. The family is supplied as data, with no specialized generator.
repository_root=$(CDPATH= cd "$(dirname "$0")/../.." && pwd)
artifact=${1:-"$repository_root/k6.rr"}
workers=${2:-${RUSTRED_K6_WORKERS:-6}}
rustred_bin=${RUSTRED_BIN:-"$repository_root/target/release/rustred"}

if [ "$#" -gt 2 ]; then
  echo "usage: $0 [new-artifact.rr] [workers]" >&2
  exit 2
fi
case "$workers" in
  ''|*[!0-9]*|0)
    echo "workers must be a positive integer" >&2
    exit 2
    ;;
esac
if [ ! -x "$rustred_bin" ]; then
  echo "build the generic release CLI first, or set RUSTRED_BIN:" >&2
  echo "cargo build --release --locked -p rustred-app --bin rustred" >&2
  exit 127
fi

case "$artifact" in
  /*) : ;;
  *) artifact="$repository_root/$artifact" ;;
esac

if [ -e "$artifact" ] || [ -L "$artifact" ]; then
  echo "refusing to overwrite existing artifact: $artifact" >&2
  exit 2
fi
mkdir -p "$(dirname "$artifact")"

"$rustred_bin" family-close \
  --input "$repository_root/examples/input/three_loop_k6.toml" \
  --input-format toml --n-cores "$workers" --progress --output "$artifact"
"$rustred_bin" campaign inspect --artifact "$artifact" --output -
"$rustred_bin" campaign reduce --artifact "$artifact" \
  --powers 2,1,1,1,1,1 --output -

echo "K=6 artifact written to $artifact"
