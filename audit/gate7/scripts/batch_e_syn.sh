#!/usr/bin/env bash
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
echo "cpu.max of this cgroup: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max 2>/dev/null)"
for a in "e81 81 1" "e82 82 0" "e83 83 0" "e84 84 0"; do set -- $a; echo "=== g7_e_inner.sh $1 seed $2 strace $3 ($(date -u +%FT%TZ))"; timeout -s KILL 7000 scripts/inns.sh scripts/g7_e_inner.sh $1 $2 $3; done
echo "=== done $(date -u +%FT%TZ)"
