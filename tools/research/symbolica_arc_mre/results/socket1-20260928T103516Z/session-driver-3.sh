#!/usr/bin/env bash
# Driver (not part of the MRE): re-measure the final proposed.diff build under the socket-1 lock.
set -uo pipefail
M=/common/dev/rustred/tools/research/symbolica_arc_mre
RES=$M/results/socket1-20260928T103516Z
export PERF=/nix/store/wizn21b9virxqcnm4n89b09pgqkaxfn3-perf-linux-7.2.5/bin/perf
export RUSTC_VERSION="rustc 1.97.1 (8bab26f4f 2026-07-14) (nix develop /common/dev/rustred)"
echo "$(date -u +%FT%TZ) session3 waiting for socket1.lock" >> "$RES/session.log"
flock -w 3600 /common/dev/rustred/TMP/locks/socket1.lock bash -c '
  RES='"$RES"'; cd '"$M"'
  echo "$(date -u +%FT%TZ) session3 socket1 lock acquired" >> "$RES/session.log"
  BIN=/common/dev/rustred/TMP/symbolica-arc-mre-builds/bin/symbolica-arc-mre-proposed2 OUT=$RES/patched-proposed FIRST_CPU=128 KS="1 24 96" VARIANTS="shared rehome rehome-api private" WORKS="full specialize split" REPEATS=2 OPS=40000 nice -n 5 ./scripts/run.sh >> "$RES/session.log" 2>&1
  echo "$(date -u +%FT%TZ) session3 done" >> "$RES/session.log"
'
echo "$(date -u +%FT%TZ) session3 flock rc=$?" >> "$RES/session.log"
