#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
for n in 0 1 2 3 4 5 6 7; do echo "=== innsu g7_partial_inner.sh p$n $n ($(date -u +%FT%TZ))"; timeout -s KILL 1200 scripts/innsu.sh scripts/g7_partial_inner.sh p$n $n; done
echo "=== done $(date -u +%FT%TZ)"
