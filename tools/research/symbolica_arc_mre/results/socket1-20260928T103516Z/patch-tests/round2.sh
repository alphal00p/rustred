#!/usr/bin/env bash
set -uo pipefail
B=/common/dev/rustred/TMP/symbolica-arc-mre-builds
LOG=$B/logs/round2.log
echo "$(date -u +%FT%TZ) round2 start" > $LOG
awk '/MemAvailable/ {if ($2/1048576 < 150) exit 1}' /proc/meminfo || { echo "low mem" >> $LOG; exit 1; }
( cd $B/proposed && CARGO_TARGET_DIR=$B/target-shared flock -w 14400 /common/dev/rustred/TMP/locks/build-3.lock nice -n 5 taskset -c 88-127 \
    env TMPDIR=/common/dev/rustred/TMP nix develop /common/dev/rustred --command cargo build --release --offline --features proposed-api ) > $B/logs/build-proposed2.log 2>&1
rc=$?; echo "$(date -u +%FT%TZ) build proposed2 rc=$rc" >> $LOG
[ $rc -eq 0 ] && cp $B/target-shared/release/symbolica-arc-mre $B/bin/symbolica-arc-mre-proposed2 && sha256sum $B/bin/symbolica-arc-mre-proposed2 >> $LOG
for w in patched padded; do
  cd /common/dev/rustred/TMP/symbolica-upstream-mre-$w || continue
  [ -f Cargo.lock ] || cp /common/dev/rustred/Cargo.lock Cargo.lock
  CARGO_TARGET_DIR=$B/target-test flock -w 14400 /common/dev/rustred/TMP/locks/build-3.lock nice -n 5 taskset -c 88-127 \
    env TMPDIR=/common/dev/rustred/TMP nix develop /common/dev/rustred --command \
    cargo test --offline --release --no-default-features --features "integer-gmp float-mpfr native_code_generation" --lib -- poly:: \
    > $B/logs/test-$w.log 2>&1
  trc=$?
  echo "$(date -u +%FT%TZ) test $w rc=$trc $(grep -E '^test result' $B/logs/test-$w.log | tail -1)" >> $LOG
done
echo "$(date -u +%FT%TZ) round2 done" >> $LOG
