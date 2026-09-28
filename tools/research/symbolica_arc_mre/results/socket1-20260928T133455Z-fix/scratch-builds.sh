#!/usr/bin/env bash
# Scratch builds for the fix round (not part of the MRE). Runs inside flock build-3.lock.
# Usage: build_all.sh [steps...]  steps: check crates pristine p1a proposed padded
set -uo pipefail
B=/common/dev/rustred/TMP/symbolica-arc-mre-builds/fix
M=/common/dev/rustred/tools/research/symbolica_arc_mre
F=/common/dev/rustred/TMP/symbolica-mre-fix
LOG=$B/build.log
STEPS=${*:-check crates pristine p1a proposed padded}
cargo_build() {  # dir target_dir features...
  local dir=$1 tdir=$2; shift 2
  ( cd "$dir" && CARGO_TARGET_DIR=$tdir nice -n 5 taskset -c 88-127 env TMPDIR=/common/dev/rustred/TMP \
      nix develop /common/dev/rustred --command cargo build --release --offline "$@" )
}
mk_scratch() {  # name symbolica_path
  local d=$B/$1 s=$2
  mkdir -p "$d/src"; cp "$M/src/main.rs" "$d/src/main.rs"; cp "$M/Cargo.lock" "$d/Cargo.lock"
  { cat "$M/Cargo.toml"; printf '\n[patch.crates-io]\nsymbolica = { path = "%s" }\nnumerica = { path = "%s/lib/numerica" }\ngraphica = { path = "%s/lib/graphica" }\n' "$s" "$s" "$s"; } > "$d/Cargo.toml"
}
for step in $STEPS; do
  echo "$(date -u +%FT%TZ) step $step start" >> $LOG
  case $step in
    check)  mk_scratch check /common/dev/rustred/TMP/symbolica-upstream-mre
            cargo_build $B/check /common/dev/rustred/TMP/symbolica-arc-mre-builds/target-shared > $B/build-check.log 2>&1 ;;
    crates) cargo_build $M $M/target --bins --examples > $B/build-crates.log 2>&1 && cp $M/target/release/symbolica-arc-mre $B/bin/mre-crates300 && cp $M/target/release/examples/minimal $B/bin/minimal-crates300 ;;
    pristine|p1a|proposed|padded)
            mk_scratch $step $F/$step
            feat=(); [ $step = p1a ] || [ $step = proposed ] && feat=(--features proposed-api)
            cargo_build $B/$step $B/target "${feat[@]}" > $B/build-$step.log 2>&1 && cp $B/target/release/symbolica-arc-mre $B/bin/mre-$step ;;
  esac
  rc=$?
  echo "$(date -u +%FT%TZ) step $step rc=$rc" >> $LOG
  [ $rc -eq 0 ] || { tail -40 $B/build-$step.log >> $LOG; exit $rc; }
done
sha256sum $B/bin/* >> $LOG
echo "$(date -u +%FT%TZ) all done" >> $LOG
