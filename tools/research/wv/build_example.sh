#!/usr/bin/env bash
# Build the rustred-core example wv-family-automorphisms (release) under the lane's
# resource protocol: build-4 lock, MemAvailable >= 150 GiB, nice 5, lane CPUs.
set -eu
WT=/common/dev/rustred/.claude/worktrees/fable51-wv
CPUS=${CPUS:-72-87,328-343}
while [ "$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)" -lt 150 ]; do sleep 60; done
cd $WT
exec flock -w 14400 /common/dev/rustred/TMP/locks/build-4.lock \
  env TMPDIR=$WT/TMP nice -n 5 taskset -c $CPUS \
  nix develop --command cargo build --release --locked --offline -p rustred --example wv-family-automorphisms
