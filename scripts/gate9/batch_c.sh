#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
for v in ${VARS:-x1 x2 x3 x4 x5}; do echo "--- $v"; timeout -s KILL 900 scripts/ns8r.sh scripts/g8_c_inner.sh c-$v${TAGSUF:-b} $v; done
echo "=== done $(date -u +%FT%TZ)"
