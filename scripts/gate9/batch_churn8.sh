#!/usr/bin/env bash
# gate9 G9-B2 deterministic: the resumed supervisor's opens of the churner's /proc/<pid>/task/<tid>/* files are DELAYED by
# DELAY_US (a descheduled supervisor; no read fails), so each listed thread really exits before it is read.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S; export DELAY_US=${1:-2000}
for a in "D-0-1 crash 0" "D-50-1 crash 50" "D-500-1 crash 500" "D-0-2 crash 0" "D-50-2 crash 50" "D-500-2 crash 500" "D-nz crash 0 nozombie"; do
  set -- $a; timeout -s KILL 1500 scripts/ns8r.sh scripts/g9_churn_inner.sh $1-d$DELAY_US $2 $3 $4 2>&1 | grep -E "RESULT|marker sweep"; done
