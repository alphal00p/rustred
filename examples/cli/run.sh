#!/bin/sh
set -eu

example_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
repository_root=$(CDPATH= cd "$example_dir/../.." && pwd)

cd "$repository_root"
exec cargo run --locked -p rustred-app --bin rustred -- derive \
  --input "$example_dir/two_loop_single_mass_vacuum.symbolica" \
  --input-format symbolica \
  --relations ordinary \
  --n-cores 1 \
  --output -
