#!/usr/bin/env bash
# fix-inputs 2026-09-28: independent closure verifier (oracle branch fable_5_1-v3-oracle, round-2 binary
# rustred-46d4dd28 = 4395ae41 + heap-pow patch; NOT merged into fable_5_1) on the inputs lane's gate runs,
# full F10 re-inspection, --require-closure, then the round-2 audit paired with the report (--verify-report)
# and the gate helper assert_oracle_pass.py. usage: verify.sh CPUS THREADS LABEL=RUNDIR [LABEL=RUNDIR ...]
set -u
cd /common/dev/rustred
CPUS=$1; THREADS=$2; shift 2
BIN=${BIN:-/common/dev/rustred/TMP/w0/oracle/bin/rustred-46d4dd28}  # after the oracle merge: BIN=<merged verifier> V=<new dir>
V=${V:-/common/dev/rustred/TMP/w0/inputs/oracle-verify}
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
TIME=/run/current-system/sw/bin/time
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1
for spec in "$@"; do
  name=${spec%%=*}; dir=${spec#*=}
  start=$(date +%s)
  taskset -c $CPUS nice -n 5 $TIME -v "$BIN" walk-verify-closure --command "$dir/command.json" --threads "$THREADS" \
    --output "$V/$name.json" --force --require-closure > "$V/$name.stdout" 2> "$V/$name.stderr"
  code=$?
  taskset -c $CPUS nice -n 5 $PY $V/tools/audit_owner_domain_walk.py "$dir" --command "$dir/command.json" --require-closure \
    --verify-report "$V/$name.json" --output "$V/$name.audit.json" > "$V/$name.audit.stdout" 2> "$V/$name.audit.stderr"
  acode=$?
  $PY $V/tools/assert_oracle_pass.py "$V/$name.json" > "$V/$name.gate.txt" 2>&1
  gcode=$?
  echo "$name dir=$dir verify_exit=$code audit_exit=$acode gate_exit=$gcode wall=$(( $(date +%s) - start ))s maxrss_kb=$(grep 'Maximum resident' $V/$name.stderr | awk '{print $NF}') $(date -u +%FT%TZ)"
done
