#!/usr/bin/env bash
source /shared/vhirschi/t3code/vakint_dev/p_integral_dev/ops/env.sh
export TMPDIR=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone/tmp; mkdir -p $TMPDIR
echo "$(date -u +%FT%TZ) start" >> /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone/excess_run.log
xargs -a /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone/excess_jobs.txt -P 4 -L 1 bash /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/cones/excess_one.sh /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/bin/rr_arrivals.a27 /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone/excess
echo "$(date -u +%FT%TZ) end" >> /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physcone/excess_run.log
