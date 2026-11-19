#!/usr/bin/env bash
# gate9 G9-B2 rate: TAGPREFIX N : N crash+resume runs each of LIFE 0 and 20 us (the unit sets the quota)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S; p=$1; n=$2
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max)"
for i in $(seq 1 $n); do for life in 0 20; do
  timeout -s KILL 1000 scripts/ns8r.sh scripts/g9_churn_inner.sh $p-$life-$i crash $life 2>&1 | grep RESULT; done; done
