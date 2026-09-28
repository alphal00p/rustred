#!/usr/bin/env bash
# usage: build-seq.sh LOG "cargo args 1" "cargo args 2" ...  (one build-0 lock hold for all)
# Resource protocol: MemAvailable >= 150 GiB is checked INSIDE the lock, right
# before cargo starts (the wait for the lock can take hours, so a check made
# before queueing would be stale); nice 5, lane CPUs.
set -u
WT=/common/dev/rustred/.claude/worktrees/agent-ade877816b107b1cf
LOG=$1; shift
cd $WT
script='while :; do
  avail=$(awk "/MemAvailable/{print int(\$2/1048576)}" /proc/meminfo)
  [ "$avail" -ge 150 ] && break
  echo "$(date -u +%FT%TZ) (lock held) MemAvailable ${avail} GiB < 150, waiting"
  sleep 60
done
echo "$(date -u +%FT%TZ) lock held, MemAvailable ${avail} GiB"'
for c in "$@"; do script="$script
echo \"\$(date -u +%FT%TZ) start: cargo $c\"; cargo $c || exit \$?"; done
echo "$(date -u +%FT%TZ) queued" >> $LOG
flock -w 14400 /common/dev/rustred/TMP/locks/build-0.lock \
  env TMPDIR=$WT/TMP nice -n 5 taskset -c 80-87,336-343 nix develop --command bash -c "$script" >> $LOG 2>&1
rc=$?
echo "$(date -u +%FT%TZ) exit $rc" >> $LOG
exit $rc
