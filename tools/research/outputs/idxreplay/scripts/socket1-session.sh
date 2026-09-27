#!/usr/bin/env bash
# One socket-1 lock session (<= 60 min) for lane intel:
#  phase 1: traced gen-7 resume (real successor streams at 74M), CPUs 128-227;
#  phase 2: idxreplay static thread sweep 1/48/90 at 74M, CPUs 128-217, interleaved over nodes 4-7.
set -u
L=/common/dev/rustred/TMP/w0/intel/socket1-session.log
T0=$(date +%s)
echo "$(date -u +%FT%TZ) socket1 lock acquired; MemAvailable $(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo) GiB" >> $L
RUN_S=${RUN_S:-900} /common/dev/rustred/TMP/w0/intel/g7-trace-resume/run.sh
echo "$(date -u +%FT%TZ) phase 1 done after $(( $(date +%s) - T0 )) s" >> $L
if [ $(( $(date +%s) - T0 )) -lt 2700 ]; then
  cd /common/dev/rustred/TMP/w0/intel/replay
  timeout $(( 3480 - ($(date +%s) - T0) )) nice -n 5 /nix/store/00p2pzg3i0bdlg9iab09jyr46lnvpi6n-numactl-2.0.18/bin/numactl --interleave=4-7 \
    taskset -c 128-217 /run/current-system/sw/bin/time -v /common/dev/rustred/TMP/w0/intel/bin/idxreplay-v1 static \
    --ckpt /common/dev/rustred/.claude/worktrees/fable51-py/TMP/gen7 --gen 00000000000000000007 --samples 20000 \
    --fracs 1024 --threads 1,48,90 --seconds 8 --load-threads 32 --label g7-threads-socket1 --out static-g7-threads.jsonl \
    > static-g7-threads.stdout 2> static-g7-threads.stderr
  echo "$(date -u +%FT%TZ) phase 2 exit $? after $(( $(date +%s) - T0 )) s" >> $L
else
  echo "$(date -u +%FT%TZ) phase 2 skipped (time budget)" >> $L
fi
echo "$(date -u +%FT%TZ) socket1 lock released after $(( $(date +%s) - T0 )) s" >> $L
