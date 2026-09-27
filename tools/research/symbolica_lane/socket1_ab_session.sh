#!/usr/bin/env bash
# Socket-1 A/B session for the Symbolica lane: four-loop C-4L (W6 x 4 families on 128-151)
# then five-loop C-5F (W50 on 128-177), both Ordered, binaries ref=4a17f9c7 and dev=dev445.
# Run as: flock -w 14400 /common/dev/rustred/TMP/locks/socket1.lock nice -n 5 taskset -c 288-319 bash socket1_ab_session.sh
# (NEW=/path/binary LABEL=name OUT=/run/root SKIP_C5F=1 override the defaults).
set -u
R=/common/dev/rustred/TMP/w0/symbolica
PY="nix develop --command python"
T=/common/dev/rustred/.claude/worktrees/fable51-symbolica/tools/research/symbolica_lane/ab_controls.py
REF=/common/dev/rustred/TMP/fable51-controls/bin/rustred-4a17f9c7
NEW=${NEW:-$R/bin/rustred-dev445-9f2f4c4d}
LABEL=${LABEL:-dev}
OUT=${OUT:-$R/runs}
cd /common/dev/rustred
echo "$(date -u +%FT%TZ) session start (lock held)"
$PY $T --out-root $OUT/c4l --bin ref=$REF --bin $LABEL=$NEW \
  --family fg:128-133 --family bmw:134-139 --family h:140-145 --family x:146-151 \
  --repeats 2 --alternate --policy ordered
echo "$(date -u +%FT%TZ) c4l done"
if [ "${SKIP_C5F:-0}" != 1 ]; then
$PY $T --out-root $OUT/c5f --bin ref=$REF --bin $LABEL=$NEW \
  --family five-finite:128-177 --repeats 2 --alternate --policy ordered --workers 50
echo "$(date -u +%FT%TZ) c5f done"
fi
