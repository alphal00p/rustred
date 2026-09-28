#!/usr/bin/env bash
# Real-data multi-target union-cover validation on a clone of the v2 gen-7 checkpoint (read-only
# clone; no re-inspection: the verdict is INCOMPLETE/FAIL by design, the union_sample block is the
# measurement). usage: gen7_union.sh BIN [COUNT] [THREADS]; aborts if MemAvailable < 120 GiB; 55 min cap.
set -u
BIN=$1; COUNT=${2:-20000}; THREADS=${3:-32}
G=/common/dev/rustred/.claude/worktrees/fable51-oracle/TMP/gen7
OUT=${OUT:-/common/dev/rustred/TMP/w0/oracle/r3/gen7}
mkdir -p $OUT
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1 SYMBOLICA_HIDE_BANNER=1
start=$(date +%s)
taskset -c ${CPUS:-0-15,256-271} nice -n 5 timeout 3300 /run/current-system/sw/bin/time -v "$BIN" walk-verify-closure \
  --command $G/request.json --checkpoint $G/checkpoint --no-result --reinspect none \
  --union-sample $COUNT:1 --threads $THREADS --output $OUT/report-union$COUNT.json --force \
  > $OUT/stdout-union$COUNT 2> $OUT/stderr-union$COUNT &
pid=$!
: > $OUT/mem-union$COUNT.tsv
while kill -0 $pid 2>/dev/null; do
  avail=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)
  vpid=$(pgrep -f "^$BIN walk-verify-closure" | head -1)
  vrss=$( [ -n "$vpid" ] && awk '/VmRSS/{print $2}' /proc/$vpid/status 2>/dev/null)
  echo -e "$(( $(date +%s) - start ))\t$avail\t${vrss:-0}" >> $OUT/mem-union$COUNT.tsv
  if [ "$avail" -lt 120 ]; then echo "MemAvailable $avail GiB < 120: killing verifier" >> $OUT/mem-union$COUNT.tsv; kill $vpid; fi
  sleep 5
done
wait $pid; code=$?
echo "exit=$code wall=$(( $(date +%s) - start ))s maxrss_kb=$(grep 'Maximum resident' $OUT/stderr-union$COUNT | awk '{print $NF}')"
