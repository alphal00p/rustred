#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6
cd $S
echo "=== innsu supnd_inner.sh ($(date -u +%FT%TZ))"; timeout -s KILL 900 scripts/innsu.sh scripts/supnd_inner.sh
echo "=== batchE done $(date -u +%FT%TZ)"
