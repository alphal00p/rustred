#!/usr/bin/env bash
# batchD: reruns of two void cases (ND 'before' hung in my harness; m3 'noprealloc' failpoint value lacked ':1')
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6
cd $S
echo "=== innsu nd_inner.sh before2 before ($(date -u +%FT%TZ))"; timeout -s KILL 1800 scripts/innsu.sh scripts/nd_inner.sh before2 before
echo "=== m3_inner.sh noprealloc2 noprealloc ($(date -u +%FT%TZ))"; timeout -s KILL 1800 scripts/inns.sh scripts/m3_inner.sh noprealloc2 noprealloc
echo "=== batchD done $(date -u +%FT%TZ)"
