#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; cd $S
echo "unit cgroup: $(cut -d: -f3 /proc/self/cgroup) delegate xattr: $(getfattr -n user.delegate --only-values /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup) 2>&1)"
for k in escapee zchain daemon; do for i in 1 2; do timeout -s KILL 900 scripts/ns8r.sh scripts/g11_deleg_inner.sh d-$k-$i $k 2>&1 | grep -E 'DELEG|own cgroup'; done; done
echo "=== done $(date -u +%FT%TZ)"
