#!/usr/bin/env bash
# Compatibility entry point. RustRed now requires a licensed Symbolica runtime
# and executes its tests in parallel.

set -euo pipefail

script_dir=$(cd -- "${BASH_SOURCE[0]%/*}" && pwd)
echo "test-serial.sh is deprecated; running the licensed parallel suite." >&2
exec "${script_dir}/test.sh" "$@"
