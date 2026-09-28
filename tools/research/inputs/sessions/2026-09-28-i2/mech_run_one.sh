#!/usr/bin/env bash
# i2 lane mechanism diagnostic (not a gate arm): C-5F Ordered, W18, with ONLY owner 000011001001011 rotated to its
# i2b frame (44 routes; every other route = the original witness). usage: run_one.sh CPUS
set -u
CPUS=$1
cd /common/dev/rustred
export TMPDIR=/common/dev/rustred/TMP
WT=/common/dev/rustred/.claude/worktrees/fable51-inputs
T=$WT/tools/research/inputs
PY=/nix/store/2dkfxh789byan1h81sjhjzsijjsfb57m-python3-3.11.15-env/bin/python
O=TMP/w1/i2/mech
L=one-000011001001011-ord
nice -n 19 taskset -c 126 $PY $T/loadmon.py --cpus $CPUS --match $O/$L/five-finite/ --out $O/$L.load.jsonl &
nice -n 5 taskset -c 88-127,344-383 $PY $T/run_four.py --family five-finite --label $L --cpus $CPUS --workers 18 \
  --max-seconds 1500 --manifest /common/dev/rustred/TMP/w1/i2/mech/selection-one-000011001001011.json --policy ordered \
  --out-root $O --cp5hop $WT/TMP/bin/cp5hop --owners-txt TMP/w0/inputs/owners.txt > $O/$L.run.stdout 2> $O/$L.run.stderr
echo "$L run exit $? $(date -u +%FT%TZ)" >> $O/run.log
nice -n 5 taskset -c $CPUS $PY $WT/examples/python/audit_owner_domain_walk.py $O/$L/five-finite > $O/$L.audit.stdout 2>&1
echo "$L audit exit $? $(date -u +%FT%TZ)" >> $O/run.log
