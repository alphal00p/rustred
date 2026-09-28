#!/usr/bin/env bash
# Driver (not part of the MRE): socket-1 measurement session of the fix round, in three holds of
# the shared socket-1 lock (A: sweep, B: layout A/B + profiles, C: patch A/B, D: map-offset follow-up),
# each interleaved.
#   session.sh <results dir> [A] [B] [C]
set -uo pipefail
RES=${1:?results dir}; shift
PARTS=${*:-A B C}
M=/common/dev/rustred/tools/research/symbolica_arc_mre
FB=/common/dev/rustred/TMP/symbolica-arc-mre-builds/fix/bin
LOCK=/common/dev/rustred/TMP/locks/socket1.lock
export PERF=/nix/store/wizn21b9virxqcnm4n89b09pgqkaxfn3-perf-linux-7.2.5/bin/perf
export RUSTC_VERSION="rustc 1.97.1 (8bab26f4f 2026-07-14) (nix develop /common/dev/rustred)"
export FIRST_CPU=128 OPS=20000 BUSY_SECS=0.5 TMPDIR=/common/dev/rustred/TMP
mkdir -p "$RES"; cp "$0" "$RES/session-driver-fix.sh"
LOG=$RES/session.log
D=$M/patch
note() { echo "$(date -u +%FT%TZ) $*" >> "$LOG"; }
h() { sha256sum "$D/$1" | cut -c1-16; }
S_CRATES="crates.io symbolica 3.0.0 (src/poly/polynomial.rs identical to dev 445b882d), no patch"
S_PRISTINE="upstream dev 445b882d (local clone), no patch"
S_P1A="dev 445b882d + patch/ablation-1a-2-without-4.diff (sha256 $(h ablation-1a-2-without-4.diff)...)"
S_PROPOSED="dev 445b882d + patch/proposed.diff (sha256 $(h proposed.diff)...)"
S_PADDED="dev 445b882d + patch/experiment-context-padding.diff (sha256 $(h experiment-context-padding.diff)...)"
run() { env "$@" nice -n 5 "$M/scripts/run.sh" >> "$LOG" 2>&1; }
prof() { env BIN="$FB/mre-crates300" OUT="$RES/profile" K=96 OPS=15000 XCCX_PERIOD=503 "$@" \
           nice -n 5 "$M/scripts/profile.sh" >> "$LOG" 2>&1; }

part_A() {  # headline sweep, crates.io build, default layout (c0/m32)
  run BIN=$FB/mre-crates300 OUT=$RES/sweep SOURCE_NOTE="$S_CRATES" LABEL=crates KS="1 8 24 48 96" \
    VARIANTS="shared private-ctx rehome private" WORKS="full specialize split" REPEATS=3
}
part_B() {  # layout A/B (crates.io build), then profiles
  local r lay
  for r in 1 2 3; do
    for lay in "" "--ctx-offset 48" "--ctx-offset 48 --map-offset 0"; do
      run BIN=$FB/mre-crates300 OUT=$RES/layout SOURCE_NOTE="$S_CRATES" LABEL=crates EXTRA="$lay" \
        KS="1 8 24 96" VARIANTS=shared WORKS="full specialize split" REPEATS=1 REPEAT_START=$r
      run BIN=$FB/mre-crates300 OUT=$RES/layout SOURCE_NOTE="$S_CRATES" LABEL=crates EXTRA="$lay" \
        KS="1 96" VARIANTS=private-ctx WORKS=specialize REPEATS=1 REPEAT_START=$r
    done
  done
  note "layout done"
  prof VARIANT=shared WORK=full
  prof VARIANT=shared WORK=full EXTRA="--ctx-offset 48" TAG=-c48
  prof VARIANT=shared WORK=split
  prof VARIANT=private WORK=split
  prof VARIANT=private WORK=split EXTRA="--ahash-source thread-local" TAG=-ahashtl
}
part_C() {  # patch A/B on dev 445b882d builds, interleaved per repeat
  local r
  for r in 1 2 3; do
    local R=(REPEATS=1 REPEAT_START=$r)
    local PR=("${R[@]}" BIN=$FB/mre-pristine OUT=$RES/patch-ab/pristine LABEL=pristine SOURCE_NOTE="$S_PRISTINE")
    local PA=("${R[@]}" BIN=$FB/mre-p1a OUT=$RES/patch-ab/p1a LABEL=p1a SOURCE_NOTE="$S_P1A")
    local PP=("${R[@]}" BIN=$FB/mre-proposed OUT=$RES/patch-ab/proposed LABEL=proposed SOURCE_NOTE="$S_PROPOSED")
    local PD=("${R[@]}" BIN=$FB/mre-padded OUT=$RES/patch-ab/padded LABEL=padded SOURCE_NOTE="$S_PADDED")
    run "${PR[@]}" KS="1 24 96" VARIANTS=shared WORKS="full specialize split"
    run "${PR[@]}" EXTRA="--ctx-offset 48" KS="1 96" VARIANTS=shared WORKS="full specialize split"
    run "${PR[@]}" KS="1 96" VARIANTS="private rehome" WORKS="full specialize split"
    run "${PR[@]}" EXTRA="--ahash-source thread-local" KS="1 96" VARIANTS="shared private" WORKS=split
    run "${PA[@]}" KS="1 24 96" VARIANTS=shared WORKS="full split"
    run "${PA[@]}" KS="1 96" VARIANTS=private WORKS=split
    run "${PA[@]}" EXTRA="--ahash-source thread-local" KS="1 96" VARIANTS="shared private" WORKS=split
    run "${PP[@]}" KS="1 24 96" VARIANTS=shared WORKS="full specialize split"
    run "${PP[@]}" EXTRA="--ctx-offset 48" KS="1 96" VARIANTS=shared WORKS="full specialize split"
    run "${PP[@]}" KS="1 96" VARIANTS="private rehome-api" WORKS="full specialize split"
    run "${PD[@]}" EXTRA="--ctx-vars-at 136" KS="1 24 96" VARIANTS=shared WORKS="full specialize split"
    run "${PD[@]}" EXTRA="--ctx-vars-at 136 --ctx-offset 48" KS="1 96" VARIANTS=shared WORKS="full specialize split"
    note "patch-ab repeat $r done"
  done
  cat "$RES"/patch-ab/{pristine,p1a,proposed,padded}/runs.jsonl > "$RES/patch-ab/runs.jsonl"
  "$M/scripts/summarize.sh" "$RES/patch-ab" > "$RES/patch-ab/summary.md"
}
part_D() {  # map-offset follow-up: does caching nvars (padding diff) remove the map-length penalty?
  local r
  for r in 1 2 3; do
    local R=(REPEATS=1 REPEAT_START=$r)
    local PR=("${R[@]}" BIN=$FB/mre-pristine OUT=$RES/map-offset/pristine LABEL=pristine SOURCE_NOTE="$S_PRISTINE")
    local PD=("${R[@]}" BIN=$FB/mre-padded OUT=$RES/map-offset/padded LABEL=padded SOURCE_NOTE="$S_PADDED")
    for lay in "--map-offset 0" "--ctx-offset 48 --map-offset 0"; do
      run "${PR[@]}" EXTRA="$lay" KS="1 96" VARIANTS=shared WORKS="full specialize"
      run "${PD[@]}" EXTRA="--ctx-vars-at 136 $lay" KS="1 96" VARIANTS=shared WORKS="full specialize"
    done
    run "${PR[@]}" EXTRA="--ctx-offset 48 --map-offset 0" KS="1 96" VARIANTS=private-ctx WORKS=specialize
    run "${PD[@]}" EXTRA="--ctx-vars-at 136 --ctx-offset 48 --map-offset 0" KS="1 96" VARIANTS=private-ctx WORKS=specialize
    note "map-offset repeat $r done"
  done
  cat "$RES"/map-offset/{pristine,padded}/runs.jsonl > "$RES/map-offset/runs.jsonl"
  "$M/scripts/summarize.sh" "$RES/map-offset" > "$RES/map-offset/summary.md"
}
for part in $PARTS; do
  note "part $part waiting for socket1.lock"
  exec 9>>"$LOCK"
  if ! flock -w 3600 9; then note "part $part: lock not acquired within 3600 s"; exec 9>&-; continue; fi
  note "part $part lock acquired"
  part_$part
  note "part $part done"
  flock -u 9; exec 9>&-
  sleep 20   # let other waiters in between parts
done
note "session done"
