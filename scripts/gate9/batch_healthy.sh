#!/usr/bin/env bash
# gate9 (i): WHICH SEEDS... sequentially (the unit sets the quota)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S; which=$1; shift
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max 2>/dev/null) load: $(cut -d' ' -f1-3 /proc/loadavg)"
for s in "$@"; do timeout -s KILL 3600 scripts/ns8r.sh scripts/g9_healthy_inner.sh $s $which 2>&1 | grep HEALTHY; done
echo "=== done $(date -u +%FT%TZ) load: $(cut -d' ' -f1-3 /proc/loadavg)"
