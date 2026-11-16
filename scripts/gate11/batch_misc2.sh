#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate11; cd $S
bash scripts/batch_bcd.sh 2>&1 | cut -c1-260
echo "##### (a)(2) helpers, SIGKILL at a random 3-12 s then resume (and once more after an exit 8)"
run_h() { timeout -s KILL 1800 $S/scripts/ns8r.sh $S/scripts/g11_helpers_inner.sh "$@" 2>&1 | grep HELPERS; }
export -f run_h; export S
for s in $(seq 241 252); do echo "$s kill"; done | xargs -P 4 -L 1 bash -c 'run_h "$@"' _
echo "=== done $(date -u +%FT%TZ)"
