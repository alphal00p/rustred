#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8; cd $S
for a in "none-1 none 1" "during-1 during 1" "atlimit-1 atlimit 1" "pidreuse-1 pidreuse 1" "none-2 none 2" "atlimit-2 atlimit 2"; do set -- $a
  ( timeout -s KILL 1500 scripts/ns8r.sh scripts/g8_mj1_inner.sh $1 $2 $3 > $S/logs/mj1_$1.txt 2>&1 ) & done; wait
for f in $S/logs/mj1_*.txt; do echo "== $f"; cat $f; done; echo "=== done $(date -u +%FT%TZ)"
