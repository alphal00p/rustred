#!/usr/bin/env bash
# W1.3 witness gate: C-5F (five-loop 1,324-tuple finite control, one query on owner 011101110111000)
# with the original vs the I2-rewritten route witnesses; W18 each, concurrently on disjoint 18-core sets,
# cooperative stop at 1,500 s; then the walk audit and the census of each arm.
# Usage: c5f_ab.sh REPEAT CPUS_ORIG CPUS_I2    (labels orig-REPEAT, i2-REPEAT)
set -u
R=$1; CO=$2; CI=$3
cd /common/dev/rustred
export TMPDIR=/common/dev/rustred/TMP
T=/common/dev/rustred/.claude/worktrees/fable51-inputs/tools/research/inputs
H=/common/dev/rustred/.claude/worktrees/fable51-inputs/TMP/bin/cp5hop
A=/common/dev/rustred/.claude/worktrees/fable51-inputs/examples/python/audit_owner_domain_walk.py
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
O=TMP/w0/inputs/i2/c5f
LOG=$O/ab-$R.log
arm() { # label cpus [manifest]
  local label=$1 cpus=$2 man=${3:-}
  nice -n 19 taskset -c 96 $PY $T/loadmon.py --cpus $cpus --match $O/$label/five-finite/ --out $O/$label.load.jsonl &
  nice -n 5 taskset -c 88-127,344-383 nix develop --command python $T/run_four.py --family five-finite --label $label \
    --cpus $cpus --workers 18 --max-seconds 1500 ${man:+--manifest $man} --out-root $O --cp5hop $H \
    --owners-txt TMP/w0/inputs/owners.txt > $O/$label.run.stdout 2> $O/$label.run.stderr
  echo "$label run exit $? $(date -u +%FT%TZ)" >> $LOG
  nice -n 5 taskset -c $cpus nix develop --command python $A $O/$label/five-finite > $O/$label.audit.stdout 2>&1
  echo "$label audit exit $? $(date -u +%FT%TZ)" >> $LOG
}
echo "start $(date -u +%FT%TZ) orig=$CO i2=$CI" >> $LOG
arm orig-$R $CO &
arm i2-$R $CI /common/dev/rustred/TMP/w0/inputs/i2/selection-i2.json &
wait
echo "done $(date -u +%FT%TZ)" >> $LOG
