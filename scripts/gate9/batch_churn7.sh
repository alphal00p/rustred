#!/usr/bin/env bash
# gate9 G9-B2 under heavy contention: R rounds of K parallel crash+resume attempts in one unit (the unit caps the CPU)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S; P=$1; R=$2; K=$3
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max) load $(cut -d' ' -f1 /proc/loadavg)"
for r in $(seq 1 $R); do for k in $(seq 1 $K); do life=$(( (k % 2) * 50 )); timeout -s KILL 1500 scripts/ns8r.sh scripts/g9_churn_inner.sh $P-$r-$k crash $life 2>&1 | grep -E "RESULT" & done; wait; done
