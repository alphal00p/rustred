#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
for a in "ch-crash-50 crash 50" "ch-crash-0 crash 0" "ch-live-50 live 50" "ch-crash-0-nz crash 0 nozombie"; do
  timeout -s KILL 1000 scripts/ns8r.sh scripts/g9_churn_inner.sh $a 2>&1 | grep -E "RESULT|log:|session 1"; done
