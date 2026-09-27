#!/usr/bin/env bash
# Build a std-only research tool (cargo project dir as $1) under build-1.lock,
# pinned to the intel lane CPUs of the close-out session (32-39,288-295).
set -u
D=$1
exec flock -w 14400 /common/dev/rustred/TMP/locks/build-1.lock bash -c '
  m=$(awk "/MemAvailable/{print int(\$2/1048576)}" /proc/meminfo)
  while [ "$m" -lt 150 ]; do sleep 60; m=$(awk "/MemAvailable/{print int(\$2/1048576)}" /proc/meminfo); done
  echo "MemAvailable ${m} GiB at $(date -u +%FT%TZ)"
  cd '"$D"' && env TMPDIR=/common/dev/rustred/.claude/worktrees/fable51-py/TMP nice -n 5 taskset -c 32-39,288-295 nix develop /common/dev/rustred/.claude/worktrees/fable51-py --command cargo build --release --locked --offline -j 16 2>&1 | grep -E "^(error|warning)|-->|^[0-9 ]+\||Finished|^\s+=" | head -150; echo "rc=${PIPESTATUS[0]}"'
