#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
for c in live resume pidreuse nested otherns live resume; do echo "=== inns g7_lindecoy_inner.sh $c ($(date -u +%FT%TZ))"; timeout -s KILL 2400 scripts/inns.sh scripts/g7_lindecoy_inner.sh $c-$RANDOM $c; done
echo "=== done $(date -u +%FT%TZ)"
