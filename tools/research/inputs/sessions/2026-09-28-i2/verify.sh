#!/usr/bin/env bash
# i2 lane: independent closure verifier = the MERGED oracle (binary rustred-8da58390, fable_5_1 94d60605 incl. the
# fable_5_1-v3-oracle merge + heap-pow patch; integrator build) with full F10 re-inspection and --require-closure,
# then the record audit paired with the report (--verify-report) and assert_oracle_pass.py (fable_5_1 tip copies in
# the i2 worktree). usage: verify.sh CPUS THREADS LABEL=RUNDIR [LABEL=RUNDIR ...]
set -u
cd /common/dev/rustred
CPUS=$1; THREADS=$2; shift 2
BIN=${BIN:-/common/dev/rustred/TMP/fable51-controls/bin/rustred-8da58390}
V=${V:-/common/dev/rustred/TMP/w1/i2/verify}
WT=/common/dev/rustred/.claude/worktrees/fable51-inputs
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
TIME=/run/current-system/sw/bin/time
mkdir -p $V
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1
for spec in "$@"; do
  name=${spec%%=*}; dir=${spec#*=}
  start=$(date +%s)
  taskset -c $CPUS nice -n 5 $TIME -v "$BIN" walk-verify-closure --command "$dir/command.json" --threads "$THREADS" \
    --output "$V/$name.json" --force --require-closure > "$V/$name.stdout" 2> "$V/$name.stderr"
  code=$?
  taskset -c $CPUS nice -n 5 $PY $WT/examples/python/audit_owner_domain_walk.py "$dir" --command "$dir/command.json" --require-closure \
    --verify-report "$V/$name.json" --output "$V/$name.audit.json" > "$V/$name.audit.stdout" 2> "$V/$name.audit.stderr"
  acode=$?
  $PY $WT/examples/python/assert_oracle_pass.py "$V/$name.json" > "$V/$name.gate.txt" 2>&1
  gcode=$?
  echo "$name dir=$dir verify_exit=$code audit_exit=$acode gate_exit=$gcode wall=$(( $(date +%s) - start ))s maxrss_kb=$(grep 'Maximum resident' $V/$name.stderr | awk '{print $NF}') $(date -u +%FT%TZ)"
done
