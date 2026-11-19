#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; T=$S/bin/tests
$S/scripts/r11.sh g11_walk $T/gate11_walk --include-ignored --test-threads=1 --nocapture
for n in 1000 5000 10000; do G11_N=$n G11_FANOUT=50 G11_REPEAT=7 $S/scripts/r11.sh g11_cost_$n $T/gate11_cost --ignored --nocapture; grep G11COST $S/logs/g11_cost_$n.txt; done
echo "=== done $(date -u +%FT%TZ)"
