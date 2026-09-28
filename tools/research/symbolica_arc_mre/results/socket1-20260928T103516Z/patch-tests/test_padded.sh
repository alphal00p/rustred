#!/usr/bin/env bash
set -uo pipefail
B=/common/dev/rustred/TMP/symbolica-arc-mre-builds
cd /common/dev/rustred/TMP/symbolica-upstream-mre-padded
awk '/MemAvailable/ {if ($2/1048576 < 150) exit 1}' /proc/meminfo || { echo "low mem" >> $B/logs/round2.log; exit 1; }
echo "$(date -u +%FT%TZ) test padded (separate target dir; the previous padded run reused the patched test binary)" >> $B/logs/round2.log
CARGO_TARGET_DIR=$B/target-test-padded flock -w 14400 /common/dev/rustred/TMP/locks/build-3.lock nice -n 5 taskset -c 88-127 \
  env TMPDIR=/common/dev/rustred/TMP nix develop /common/dev/rustred --command \
  cargo test --offline --release --no-default-features --features "integer-gmp float-mpfr native_code_generation" --lib -- poly:: \
  > $B/logs/test-padded2.log 2>&1
trc=$?
echo "$(date -u +%FT%TZ) test padded2 rc=$trc $(grep -E '^test result' $B/logs/test-padded2.log | tail -1)" >> $B/logs/round2.log
