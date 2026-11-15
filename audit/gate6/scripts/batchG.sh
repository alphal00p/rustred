#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6
cd $S
echo "=== innsu twosup_inner.sh ($(date -u +%FT%TZ))"; timeout -s KILL 900 scripts/innsu.sh scripts/twosup_inner.sh
echo "=== batchG done $(date -u +%FT%TZ)"
