#!/usr/bin/env bash
# gate10 (a)(4): PREFIX N HOP_US... with the session-2 supervisor throttled to ~20 % of one CPU (4 busy loops on its CPU)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
pre=$1; n=$2; shift 2
for hop in "$@"; do for i in $(seq 1 $n); do
  SUP_HOGS=4 $S/scripts/ns8r.sh $S/scripts/g10_chain_inner.sh $pre-h$hop-$i $hop 2>&1 | grep RESULT
done; done
