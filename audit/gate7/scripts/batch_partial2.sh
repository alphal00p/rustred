#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
for n in 2 2; do echo "=== inns dump $n"; timeout -s KILL 1200 scripts/inns.sh scripts/g7_partial2_inner.sh d$n-$RANDOM dump $n; done
for k in 14 16 18 20 22 24 26 28 30 32 36 40; do echo "=== innsu nofile $k"; timeout -s KILL 1200 scripts/innsu.sh scripts/g7_partial2_inner.sh nf$k-$RANDOM nofile $k; done
for k in 14 16 18 20 22 24 26 28 30 32 36 40; do echo "=== inns nofile $k"; timeout -s KILL 1200 scripts/inns.sh scripts/g7_partial2_inner.sh rnf$k-$RANDOM nofile $k; done
echo "=== done $(date -u +%FT%TZ)"
