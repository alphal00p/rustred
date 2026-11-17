#!/usr/bin/env bash
source /shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/env.sh
export TMPDIR=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone/tmp; mkdir -p $TMPDIR
bash /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/cones/excess_one.sh /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/bin/rr_arrivals.a27 /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone/excess_rel T266 /common/build-cache/vhirschi/p_integral_dev/t266-close/ship/T266-box.jsonl /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/phys/t266rel/T266-relations.table.jsonl
