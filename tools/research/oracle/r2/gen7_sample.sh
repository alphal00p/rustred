#!/usr/bin/env bash
# (f) walk-verify-closure in sample mode on a clone of the v2 gen-7 checkpoint (read-only clone).
# usage: gen7_sample.sh BIN [SAMPLE] [THREADS]; guard: aborts if MemAvailable < 120 GiB; 55 min cap.
set -u
BIN=$1; SAMPLE=${2:-10000}; THREADS=${3:-32}
G=/common/dev/rustred/.claude/worktrees/fable51-oracle/TMP/gen7
OUT=${OUT:-/common/dev/rustred/TMP/w0/oracle/r2/gen7}
mkdir -p $OUT
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1
start=$(date +%s)
taskset -c ${CPUS:-0-15,256-271} nice -n 5 timeout 3300 /run/current-system/sw/bin/time -v "$BIN" walk-verify-closure \
  --command $G/request.json --checkpoint $G/checkpoint --no-result --reinspect sample:$SAMPLE:1 \
  --require-closure --threads $THREADS --output $OUT/report-sample$SAMPLE.json --force \
  > $OUT/stdout-sample$SAMPLE 2> $OUT/stderr-sample$SAMPLE &
pid=$!
: > $OUT/mem-sample$SAMPLE.tsv
while kill -0 $pid 2>/dev/null; do
  avail=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)
  vpid=$(pgrep -f "^$BIN walk-verify-closure" | head -1)
  vrss=$( [ -n "$vpid" ] && awk '/VmRSS/{print $2}' /proc/$vpid/status 2>/dev/null)
  echo -e "$(( $(date +%s) - start ))\t$avail\t${vrss:-0}" >> $OUT/mem-sample$SAMPLE.tsv
  if [ "$avail" -lt 120 ]; then echo "MemAvailable $avail GiB < 120: killing verifier" >> $OUT/mem-sample$SAMPLE.tsv; kill $vpid; fi
  sleep 5
done
wait $pid; code=$?
echo "exit=$code wall=$(( $(date +%s) - start ))s maxrss_kb=$(grep 'Maximum resident' $OUT/stderr-sample$SAMPLE | awk '{print $NF}')"
