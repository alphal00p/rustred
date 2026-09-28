#!/usr/bin/env bash
# Driver (not part of the MRE): whole socket-1 session under the shared socket-1 lock.
set -uo pipefail
M=/common/dev/rustred/tools/research/symbolica_arc_mre
SB=/common/dev/rustred/TMP/symbolica-arc-mre-builds/bin
TS=$(date -u +%Y%m%dT%H%M%SZ)
RES=$M/results/socket1-$TS
mkdir -p "$RES"
export PERF=/nix/store/wizn21b9virxqcnm4n89b09pgqkaxfn3-perf-linux-7.2.5/bin/perf
export RUSTC_VERSION="rustc 1.97.1 (8bab26f4f 2026-07-14) (nix develop /common/dev/rustred)"
echo "$(date -u +%FT%TZ) waiting for socket1.lock (first attempt started 10:12Z, cancelled/re-queued 10:35Z)" > "$RES/session.log"
flock -w 3600 /common/dev/rustred/TMP/locks/socket1.lock bash -c '
  RES='"$RES"'; M='"$M"'; SB='"$SB"'
  echo "$(date -u +%FT%TZ) socket1 lock acquired" >> "$RES/session.log"
  cd "$M"
  OUT=$RES/sweep FIRST_CPU=128 KS="1 8 24 48 96" REPEATS=2 OPS=40000 nice -n 5 ./scripts/run.sh >> "$RES/session.log" 2>&1
  echo "$(date -u +%FT%TZ) sweep done" >> "$RES/session.log"
  for spec in "shared full" "private full" "private split" "shared split"; do
    set -- $spec
    OUT=$RES/profile VARIANT=$1 WORK=$2 K=96 FIRST_CPU=128 OPS=15000 nice -n 5 ./scripts/profile.sh >> "$RES/session.log" 2>&1
  done
  echo "$(date -u +%FT%TZ) profiles done" >> "$RES/session.log"
  BIN=$SB/symbolica-arc-mre-upstream OUT=$RES/upstream-445b882d FIRST_CPU=128 KS="1 96" VARIANTS="shared private rehome" WORKS="full" REPEATS=2 OPS=40000 nice -n 5 ./scripts/run.sh >> "$RES/session.log" 2>&1
  BIN=$SB/symbolica-arc-mre-proposed OUT=$RES/patched-proposed FIRST_CPU=128 KS="1 24 96" VARIANTS="shared rehome rehome-api private" WORKS="full specialize split" REPEATS=2 OPS=40000 nice -n 5 ./scripts/run.sh >> "$RES/session.log" 2>&1
  BIN=$SB/symbolica-arc-mre-padded OUT=$RES/experiment-padded FIRST_CPU=128 KS="1 24 96" VARIANTS="shared private" WORKS="full specialize split" REPEATS=2 OPS=40000 nice -n 5 ./scripts/run.sh >> "$RES/session.log" 2>&1
  echo "$(date -u +%FT%TZ) session done" >> "$RES/session.log"
'
rc=$?
echo "$(date -u +%FT%TZ) flock rc=$rc" >> "$RES/session.log"
echo "$RES"
