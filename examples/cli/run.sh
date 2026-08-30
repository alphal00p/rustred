#!/bin/sh
set -eu

example_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
repository_root=$(CDPATH= cd "$example_dir/../.." && pwd)
temporary_directory=$(mktemp -d)
artifact="$temporary_directory/two_loop_sunset.rr"
cleanup() {
  rm -f "$artifact"
  rmdir "$temporary_directory"
}
trap cleanup EXIT HUP INT TERM

cd "$repository_root"
cargo run --locked -p rustred-app --bin rustred -- campaign generate \
  --family unit-mass-vacuum-k3 \
  --output "$artifact"
cargo run --locked -p rustred-app --bin rustred -- campaign inspect \
  --artifact "$artifact" \
  --output -
cargo run --locked -p rustred-app --bin rustred -- campaign reduce \
  --artifact "$artifact" \
  --powers 2,2,1 \
  --output -
