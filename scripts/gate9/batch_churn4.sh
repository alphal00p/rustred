#!/usr/bin/env bash
# gate9 G9-B2 under CPU contention: N attempts (LIFE 0 / 50 alternating), each with B busy loops beside it in the same unit
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S; N=$1; B=$2; P=$3
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max) busy loops $B"
pids=""; for b in $(seq 1 $B); do ( while :; do :; done ) & pids="$pids $!"; done
for i in $(seq 1 $N); do life=$(( (i % 2) * 50 )); timeout -s KILL 1500 scripts/ns8r.sh scripts/g9_churn_inner.sh $P-$life-$i crash $life 2>&1 | grep -E "RESULT|marker sweep"; done
for p in $pids; do kill $p; done; wait 2>/dev/null; echo "=== done"
