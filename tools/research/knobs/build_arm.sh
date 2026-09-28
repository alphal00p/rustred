#!/usr/bin/env bash
# W0.8 build arm helper: waits for >=150 GiB MemAvailable, holds the shared
# build lock, builds rustred-app's `rustred` binary for one arm and copies it
# to $OUT/rustred-<arm>-<sha8>. Usage:
#   build_arm.sh <arm> [profile] [rustflags]
# arm: a label (e.g. release, znver4, campaign, campaign-znver4);
# profile: cargo profile (default release); rustflags: extra RUSTFLAGS.
set -euo pipefail
ARM=${1:?arm}
PROFILE=${2:-release}
FLAGS=${3:-}
WT=/common/dev/rustred/.claude/worktrees/agent-ab06981cd80007c75
OUT=${OUT:-/common/dev/rustred/TMP/w0/knobs/bin}
LOCK=${BUILD_LOCK:-/common/dev/rustred/TMP/locks/build-2.lock}
mkdir -p "$OUT" "$WT/TMP"
if [[ -n "$FLAGS" ]]; then
  TARGET="$WT/target-$(echo "$FLAGS" | tr -c 'a-zA-Z0-9' '_' | sed 's/_*$//')"
else
  TARGET="$WT/target"
fi
while :; do
  avail=$(awk '/MemAvailable/{print int($2/1048576)}' /proc/meminfo)
  [[ $avail -ge 150 ]] && break
  echo "$(date -u +%FT%TZ) MemAvailable ${avail} GiB < 150, waiting" >&2
  sleep 60
done
cd "$WT"
start=$(date +%s)
flock -w 14400 "$LOCK" nice -n 5 taskset -c "${BUILD_CPUS:-44-51,300-307}" env \
  TMPDIR="$WT/TMP" CARGO_TARGET_DIR="$TARGET" RUSTFLAGS="$FLAGS" \
  nix develop --command cargo build --profile "$PROFILE" --locked --offline \
  -p rustred-app --bin rustred
end=$(date +%s)
BIN="$TARGET/$PROFILE/rustred"
SHA=$(sha256sum "$BIN" | cut -c1-64)
DEST="$OUT/rustred-$ARM-${SHA:0:8}"
cp "$BIN" "$DEST"
chmod 555 "$DEST"
echo "{\"arm\":\"$ARM\",\"profile\":\"$PROFILE\",\"rustflags\":\"$FLAGS\",\"commit\":\"$(git rev-parse HEAD)\",\"dirty\":$(git diff --quiet -- crates Cargo.toml Cargo.lock && echo false || echo true),\"sha256\":\"$SHA\",\"binary\":\"$DEST\",\"build_seconds\":$((end-start))}" | tee "$DEST.json"
