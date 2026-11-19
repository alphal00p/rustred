#!/usr/bin/env bash
# gate11 (e): interrupted = uninterrupted, 10 campaigns (gate10's eA = g8_e_inner 14 items x5, eB = g10_e_inner 40 items x5,
# external work_dir, hostile marked daemons / chains, 4 host-like with a non-dumpable descendant), at the unit's CPUQuota 40 %
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11
bash $S/scripts/batch_eA.sh
bash $S/scripts/batch_eB.sh
echo "=== e11 done $(date -u +%FT%TZ)"
