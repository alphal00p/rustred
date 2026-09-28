#!/usr/bin/env bash
# W0.5 M1 launcher: holds the socket-1 lock for the whole run, harness on the lane's
# CPUs (default 94-99 of 82-99, nice 0 so the native child keeps production nice 0), native
# pinned to CPUs 128-227 by the harness, perf helpers at nice 5 on the harness CPUs.
# The lock is held until the native process has exited, even if the harness dies
# (run1 lost the lock to another lane when the harness crashed mid-run).
# Usage: m1_run.sh BINARY OUT_DIR [CHECKPOINT_CLONE [INPUTS_CLONE]]
# Env:   M1_HARNESS_CPUS  harness + perf CPUs (default 94-99; the W2 comparator used 40-43,296-299)
set -u
BIN=$1
OUT=$2
BASE=/common/dev/rustred/TMP/w0/baseline/m1
CKPT=${3:-$BASE/checkpoint}
INPUTS=${4:-$BASE/inputs}
HCPUS=${M1_HARNESS_CPUS:-94-99}
REQ=/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2/runs/20260926T151353.794886Z/request.json
PERF=/nix/store/gyp2si1k1w7jhw8z4xx1bwr2m0pr5445-perf-linux-7.2/bin/perf
HERE=$(cd "$(dirname "$0")" && pwd)
cd /common/dev/rustred/.claude/worktrees/fable51-fp
echo "$(date -u +%FT%TZ) waiting for socket1 lock"
exec flock -w 14400 /common/dev/rustred/TMP/locks/socket1.lock bash -c "
  echo \"\$(date -u +%FT%TZ) socket1 lock acquired\"
  taskset -c $HCPUS nix develop --command python $HERE/m1_resume_profile.py \
    --binary $BIN --request $REQ --checkpoint $CKPT --inputs $INPUTS --out $OUT \
    --cpus 128-227 --perf $PERF --perf-cpus $HCPUS
  rc=\$?
  if [ -s $OUT/native.pid ] && kill -0 \$(cat $OUT/native.pid) 2>/dev/null; then
    # harness gone while the native runs: cooperative stop, then SIGKILL after 15 min
    [ -e $OUT/stop-request.json ] || echo '{\"reason\": \"harness_exit\", \"family_closure_claim\": false}' > $OUT/stop-request.json
    for i in \$(seq 1 450); do kill -0 \$(cat $OUT/native.pid) 2>/dev/null || break; sleep 2; done
    kill -9 \$(cat $OUT/native.pid) 2>/dev/null
  fi
  echo \"\$(date -u +%FT%TZ) harness exit \$rc; native gone; releasing socket1 lock\"
  exit \$rc"
