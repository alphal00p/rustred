#!/usr/bin/env bash
# epoch-s2 lane: oracle gate on one run directory (verify-closure full F10 + gate + paired audit).
# usage: oracle.sh BIN RUNDIR CPUS THREADS
set -u
BIN=${1:?}; DIR=${2:?}; CPUS=${3:?}; THREADS=${4:?}
W=/common/dev/rustred/.claude/worktrees/fable51-epoch
R=/common/dev/rustred
OUT=$DIR/oracle
mkdir -p $OUT
export TMPDIR=$R/TMP RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1
start=$(date +%s)
cd $R
taskset -c $CPUS nice -n 5 $BIN walk-verify-closure --command "$DIR/command.json" --threads $THREADS --require-closure \
  --reinspect all --output $OUT/verify.json --force > $OUT/verify.stdout 2> $OUT/verify.stderr
vx=$?
nix develop --command python $W/examples/python/assert_oracle_pass.py $OUT/verify.json > $OUT/gate.txt 2>&1
gx=$?
taskset -c $CPUS nix develop --command python $W/examples/python/audit_owner_domain_walk.py "$DIR" --command "$DIR/command.json" \
  --require-closure --verify-report $OUT/verify.json --output $OUT/audit.json > $OUT/audit.stdout 2> $OUT/audit.stderr
ax=$?
av=$(grep -o '"audit": "[A-Z]*"' $OUT/audit.json 2>/dev/null | head -1 | grep -o '[A-Z]*"$' | tr -d '"')
paired=$(grep -o '"paired": [a-z]*' $OUT/audit.json 2>/dev/null | head -1 | awk '{print $2}')
roots=$(/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python -c "import json,sys;r=json.load(open(sys.argv[1]));print(f\"{r.get('roots_independently_verified')}/{r.get('roots_total')}\")" $OUT/verify.json 2>/dev/null)
verdict=$(grep -o '"verdict": "[A-Z]*"' $OUT/verify.json | head -1 | grep -o '[A-Z]*"$' | tr -d '"')
gate=FAIL; [ $vx = 0 ] && [ $gx = 0 ] && [ $ax = 0 ] && [ "$av" = PASS ] && [ "$paired" = true ] && gate=PASS
line="$(date -u +%FT%TZ) ORACLE $DIR $gate verdict=$verdict verify_exit=$vx gate_exit=$gx audit_exit=$ax audit=$av paired=$paired roots=$roots wall=$(( $(date +%s) - start ))s bin=$(basename $BIN)"
echo "$line" >> $OUT/log
echo "$line"
