#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
echo "=== inns g7_fbfull_inner.sh fb1 ($(date -u +%FT%TZ))"; timeout -s KILL 1200 scripts/inns.sh scripts/g7_fbfull_inner.sh fb1
echo "=== done $(date -u +%FT%TZ)"
