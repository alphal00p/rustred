#!/usr/bin/env bash
# usage: suites.sh BUILDLOG OUTDIR RUNS [ENVSPEC...]
# Runs every test executable listed in BUILDLOG (the "Executable SRC (EXE)"
# lines of a `cargo test --no-run`) RUNS times, from its crate directory like
# cargo does, pinned to the lane CPUs at nice 5, and writes one log per
# executable and run plus summary.txt (libtest result line, SKIPPED markers,
# seconds). ENVSPEC arguments are passed to `env` (e.g. -u SYMBOLICA_LICENSE).
set -u
WT=/common/dev/rustred/.claude/worktrees/agent-ade877816b107b1cf
LOG=$1; OUT=$2; RUNS=${3:-2}; shift 3
mkdir -p $OUT
export TMPDIR=$WT/TMP
cd $WT
: > $OUT/summary.txt
grep -E '^\s+Executable ' $LOG | sed -E 's/^\s+Executable (.*) \((.*)\)$/\1 \2/' | sort -u > $OUT/executables.txt
for run in $(seq 1 $RUNS); do
  while read -r first second third; do
    if [ "$first" = unittests ]; then src=$second; exe=$third; else src=$first; exe=$second; fi
    name=$(basename $exe)
    base=${name%-*}
    if [ "$base" = rustred ] && [ "$src" != src/main.rs ]; then dir=crates/rustred-core
    elif [ -f crates/rustred-core/$src ] && [ ! -f crates/rustred-app/$src ]; then dir=crates/rustred-core
    else dir=crates/rustred-app; fi
    start=$(date +%s.%N)
    (cd $dir && env "$@" taskset -c 80-87,336-343 nice -n 5 $WT/$exe > $OUT/$name-r$run.log 2>&1)
    rc=$?
    secs=$(awk -v a=$start -v b=$(date +%s.%N) 'BEGIN{printf "%.1f", b-a}')
    res=$(grep -E '^test result:' $OUT/$name-r$run.log | tail -1)
    skips=$(grep -c '^SKIPPED ' $OUT/$name-r$run.log)
    echo "run=$run exe=$name dir=$dir src=$src rc=$rc seconds=$secs skipped_markers=$skips $res" >> $OUT/summary.txt
  done < $OUT/executables.txt
done
cat $OUT/summary.txt
