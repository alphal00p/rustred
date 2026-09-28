#!/usr/bin/env bash
# Builds the rustred core lib tests against two Symbolica variants (via --config patch overrides; the
# worktree's vendor/symbolica is untouched) and runs the two native_heap_pow regression tests.
set -uo pipefail
D=/common/dev/rustred/TMP/symbolica-main/symeval-dev
LOG=$D/logs/heappow-variants.log
exec > "$LOG" 2>&1
exec 8> /common/dev/rustred/TMP/locks/build-6.lock
echo "waiting build-6 lock $(date -u +%FT%TZ)"
flock -w 14400 8 || { echo "lock timeout"; exit 3; }
while :; do ma=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo); [ "$ma" -ge 150 ] && break; echo "MemAvailable $ma < 150"; sleep 60; done
WT=/common/dev/rustred/.claude/worktrees/fable51-symeval
cd "$WT"
export TMPDIR=/common/dev/rustred/TMP
for v in dev-fix-only dev-fix-localpatch; do
  S=/common/dev/rustred/TMP/symbolica-main/$v
  echo "== variant $v ($(git -C $S rev-parse HEAD), dirty: $(git -C $S status --short | tr '\n' ' ')) $(date -u +%FT%TZ)"
  nice -n 5 taskset -c 40-55,296-311 nix develop /common/dev/rustred --command cargo test --release --locked --offline -p rustred --lib \
    --target-dir /common/dev/rustred/TMP/symbolica-main/symeval-dev/target-$v -j 32 \
    --config "patch.crates-io.symbolica.path=\"$S\"" \
    --config "patch.crates-io.numerica.path=\"$S/lib/numerica\"" \
    --config "patch.crates-io.graphica.path=\"$S/lib/graphica\"" \
    native_heap_pow -- --test-threads 2
  echo "== rc=$? $(date -u +%FT%TZ)"
done
echo done
