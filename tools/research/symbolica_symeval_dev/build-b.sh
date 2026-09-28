#!/usr/bin/env bash
# build.sh <tag> <worktree> <target-dir> : builds rustred bin + lib test binaries for core/app + cli_routed_campaign
set -uo pipefail
tag=$1; wt=$2; tgt=$3
LOG=/common/dev/rustred/TMP/symbolica-main/symeval-dev/logs/build-$tag.log
exec > "$LOG" 2>&1
exec 8> /common/dev/rustred/TMP/locks/build-6.lock
echo "waiting build-6 lock $(date -u +%FT%TZ)"
flock -w 14400 8 || { echo "lock timeout"; exit 3; }
while :; do
  ma=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)
  [ "$ma" -ge 150 ] && break
  echo "MemAvailable ${ma} GiB < 150, waiting"; sleep 60
done
echo "lock held, MemAvailable $(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo) GiB, start $(date -u +%FT%TZ)"
cd "$wt"
export TMPDIR=/common/dev/rustred/TMP CARGO_TARGET_DIR="$tgt"
run() { echo "== $* $(date -u +%FT%TZ)"; nice -n 5 taskset -c 40-55,296-311 nix develop /common/dev/rustred --command "$@"; echo "== rc=$? $(date -u +%FT%TZ)"; }
run cargo test --release --locked --offline -p rustred-app --lib --no-run -j 32
run cargo build --release --locked --offline -p rustred-app --bin rustred -j 32
run cargo test --release --locked --offline -p rustred --lib --no-run -j 32
run cargo test --release --locked --offline -p rustred-app --test cli_routed_campaign --no-run -j 32
echo "done $(date -u +%FT%TZ)"
