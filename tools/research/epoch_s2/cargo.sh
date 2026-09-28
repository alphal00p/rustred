#!/usr/bin/env bash
# epoch-s2 lane: run one cargo command under build-3.lock on CPUs 0-15,256-271.
# usage: cargo.sh LOGNAME cargo-args...
set -u
W=/common/dev/rustred/.claude/worktrees/fable51-epoch
LOG=$W/TMP/epoch-s2/$1.log; shift
cd $W
while [ "$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)" -lt 250 ]; do sleep 60; done
echo "$(date -u +%FT%TZ) waiting lock: cargo $*" > $LOG
flock -w 14400 /common/dev/rustred/TMP/locks/build-3.lock bash -c '
  while [ "$(awk "/MemAvailable/{print int(\$2/1048576)}" /proc/meminfo)" -lt 250 ]; do sleep 60; done
  echo "$(date -u +%FT%TZ) lock acquired" >> '"$LOG"'
  env TMPDIR='"$W"'/TMP RUSTRED_TESTS_REQUIRE_LICENSE=1 nice -n 5 taskset -c 0-15,256-271 nix develop --command cargo "$@" >> '"$LOG"' 2>&1
  echo "$(date -u +%FT%TZ) rc=$?" >> '"$LOG"'
' _ "$@"
tail -3 $LOG
