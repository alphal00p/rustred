#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
for i in 1 2 3 4 5 6; do echo "=== innsu nofile 15 #$i"; timeout -s KILL 1200 scripts/innsu.sh scripts/g7_partial2_inner.sh nf15-$i-$RANDOM nofile 15; echo "=== inns nofile 15 #$i"; timeout -s KILL 1200 scripts/inns.sh scripts/g7_partial2_inner.sh rnf15-$i-$RANDOM nofile 15; done
echo "=== done $(date -u +%FT%TZ)"
