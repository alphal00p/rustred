#!/usr/bin/env bash
# gate9 G9-B2 under load: ROUNDS rounds of 6 parallel crash+resume attempts (LIFE 0 x3, 50 x3)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max) load $(cut -d' ' -f1 /proc/loadavg)"
for r in $(seq 1 $1); do
  for k in 1 2 3; do for life in 0 50; do timeout -s KILL 1000 scripts/ns8r.sh scripts/g9_churn_inner.sh L$r-$life-$k crash $life 2>&1 | grep RESULT & done; done; wait
done
