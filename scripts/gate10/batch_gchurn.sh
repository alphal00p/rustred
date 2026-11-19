#!/usr/bin/env bash
# gate10 (g) G9-B2 variants: zombie leader + K self-replacing threads (K 1 / 2 / 64), thread life 0 / 1 / 2 / 10 ms, crashed
# session; with the 2 ms open delay of the resumed supervisor's per-thread /proc opens (gate9's deterministic mode) and without.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10
for d in 2000 none; do for k in 1 2 64; do for life in 0 1000 2000 10000; do
  if [ $d = none ]; then K=$k $S/scripts/ns8r.sh $S/scripts/g10_churn_inner.sh gc-k$k-l$life-nodelay crash $life 2>&1 | grep RESULT
  else K=$k DELAY_US=$d $S/scripts/ns8r.sh $S/scripts/g10_churn_inner.sh gc-k$k-l$life-d$d crash $life 2>&1 | grep RESULT; fi
done; done; done
