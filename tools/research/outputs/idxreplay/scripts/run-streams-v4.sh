#!/usr/bin/env bash
# Re-run idxreplay streams (v4 binary) on the read-only gen-7 fixture, lane CPUs 32-39,288-295.
set -u
cd /common/dev/rustred/TMP/w0/intel/replay
m=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)
while [ "$m" -lt 150 ]; do sleep 60; m=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo); done
echo "$(date -u +%FT%TZ) start; MemAvailable $m GiB" > streams-g7-v4.session.log
T0=$(date +%s)
/common/dev/rustred/TMP/w0/intel/loadsampler2.sh 32-39,288-295 streams-g7-v4.load.txt $((T0 + 1500)) 'idxreplay-v4 streams' &
LS=$!
nice -n 19 taskset -c 32-39,288-295 /run/current-system/sw/bin/time -v /common/dev/rustred/TMP/w0/intel/bin/idxreplay-v4 streams \
  --ckpt /common/dev/rustred/.claude/worktrees/fable51-py/TMP/gen7 --gen 00000000000000000007 \
  --trace ../g7-trace-resume/trace --initial 67 --layer-sample 200000 --label g7-streams-w100-v4 \
  --out streams-g7-v4.jsonl > streams-g7-v4.stdout 2> streams-g7-v4.stderr
rc=$?
kill $LS 2>/dev/null
echo "$(date -u +%FT%TZ) exit $rc after $(( $(date +%s) - T0 )) s" >> streams-g7-v4.session.log
