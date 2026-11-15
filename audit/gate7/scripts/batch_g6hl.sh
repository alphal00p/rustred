#!/usr/bin/env bash
# gate7: gate6's host-like reproductions (MJ-A nd, supnd, m2) re-run on procfix7 452d0207
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7; cd $S
for a in "nd_inner.sh before before" "nd_inner.sh during during" "nd_inner.sh resume resume" "supnd_inner.sh" "m2_inner.sh probe probe" "m2_inner.sh steal steal" "m2_inner.sh garbage garbage"; do
  echo "=== innsu $a ($(date -u +%FT%TZ))"; timeout -s KILL 2400 scripts/innsu.sh scripts/$a
done
echo "=== batch_g6hl done $(date -u +%FT%TZ)"
