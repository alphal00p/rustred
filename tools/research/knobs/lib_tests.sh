#!/usr/bin/env bash
# Release lib suite of rustred-app under a shared build lock (license set).
# Waits for MemAvailable >= 150 GiB (a release rustc of rustred_app peaks at
# 35-49 GB), then holds BUILD_LOCK for the whole cargo invocation, pinned to
# BUILD_CPUS with nice 5. Full output: $WT/TMP/lib-tests.log.
set -uo pipefail
WT=/common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75
LOCK=${BUILD_LOCK:-/common/dev/rustred/TMP/locks/build-2.lock}
CPUS=${BUILD_CPUS:-44-51,300-307}
cd "$WT"
mkdir -p "$WT/TMP"
while :; do
  avail=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)
  [[ $avail -ge 150 ]] && break
  echo "$(date -u +%FT%TZ) MemAvailable ${avail} GiB < 150, waiting"
  sleep 60
done
flock -w 14400 "$LOCK" nice -n 5 taskset -c "$CPUS" env \
  TMPDIR="$WT/TMP" nix develop --command bash -c 'test -n "$SYMBOLICA_LICENSE" && echo license-set; cargo test --release --locked --offline -p rustred-app --lib' \
  > "$WT/TMP/lib-tests.log" 2>&1
status=$?
tail -40 "$WT/TMP/lib-tests.log"
exit $status
