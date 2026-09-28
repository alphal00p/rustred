#!/usr/bin/env bash
# rustred walk-verify-closure over every W0.2 calibration output (full F10 re-inspection).
# usage: verify_all.sh BIN [THREADS] [names...]
set -u
BIN=$1; THREADS=${2:-18}; shift 2 || true
O=/common/dev/rustred/TMP/w0/oracle
R=$O/runs
mkdir -p $O/verify
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1
declare -A DIRS=(
  [c4l-ordered-fg]=$R/c4l-ordered/fg [c4l-ordered-bmw]=$R/c4l-ordered/bmw [c4l-ordered-h]=$R/c4l-ordered/h [c4l-ordered-x]=$R/c4l-ordered/x
  [c4l-ready-fg]=$R/c4l-ready/fg [c4l-ready-bmw]=$R/c4l-ready/bmw [c4l-ready-h]=$R/c4l-ready/h [c4l-ready-x]=$R/c4l-ready/x
  [c5f-ordered]=$R/c5f-ordered/five-finite [c5f-ready]=$R/c5f-ready/five-finite
  [frontier-fixture-fg]=$R/frontier-fixture/fg
)
names=("$@")
[ ${#names[@]} -eq 0 ] && names=(c4l-ordered-fg c4l-ordered-bmw c4l-ordered-h c4l-ordered-x c4l-ready-fg c4l-ready-bmw c4l-ready-h c4l-ready-x c5f-ordered c5f-ready frontier-fixture-fg)
for name in "${names[@]}"; do
  dir=${DIRS[$name]}
  extra=(--require-closure)
  [ "$name" = frontier-fixture-fg ] && extra=()
  start=$(date +%s)
  nix develop /common/dev/rustred --command taskset -c 46-63 nice -n 5 /run/current-system/sw/bin/time -v "$BIN" walk-verify-closure \
    --command "$dir/command.json" --threads "$THREADS" --output "$O/verify/$name.json" --force "${extra[@]}" \
    > "$O/verify/$name.stdout" 2> "$O/verify/$name.stderr"
  code=$?
  echo "$name exit=$code wall=$(( $(date +%s) - start ))s"
done
