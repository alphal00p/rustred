#!/usr/bin/env bash
# usage: stress.sh BIN FILTER ITER LOG [extra test args...]
# A run passes only when the test binary exits 0 AND its last libtest summary
# line is "test result: ok." (see contend.sh).
BIN=$1; FILTER=$2; N=$3; LOG=$4; shift 4
export TMPDIR=/common/dev/rustred/.claude/worktrees/agent-ade877816b107b1cf/TMP
pass=0; fail=0
: > $LOG
for i in $(seq 1 $N); do
  out=$(taskset -c 80-87,336-343 nice -n 5 $BIN $FILTER "$@" 2>&1); rc=$?
  last=$(printf '%s\n' "$out" | grep -E '^test result: ' | tail -1)
  if [ $rc = 0 ] && [ "${last#test result: ok.}" != "$last" ]; then pass=$((pass+1))
  else fail=$((fail+1)); echo "=== iteration $i FAILED rc=$rc" >> $LOG; printf '%s\n' "$out" | grep -E 'FAILED|panicked|failures:' | head -20 >> $LOG; fi
done
echo "pass=$pass fail=$fail" >> $LOG
echo "pass=$pass fail=$fail"
