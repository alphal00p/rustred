#!/usr/bin/env bash
# usage: contend.sh BIN SECONDS OUTDIR COPIES FILTER...   (equal-priority contention: COPIES loops per filter;
#        an empty FILTER "" runs the whole test binary, logged as all-<copy>.log)
# A run counts as ok only when the test binary exits 0 AND its LAST libtest
# summary line is "test result: ok." (an isolated() child's output embedded in
# a parent's failure message can contain an earlier "test result: ok" line).
# env: CPUS (default the lane CPUs 80-87,336-343), CWD (directory to run in).
BIN=$1; SECS=$2; OUT=$3; COPIES=$4; shift 4
CPUS=${CPUS:-80-87,336-343}
export TMPDIR=/common/dev/rustred/.claude/worktrees/agent-ade877816b107b1cf/TMP
[ -n "${CWD:-}" ] && cd "$CWD"
mkdir -p $OUT
end=$(( $(date +%s) + SECS ))
for f in "$@"; do
  for c in $(seq 1 $COPIES); do
    (
      i=0
      name=$(echo "$f" | tr ':' '_'); log=$OUT/${name:-all}-$c.log
      while [ $(date +%s) -lt $end ]; do
        i=$((i+1))
        out=$(taskset -c $CPUS nice -n 5 $BIN "$f" 2>&1); rc=$?
        last=$(printf '%s\n' "$out" | grep -E '^test result: ' | tail -1)
        if [ $rc = 0 ] && [ "${last#test result: ok.}" != "$last" ]; then echo "ok" >> $log
        else echo "FAIL iteration $i rc=$rc" >> $log; printf '%s\n' "$out" | grep -E '^test .* FAILED|panicked' | head -5 >> $log; fi
      done
    ) &
  done
done
wait
for f in $OUT/*.log; do echo "$(basename $f): ok=$(grep -c '^ok' $f) fail=$(grep -c '^FAIL' $f)"; done
grep -h 'FAILED' $OUT/*.log | sort | uniq -c
