#!/usr/bin/env bash
# gate10 M10 candidate: recovery saw a marked process of the crashed session alive that vanished before its stop
# (recovery_sweep_vanished), the chain then runs fast: does the end still claim exit 0 with the chain alive?
# PREFIX N HOP1_US HOP2_US DELAY_US
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
pre=$1; n=$2; h1=$3; h2=$4; d=$5
for i in $(seq 1 $n); do
  PRELOAD_PIDFD_DELAY_US=$d ITEMS=${ITEMS:-3} CASE_MS=${CASE_MS:-[1500,2500]} $S/scripts/ns8r.sh $S/scripts/g10_chain_inner.sh $pre-$i $h1 $h2 2>&1 | grep RESULT
done
