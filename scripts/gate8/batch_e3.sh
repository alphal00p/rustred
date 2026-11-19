#!/usr/bin/env bash
# gate8 (e) batch: campaigns run one after the other inside this unit (its CPU quota); each in a fresh private namespace
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8; cd $S
echo "cpu.max of this cgroup: $(cat /sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)/cpu.max 2>/dev/null)"
for a in "$@"; do set -- $a; [ -n "${5:-}" ] && export UNPROVEN_LIMIT=$5; [ -n "${6:-}" ] && export AIMHOW=$6; w=ns8r.sh; [ $3 = unp ] && w=ns8u.sh
  echo "=== $1 seed $2 kind $3 max_jobs $4 ($(date -u +%FT%TZ))"; timeout -s KILL 9000 scripts/$w scripts/g8_e3_inner.sh $1 $2 $3 $4; done
echo "=== done $(date -u +%FT%TZ)"
