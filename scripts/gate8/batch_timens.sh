#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate8; cd $S
for v in t1 t2 t3 t4 t5 t6; do ( timeout -s KILL 900 scripts/ns8r.sh scripts/g8_timens_inner.sh tn-$v $v > $S/logs/timens_$v.txt 2>&1 ) & done; wait
for v in t1 t2 t3 t4 t5 t6; do echo "== $v"; cat $S/logs/timens_$v.txt; done; echo "=== done $(date -u +%FT%TZ)"
