#!/usr/bin/env bash
# usage: build.sh <label> <cargo args...>
set -u
WT=/common/dev/rustred/.claude/worktrees/fable51-symbolica
label=$1; shift
log=$WT/TMP/symbolica-lane/build-$label.log
cd $WT
wait_mem() {
  while true; do
    m=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)
    if [ "$m" -ge 150 ]; then echo "$(date -u +%FT%TZ) MemAvailable ${m} GiB ok" >> $log; return; fi
    echo "$(date -u +%FT%TZ) MemAvailable ${m} GiB < 150, sleeping" >> $log; sleep 60
  done
}
wait_mem
echo "$(date -u +%FT%TZ) waiting for build-2.lock" >> $log
exec flock -w 14400 /common/dev/rustred/TMP/locks/build-2.lock bash -c '
  m=$(awk "/MemAvailable/{print int(\$2/1048576)}" /proc/meminfo)
  while [ "$m" -lt 150 ]; do echo "$(date -u +%FT%TZ) in-lock MemAvailable $m < 150, sleeping" >> '"$log"'; sleep 60; m=$(awk "/MemAvailable/{print int(\$2/1048576)}" /proc/meminfo); done
  echo "$(date -u +%FT%TZ) lock acquired, MemAvailable $m GiB; cargo $*" >> '"$log"'
  start=$(date +%s)
  nice -n 5 taskset -c 288-319 env TMPDIR='"$WT"'/TMP nix develop --command cargo "$@" >> '"$log"' 2>&1
  rc=$?
  echo "$(date -u +%FT%TZ) cargo exit $rc after $(( $(date +%s) - start )) s" >> '"$log"'
  exit $rc
' _ "$@"
