#!/usr/bin/env bash
# usage: stress.sh BIN FILTER ITER LOG [extra test args...]
BIN=$1; FILTER=$2; N=$3; LOG=$4; shift 4
export TMPDIR=/common/dev/rustred/.claude/worktrees/agent-ade877816b107b1cf/TMP
pass=0; fail=0
: > $LOG
for i in $(seq 1 $N); do
  out=$(taskset -c 80-87,336-343 nice -n 5 $BIN $FILTER "$@" 2>&1)
  if echo "$out" | grep -q 'test result: ok'; then pass=$((pass+1)); else fail=$((fail+1)); echo "=== iteration $i FAILED" >> $LOG; echo "$out" | grep -E 'FAILED|panicked|failures:' | head -20 >> $LOG; fi
done
echo "pass=$pass fail=$fail" >> $LOG
echo "pass=$pass fail=$fail"
