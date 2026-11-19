#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6
cd $S
echo "=== e8t_inner.sh ($(date -u +%FT%TZ))"; timeout -s KILL 900 scripts/inns.sh scripts/e8t_inner.sh
echo "=== batchF done $(date -u +%FT%TZ)"
