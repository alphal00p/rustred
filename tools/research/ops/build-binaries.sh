#!/usr/bin/env bash
# W1.4 ops / N3: one build-0.lock hold that builds, from the committed tree,
#   1. the release test executables of rustred-app + rustred (target/), and
#      target/release/rustred;
#   2. [profile.campaign] rustred           (TMP/ops/target-campaign/);
#   3. [profile.campaign] rustred +mimalloc (TMP/ops/target-mimalloc/);
# concurrently (fat LTO with one codegen unit is mostly serial), then copies
# the three binaries to TMP/w1-ops/bin/rustred-ops-<arm>-<sha8> with a
# provenance sidecar each (tools/research/ops/provenance.py; critique PROV-1).
#
# Resource protocol: MemAvailable >= 150 GiB is checked again INSIDE the lock
# (the wait for the lock can take hours), nice 5, lane CPUs only.
# crates/, Cargo.toml and Cargo.lock must equal HEAD when the build starts
# (checked before queueing and again inside the lock); the sidecars record
# whether they still do when the build ends.
#
# usage: build-binaries.sh LOG
set -u
WT=/common/dev/rustred/.claude/worktrees/agent-ade877816b107b1cf
LOG=${1:?log path}
CPUS=80-87,336-343
cd $WT
if ! git diff --quiet HEAD -- crates Cargo.toml Cargo.lock; then
  echo "$(date -u +%FT%TZ) refusing: crates/ Cargo.toml Cargo.lock differ from HEAD" >> "$LOG"
  exit 2
fi
script='set -u
while :; do
  avail=$(awk "/MemAvailable/{print int(\$2/1048576)}" /proc/meminfo)
  [ "$avail" -ge 150 ] && break
  echo "$(date -u +%FT%TZ) (lock held) MemAvailable ${avail} GiB < 150, waiting"
  sleep 60
done
git diff --quiet HEAD -- crates Cargo.toml Cargo.lock || { echo "source differs from HEAD"; exit 2; }
rev=$(git rev-parse HEAD)
started=$(date -u +%FT%TZ)
echo "$started lock held, MemAvailable ${avail} GiB; start at $rev: tests, campaign, campaign mimalloc"
cargo test --release --locked --offline -p rustred-app -p rustred --tests --no-run > TMP/ops/build-r2-tests.log 2>&1 &
tests=$!
CARGO_TARGET_DIR=$PWD/TMP/ops/target-campaign cargo build --profile campaign --locked --offline -p rustred-app --bin rustred > TMP/ops/build-r2-campaign.log 2>&1 &
plain=$!
CARGO_TARGET_DIR=$PWD/TMP/ops/target-mimalloc cargo build --profile campaign --locked --offline -p rustred-app --bin rustred --features mimalloc > TMP/ops/build-r2-mimalloc.log 2>&1 &
mi=$!
wait $tests; rc0=$?; echo "$(date -u +%FT%TZ) tests exit $rc0"
if [ $rc0 = 0 ]; then
  cargo build --release --locked --offline -p rustred-app --bin rustred >> TMP/ops/build-r2-tests.log 2>&1; rc0=$?
  echo "$(date -u +%FT%TZ) release bin exit $rc0"
fi
wait $plain; rc1=$?; echo "$(date -u +%FT%TZ) campaign exit $rc1"
wait $mi; rc2=$?; echo "$(date -u +%FT%TZ) campaign mimalloc exit $rc2"
finished=$(date -u +%FT%TZ)
P=tools/research/ops/provenance.py
[ $rc0 = 0 ] && python $P --binary target/release/rustred --arm release --profile release --rev $rev --started $started --finished $finished
[ $rc1 = 0 ] && python $P --binary TMP/ops/target-campaign/campaign/rustred --arm campaign --profile campaign --rev $rev --started $started --finished $finished
[ $rc2 = 0 ] && python $P --binary TMP/ops/target-mimalloc/campaign/rustred --arm campaign-mimalloc --profile campaign --features mimalloc --rev $rev --started $started --finished $finished
[ $rc0 = 0 ] && [ $rc1 = 0 ] && [ $rc2 = 0 ]'
echo "$(date -u +%FT%TZ) queued on build-0.lock" >> "$LOG"
flock -w 14400 /common/dev/rustred/TMP/locks/build-0.lock \
  env TMPDIR=$WT/TMP nice -n 5 taskset -c $CPUS nix develop --command bash -c "$script" >> "$LOG" 2>&1
rc=$?
echo "$(date -u +%FT%TZ) exit $rc" >> "$LOG"
exit $rc
