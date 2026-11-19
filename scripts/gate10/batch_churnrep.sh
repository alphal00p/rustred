#!/usr/bin/env bash
# gate10: N runs of gate9_churn (pid 1 of a private ns each), full output kept per run
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10; n=$1
for i in $(seq 1 $n); do
  $S/scripts/r10.sh churnrep_$i $S/bin/tests/gate9_churn --test-threads=1 --nocapture | head -3
  grep -h 'panicked\|exit 0 with\|precondition\|did not' -A3 $S/logs/churnrep_$i.txt | head -8 | cut -c1-300
done
