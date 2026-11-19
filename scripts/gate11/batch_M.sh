#!/usr/bin/env bash
# gate11 (a)(3): gate10's fault matrix (716 cells) + the new class CH (round 6's children-list reads, boottime clock)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; cd $S
export EXTRA_RES=',"unproven_end_limit_s":60'
scripts/matrix10.sh scripts/cells_CH.txt logs/matrix_CH.txt 8
scripts/matrix10.sh scripts/cells_M.txt logs/matrix_M.txt 8
echo "=== done $(date -u +%FT%TZ)"
