#!/usr/bin/env bash
# usage: contend.sh BIN SECONDS OUTDIR COPIES FILTER...   (equal-priority contention: COPIES loops per filter)
BIN=$1; SECS=$2; OUT=$3; COPIES=$4; shift 4
export TMPDIR=/common/dev/rustred/.claude/worktrees/agent-ade877816b107b1cf/TMP
mkdir -p $OUT
end=$(( $(date +%s) + SECS ))
for f in "$@"; do
  for c in $(seq 1 $COPIES); do
    (
      i=0
      while [ $(date +%s) -lt $end ]; do
        i=$((i+1))
        out=$(taskset -c 80-87,336-343 nice -n 5 $BIN $f 2>&1)
        if echo "$out" | grep -q 'test result: ok'; then echo "ok" >> $OUT/$(echo $f | tr ':' '_')-$c.log
        else echo "FAIL iteration $i" >> $OUT/$(echo $f | tr ':' '_')-$c.log; echo "$out" | grep -E '^test .* FAILED|panicked' | head -5 >> $OUT/$(echo $f | tr ':' '_')-$c.log; fi
      done
    ) &
  done
done
wait
for f in $OUT/*.log; do echo "$(basename $f): ok=$(grep -c '^ok' $f) fail=$(grep -c '^FAIL' $f)"; done
grep -h 'FAILED' $OUT/*.log | sort | uniq -c
