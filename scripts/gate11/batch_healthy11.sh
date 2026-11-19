#!/usr/bin/env bash
# gate11 (i): 20 HEALTHY fake campaigns (seeds 301-320; 40 items, case [500,3000] ms, max_jobs 4), reference AND run of each
# seed inside this unit (CPUQuota 20 %), each in a fresh private namespace, beside the process-layer suite part (other unit)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; cd $S; export HTAG=11 HITEMS=40 HCASE="[500,3000]"
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max 2>/dev/null) load: $(cut -d' ' -f1-3 /proc/loadavg)"
for s in $(seq 301 320); do
  for w in ref run; do timeout -s KILL 3600 scripts/ns8r.sh scripts/g9_healthy_inner.sh $s $w 2>&1 | grep HEALTHY; done
  echo "COMPARE seed=$s $(python3 -I scripts/rescmp.py runs/healthy11/s$s/ref.result runs/healthy11/s$s/run.result)"
done
echo "=== done $(date -u +%FT%TZ) load: $(cut -d' ' -f1-3 /proc/loadavg)"
