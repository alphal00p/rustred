#!/usr/bin/env bash
# gate10 (i): SEEDS... : per seed a reference and a run of the same HEALTHY fake campaign (40 items, case [500,3000] ms,
# max_jobs 4), both inside this unit (its CPU quota), each in a fresh private namespace; then the result comparison.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10; cd $S; export HTAG=10 HITEMS=40 HCASE="[500,3000]"
echo "cpu.max: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max 2>/dev/null) load: $(cut -d' ' -f1-3 /proc/loadavg)"
for s in "$@"; do
  for w in ref run; do timeout -s KILL 3600 scripts/ns8r.sh scripts/g9_healthy_inner.sh $s $w 2>&1 | grep HEALTHY; done
  echo "COMPARE seed=$s $(python3 -I scripts/rescmp.py runs/healthy10/s$s/ref.result runs/healthy10/s$s/run.result)"
done
echo "=== done $(date -u +%FT%TZ) load: $(cut -d' ' -f1-3 /proc/loadavg)"
