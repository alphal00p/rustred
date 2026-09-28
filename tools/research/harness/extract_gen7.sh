#!/usr/bin/env bash
# Restore a block clone of the v2 gen-7 CP5 checkpoint and write the W0.3
# pending-sample fixture (reinspection_extract, mode pending).
#   BIN=<harness test binary> extract_gen7.sh <work dir> <fixture path>
# <work dir> must hold gen7-clone/ (cp -r of TMP/v2-checkpoint-copy-gen7),
# v2-inputs/ (cp -r of the v2 campaign inputs) and v2-request.json.
set -euo pipefail
WORK=$1; FIXTURE=$2
: "${BIN:?}"
CPUS=${CPUS:-64-81}
receipt=${RECEIPT:-$(dirname "$FIXTURE")/$(basename "$FIXTURE" .json)-extract-receipt.json}
env RAYON_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1 \
  RUSTRED_CHECKPOINT_RESTORE_DIRECTORY="$WORK/gen7-clone" \
  RUSTRED_CHECKPOINT_RESTORE_REQUEST="$WORK/v2-request.json" \
  RUSTRED_CHECKPOINT_RESTORE_RECEIPT="$receipt" \
  RUSTRED_CHECKPOINT_RESTORE_MANIFEST="$WORK/v2-inputs/selection.json" \
  RUSTRED_CHECKPOINT_RESTORE_OWNER_BASE="$WORK/v2-inputs" \
  RUSTRED_CHECKPOINT_RESTORE_QUERIES="$WORK/v2-inputs/queries.json" \
  RUSTRED_CHECKPOINT_RESTORE_SAVER_EXECUTABLE=/common/dev/rustred/TMP/fable51-controls/bin/rustred-102adcc3 \
  RUSTRED_REINSPECTION_FIXTURE="$FIXTURE" RUSTRED_REINSPECTION_MODE=${MODE:-pending} \
  ${ALLOCATION:+RUSTRED_REINSPECTION_ALLOCATION=$ALLOCATION} \
  nice -n 5 taskset -c "$CPUS" "$BIN" reinspection_extract --ignored --nocapture --test-threads 1
