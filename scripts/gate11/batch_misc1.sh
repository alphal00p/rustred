#!/usr/bin/env bash
# gate11 slot-1 batch: (i) cost, (h) supervisor as pid 1, (F) decoy families + pid reuse, (a)(2) helpers
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; T=$S/bin/tests; cd $S
echo "##### (i) cost of the completeness proof"
for n in 1000 2500 5000; do G11_N=$n G11_FANOUT=50 G11_REPEAT=7 $S/scripts/r11.sh g11_cost2_$n $T/gate11_cost --ignored --nocapture >/dev/null; grep G11COST $S/logs/g11_cost2_$n.txt; done
echo "##### (h) supervisor as pid 1 vs child, unmarked fast fork chain in a job"
for m in pid1 child; do for life in 0 100; do $S/scripts/g11_pid1.sh $m 8 $life p1-$m-$life 2>&1 | grep PID1RESULT; done; done
echo "##### (F)"
bash $S/scripts/batch_F10.sh 2>&1 | cut -c1-260
echo "##### (a)(2) helpers: 20 runs + 10 interrupted (SIGTERM, resume) + 3 refs"
run_h() { timeout -s KILL 1800 $S/scripts/ns8r.sh $S/scripts/g11_helpers_inner.sh "$@" 2>&1 | grep HELPERS; }
export -f run_h; export S
( for s in $(seq 201 220); do echo "$s run"; done; for s in $(seq 221 230); do echo "$s term"; done; for s in 201 202 203; do echo "$s ref"; done ) | xargs -P 4 -L 1 bash -c 'run_h "$@"' _
for s in 201 202 203; do echo "COMPARE seed=$s $(python3 -I scripts/rescmp.py runs/helpers11/s$s/ref.result runs/helpers11/s$s/run.result)"; done
echo "=== done $(date -u +%FT%TZ)"
