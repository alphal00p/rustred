#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
run() { timeout -s KILL 900 scripts/$1 scripts/g9_mf_inner.sh "${@:2}" 2>&1; }
run ns8r.sh mf2-r5 r G8_MATCH=@CDIR@/ledger G8_OPS=stat,readdir G8_AT=1,2,5 G8_ERRNO=5 &
run ns8r.sh mf2-r6 r G8_MATCH=@CDIR@/ledger G8_OPS=stat,readdir G8_AT=1,2,6 G8_ERRNO=5 &
run ns8r.sh mf2-r12 r G8_MATCH=@CDIR@/ledger G8_OPS=stat G8_AT=1,2 G8_ERRNO=5 &
run ns8u.sh mf2-u5 u G8_MATCH=@CDIR@/ledger G8_OPS=stat,readdir G8_AT=1,2,5 G8_ERRNO=5 &
run ns8r.sh mf2-r5m r G8_MATCH=@CDIR@/ledger G8_OPS=stat,readdir G8_AT=1,2,5 G8_ERRNO=12 &
wait
echo "=== done $(date -u +%FT%TZ)"
