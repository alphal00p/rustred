#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S; P=$1; N=$2
for i in $(seq 1 $N); do for life in 0 50; do timeout -s KILL 1500 scripts/ns8r.sh scripts/g9_churn_inner.sh $P-$life-$i crash $life 2>&1 | grep -E "RESULT|marker sweep"; done; done
