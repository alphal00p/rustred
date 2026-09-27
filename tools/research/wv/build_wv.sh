#!/usr/bin/env bash
# Build and test the standalone wv crate (release) under the lane's resource protocol.
# Usage: build_wv.sh [build|test]
set -eu
WT=/common/dev/rustred/.claude/worktrees/fable51-wv
CPUS=${CPUS:-72-87,328-343}
MODE=${1:-build}
while [ "$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)" -lt 150 ]; do sleep 60; done
cd $WT/tools/research/wv
exec flock -w 14400 /common/dev/rustred/TMP/locks/build-4.lock \
  env TMPDIR=$WT/TMP CARGO_TARGET_DIR=$WT/TMP/wv-target nice -n 5 taskset -c $CPUS \
  nix develop $WT --command cargo $MODE --release --locked --offline
