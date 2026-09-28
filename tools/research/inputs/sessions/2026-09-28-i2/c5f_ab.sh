#!/usr/bin/env bash
# i2 lane (W1 I2 rebuild): C-5F (five-loop 1,324-tuple finite control, one query on owner 011101110111000)
# with the original vs the rebuilt (one frame per owner) route witnesses; W18 each, concurrently on disjoint
# 18-core sets (no SMT siblings), cooperative stop at 1,500 s; then the record audit and the census of each arm.
# Usage: c5f_ab.sh REPEAT CPUS_ORIG CPUS_I2B POLICY [PERF_EVENTS]   (labels orig-REPEAT, i2b-REPEAT)
#   POLICY ready|ordered replaces the control's --publication-policy.
set -u
R=$1; CO=$2; CI=$3; P=$4; PS=${5:-}
cd /common/dev/rustred
export TMPDIR=/common/dev/rustred/TMP
WT=/common/dev/rustred/.claude/worktrees/fable51-inputs
T=$WT/tools/research/inputs
H=$WT/TMP/bin/cp5hop
A=$WT/examples/python/audit_owner_domain_walk.py
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
O=TMP/w1/i2/c5f
SEL=/common/dev/rustred/TMP/w1/i2/selection-i2b.json
mkdir -p $O
LOG=$O/ab-$R.log
arm() { # label cpus [manifest]
  local label=$1 cpus=$2 man=${3:-}
  nice -n 19 taskset -c 126 $PY $T/loadmon.py --cpus $cpus --match $O/$label/five-finite/ --out $O/$label.load.jsonl &
  nice -n 5 taskset -c 88-127,344-383 $PY $T/run_four.py --family five-finite --label $label \
    --cpus $cpus --workers 18 --max-seconds 1500 ${man:+--manifest $man} --policy $P ${PS:+--perf-stat $PS} --out-root $O --cp5hop $H \
    --owners-txt TMP/w0/inputs/owners.txt > $O/$label.run.stdout 2> $O/$label.run.stderr
  echo "$label run exit $? $(date -u +%FT%TZ)" >> $LOG
  nice -n 5 taskset -c $cpus $PY $A $O/$label/five-finite > $O/$label.audit.stdout 2>&1
  echo "$label audit exit $? $(date -u +%FT%TZ)" >> $LOG
}
echo "start $(date -u +%FT%TZ) orig=$CO i2b=$CI policy=$P selection=$(sha256sum $SEL | cut -c1-16)" >> $LOG
arm orig-$R $CO &
arm i2b-$R $CI $SEL &
wait
echo "done $(date -u +%FT%TZ)" >> $LOG
