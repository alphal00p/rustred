#!/usr/bin/env bash
# gate8 random read-fault model (transient fd / memory pressure): EMFILE on a random fraction of opens.
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
run() { timeout -s KILL 900 scripts/$1 scripts/g8_inj_inner.sh "${@:2}" 2>&1 | grep -E 'RESULT|recovery: marker|final marker|FINISHED' | cut -c1-240; }
for k in 1 2; do
  run ns8u.sh rand-u-dpstat500-$k u G8_MATCH=/proc/@DP@/stat G8_OPS=open G8_PROB=500 G8_ERRNO=24 &
  run ns8u.sh rand-u-proc300-$k u G8_MATCH=/proc/ G8_OPS=open G8_PROB=300 G8_ERRNO=24 &
  run ns8u.sh rand-u-all050-$k u G8_MATCH=/ G8_OPS=open G8_PROB=50 G8_ERRNO=24 &
  run ns8r.sh rand-r-dp500-$k r G8_MATCH=/proc/@DP@/ G8_OPS=open G8_PROB=500 G8_ERRNO=24 &
  run ns8r.sh rand-r-proc300-$k r G8_MATCH=/proc/ G8_OPS=open G8_PROB=300 G8_ERRNO=24 &
  wait
done
echo "=== done $(date -u +%FT%TZ)"
