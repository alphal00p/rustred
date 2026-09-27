#!/usr/bin/env bash
# W0.5 M1 launcher: holds the socket-1 lock for the whole run, harness on the lane's
# CPUs (94-99 of 82-99, nice 0 so the native child keeps production nice 0), native pinned to
# CPUs 128-227 by the harness, perf helpers at nice 5 on 94-99 (C-4L may use 82-87 meanwhile).
# Usage: m1_run.sh BINARY OUT_DIR
set -u
BIN=$1
OUT=$2
BASE=/common/dev/rustred/TMP/w0/baseline/m1
REQ=/common/dev/rustred/campaigns/five-loop-qcd-feynman-d9d10-v2/runs/20260926T151353.794886Z/request.json
PERF=/nix/store/gyp2si1k1w7jhw8z4xx1bwr2m0pr5445-perf-linux-7.2/bin/perf
HERE=$(cd "$(dirname "$0")" && pwd)
cd /common/dev/rustred/.claude/worktrees/fable51-fp
echo "$(date -u +%FT%TZ) waiting for socket1 lock"
exec flock -w 14400 /common/dev/rustred/TMP/locks/socket1.lock \
  bash -c "echo \"\$(date -u +%FT%TZ) socket1 lock acquired\"; exec taskset -c 94-99 nix develop --command python $HERE/m1_resume_profile.py \
    --binary $BIN --request $REQ --checkpoint $BASE/checkpoint --inputs $BASE/inputs --out $OUT \
    --cpus 128-227 --perf $PERF --perf-cpus 94-99"
