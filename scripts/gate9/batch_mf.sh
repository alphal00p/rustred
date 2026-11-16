#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
run() { timeout -s KILL 900 scripts/$1 scripts/g9_mf_inner.sh "${@:2}" 2>&1; }
# stat #1 (supervisor manifest.exists), stat #2 (create_with exists) EIO; readdir #1 of the ledger dir EIO (dropped by .flatten())
run ns8r.sh mf-r1 r G8_MATCH=@CDIR@/ledger G8_OPS=stat,readdir G8_AT=1,2,4 G8_ERRNO=5 &
run ns8r.sh mf-r2 r G8_MATCH=@CDIR@/ledger G8_OPS=stat,readdir G8_AT=1,2,3,4 G8_ERRNO=5 &
run ns8r.sh mf-r0 r G8_MATCH=@CDIR@/ledger G8_OPS=stat,readdir G8_AT=1 G8_ERRNO=5 &
run ns8r.sh mf-rctl r G8_MATCH=/nonexistent G8_OPS=stat &
run ns8u.sh mf-u1 u G8_MATCH=@CDIR@/ledger G8_OPS=stat,readdir G8_AT=1,2,4 G8_ERRNO=5 &
wait
echo "=== done $(date -u +%FT%TZ)"
