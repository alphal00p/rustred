#!/usr/bin/env bash
# i2 lane: build the route_witness_rewrite example (fable_5_1-v3-i2 worktree)
set -u
cd /common/dev/rustred/.claude/worktrees/fable51-inputs
while [ "$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)" -lt 150 ]; do sleep 60; done
echo "lock wait start $(date -u +%FT%TZ)"
exec flock -w 14400 /common/dev/rustred/TMP/locks/build-2.lock env TMPDIR=/common/dev/rustred/.claude/worktrees/fable51-inputs/TMP \
  nice -n 5 taskset -c 88-127,344-383 nix develop --command cargo build --release --locked --offline -p rustred-app --example route_witness_rewrite "$@"
