#!/usr/bin/env bash
# W0.3 differential proof for one control family: real walk with the stream
# tap -> restore of its final checkpoint -> fixture of every natively
# inspected ID -> harness re-inspection with the digest sink, compared with
# the tap (reinspect_fixture + RUSTRED_HARNESS_TAP).
#   differential.sh <family> <out-root>
# Environment: BIN (harness test binary), CPUS (default 64-81), WORKERS (tap
# walk worker count; default: the control's own), K (harness threads, 12).
# Families and commands are those of TMP/fable51-controls/run_control.py.
set -euo pipefail
FAM=$1; ROOT=$2
: "${BIN:?}"
CPUS=${CPUS:-64-81}; K=${K:-12}
HERE=$(cd "$(dirname "$0")" && pwd)
R=/common/dev/rustred
case "$FAM" in
  fg) CMD=$R/TMP/four-loop-saved-descendants.VaNmUN/fg/command-rank12orthant.json ;;
  bmw) CMD=$R/TMP/four-loop-saved-descendants.VaNmUN/bmw/command-upstream-a19.json ;;
  h) CMD=$R/TMP/four-loop-saved-descendants.VaNmUN/h/command-rank12orthant.json ;;
  x) CMD=$R/TMP/four-loop-saved-descendants.VaNmUN/x/command-rank12orthant.json ;;
  five-finite) CMD=$R/TMP/ready-five-loop-finite-w50.a6ABXd/ready-first/command.json
    # run_control.py's substitution of the retired campaign's inputs
    RET=$R/TMP/retired-campaigns-20260925.UtI4ay/five-loop-saved-coarse-cover/inputs
    export RUSTRED_HARNESS_MANIFEST=$RET/selection.json RUSTRED_HARNESS_OWNER_BASE=$RET ;;
  *) echo "unknown family $FAM" >&2; exit 2 ;;
esac
FAMDIR=$ROOT/$FAM; OUT=$FAMDIR
mkdir -p "$OUT"
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1
sha=$(sha256sum "$BIN" | cut -d' ' -f1)
echo "BIN=$BIN sha256=$sha CMD=$CMD" > "$OUT/provenance.txt"

t0=$(date +%s.%N)
env RUSTRED_TAP_COMMAND="$CMD" RUSTRED_TAP_OUT="$OUT/walk" ${WORKERS:+RUSTRED_TAP_WORKERS=$WORKERS} \
  nice -n 5 taskset -c "$CPUS" "$BIN" tap_control_walk --ignored --nocapture --test-threads 1 \
  > "$OUT/tap.stdout" 2> "$OUT/tap.stderr"
t1=$(date +%s.%N)
env RUSTRED_CHECKPOINT_RESTORE_DIRECTORY="$OUT/walk/checkpoint" \
  RUSTRED_CHECKPOINT_RESTORE_REQUEST="$OUT/walk/request.json" \
  RUSTRED_CHECKPOINT_RESTORE_RECEIPT="$OUT/extract-receipt.json" \
  RUSTRED_REINSPECTION_FIXTURE="$OUT/fixture.json" RUSTRED_REINSPECTION_MODE=inspected \
  nice -n 5 taskset -c "$CPUS" "$BIN" reinspection_extract --ignored --nocapture --test-threads 1 \
  > "$OUT/extract.stdout" 2> "$OUT/extract.stderr"
t2=$(date +%s.%N)
unset RUSTRED_HARNESS_MANIFEST RUSTRED_HARNESS_OWNER_BASE
BIN=$BIN FIXTURE="$FAMDIR/fixture.json" OUT="$FAMDIR/reinspect" CPUS="$CPUS" THREADS=$K SINK=digest \
  TAP="$FAMDIR/walk/tap.jsonl" PERF=none "$HERE/run_harness.sh"
t3=$(date +%s.%N)
printf '{"tap_walk_seconds":%s,"extract_seconds":%s,"reinspect_seconds":%s}\n' \
  "$(awk "BEGIN{print $t1-$t0}")" "$(awk "BEGIN{print $t2-$t1}")" "$(awk "BEGIN{print $t3-$t2}")" > "$OUT/timings.json"
cat "$OUT/timings.json"
