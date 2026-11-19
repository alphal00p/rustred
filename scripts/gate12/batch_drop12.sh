#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate12
export EXTRA_RES=',"unproven_end_limit_s":60' G8_TIMEOUT=420 CELL_TIMEOUT=900
$S/scripts/matrix11.sh $S/scripts/cells12.txt $S/logs/drop12.txt 8
