#!/usr/bin/env bash
# batchB: letters b, d, F, a, c, M2-env, host-script dry-run; then (e) under CPUQuota 40 % (user scope around the namespace)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate6; BASH_BIN=/home/vhirschi/.nix-profile/bin/bash
cd $S
for a in "nd_inner.sh before before" "nd_inner.sh during during" "nd_inner.sh resume resume"; do
  echo "=== innsu $a ($(date -u +%FT%TZ))"; timeout -s KILL 1800 scripts/innsu.sh scripts/$a
done
for m in probe steal garbage replay; do
  echo "=== innsu m2_inner.sh $m ($(date -u +%FT%TZ))"; timeout -s KILL 1800 scripts/innsu.sh scripts/m2_inner.sh $m $m
done
for a in "hs_inner.sh" "reg_d_inner.sh" "menv_inner.sh" "b_inner.sh" "reg_e7_inner.sh" "reg_e1_inner.sh" "reg_e1b_inner.sh" "sec_inner.sh" \
         "reg_e8_inner.sh" "reg_e8b_inner.sh" \
         "reg_e2y_inner.sh 601 flipexec flipexec" "reg_e2y_inner.sh 602 daemon chainroot flipexec" "reg_e2y_inner.sh 603 chain clearmark dropexec" \
         "reg_e2y_inner.sh 604 flipexec daemon nodump" "reg_e2y_inner.sh 605 chainroot flipexec" "reg_e2y_inner.sh 606 daemon daemon flipexec"; do
  echo "=== $a ($(date -u +%FT%TZ))"
  timeout -s KILL 2400 scripts/inns.sh scripts/$a
done
for a in "reg_e2y_inner.sh 611 daemon chainroot flipexec" "reg_e2y_inner.sh 612 flipexec flipexec daemon"; do
  echo "=== innsu (host-like) $a ($(date -u +%FT%TZ))"; timeout -s KILL 2400 scripts/innsu.sh scripts/$a
done
for a in "e1 71 1" "e2 72 0"; do
  set -- $a; echo "=== e_inner.sh $1 seed $2 strace $3 under CPUQuota 40% ($(date -u +%FT%TZ))"
  t=$S/tmp/e.$$.$RANDOM; mkdir -p $t
  TMPDIR=$t timeout -s KILL 5400 taskset -c "$CPUS" systemd-run --user --scope -q -p CPUQuota=40% -p MemoryMax=4G -p MemorySwapMax=0 -- unshare -Urpf --mount-proc --kill-child -- $BASH_BIN $S/scripts/e_inner.sh $1 $2 $3
done
echo "=== batchB done $(date -u +%FT%TZ)"
