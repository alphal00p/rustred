#!/usr/bin/env bash
# Real successor streams at 74M domains: resume a block clone of the v2 gen-7
# checkpoint with the admission-trace binary (W100 binding, Ready, v2 inputs via
# byte-identical copies outside campaigns/), run RUN_S seconds after restore,
# then cooperative stop. Socket 1 (CPUs 128-227) under the socket1 lock; hard cap 55 min.
set -u
OUT=/common/dev/rustred/TMP/w0/intel/g7-trace-resume
CK=/common/dev/rustred/.claude/worktrees/fable51-py/TMP/gen7-resume
BIN=/common/dev/rustred/TMP/w0/intel/bin/rustred-trace-edbe2729
RUN_S=${RUN_S:-900}
INP=/common/dev/rustred/TMP/retired-campaigns-20260925.UtI4ay/five-loop-saved/inputs
mkdir -p $OUT/trace
cd /common/dev/rustred
export RAYON_NUM_THREADS=1 OMP_NUM_THREADS=1 OMP_THREAD_LIMIT=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1 BLIS_NUM_THREADS=1
export TMPDIR=/common/dev/rustred/.claude/worktrees/fable51-py/TMP
export RUSTRED_ADMISSION_TRACE_DIR=$OUT/trace
echo "$(date -u +%FT%TZ) start MemAvailable $(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo) GiB" > $OUT/run.log
sha256sum $BIN >> $OUT/run.log
nice -n 5 taskset -c 128-227 nix develop --command /run/current-system/sw/bin/time -v $BIN owner-domain-match \
  --manifest $INP/selection.json --owner-base $INP \
  --output $OUT/result.json --events $OUT/events.jsonl --stop-file $OUT/stop-request.json \
  --workers 100 --queries /common/dev/rustred/examples/input/five_loop_qcd_feynman_d9d10/queries.json \
  --follow-successors --max-queries 183 --max-query-bytes 121424 --max-guard-univariate-degree 64 \
  --bounded-refinement-axes finite-axes --transfer-unreserved-lookahead 256 --publication-policy ready \
  --route-domain-overcover --reuse-initial-d-bands --resume $CK --checkpoint-interval-seconds 14400 \
  --unbounded-work --no-progress > $OUT/stdout 2> $OUT/stderr &
PID=$!
echo "pid $PID" >> $OUT/run.log
T0=$(date +%s)
# wait for the first heartbeat after restore
until grep -q '"heartbeat"' $OUT/events.jsonl 2>/dev/null; do
  sleep 5
  if ! kill -0 $PID 2>/dev/null; then echo "$(date -u +%FT%TZ) exited before heartbeat" >> $OUT/run.log; exit 1; fi
  if [ $(( $(date +%s) - T0 )) -gt 1800 ]; then echo "restore too slow; killing" >> $OUT/run.log; kill $PID; exit 1; fi
done
T1=$(date +%s)
echo "$(date -u +%FT%TZ) first heartbeat after $((T1-T0)) s" >> $OUT/run.log
while [ $(( $(date +%s) - T1 )) -lt $RUN_S ]; do sleep 10; kill -0 $PID 2>/dev/null || break; done
echo "$(date -u +%FT%TZ) stop request" >> $OUT/run.log
echo '{"reason":"w0.4 trace window complete"}' > $OUT/stop-request.json
while kill -0 $PID 2>/dev/null; do
  sleep 5
  if [ $(( $(date +%s) - T0 )) -gt 3300 ]; then echo "$(date -u +%FT%TZ) hard cap; kill" >> $OUT/run.log; kill $PID; sleep 20; kill -9 $PID 2>/dev/null; fi
done
wait $PID; echo "$(date -u +%FT%TZ) exit $? after $(( $(date +%s) - T0 )) s" >> $OUT/run.log
