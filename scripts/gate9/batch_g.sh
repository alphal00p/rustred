#!/usr/bin/env bash
# gate9 (g): gate8's own reproductions on 9876cba2 (G8-B1 parity EMFILE/EIO, random 50 %, MJ8-1 t6 + MJ-2 variants, gate7 B1 verbatim)
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
run() { timeout -s KILL 900 scripts/$1 scripts/${2} "${@:3}" 2>&1 | grep -E 'RESULT|recovery: marker|final marker|FINISHED|session 2|rc=' | cut -c1-260; }
run ns8u.sh g8_inj_inner.sh g-par-e24 u G8_MATCH=/proc/@DP@/stat G8_OPS=open G8_PERTHREAD=1 G8_FROM=2 G8_EVERY=2 G8_ERRNO=24 &
run ns8u.sh g8_inj_inner.sh g-par-e5 u G8_MATCH=/proc/@DP@/stat G8_OPS=open G8_PERTHREAD=1 G8_FROM=2 G8_EVERY=2 G8_ERRNO=5 &
run ns8u.sh g8_inj_inner.sh g-ctl-u u G8_MATCH=/nonexistent-path G8_OPS=open &
run ns8u.sh g8_inj_inner.sh g-rand500-1 u G8_MATCH=/proc/@DP@/stat G8_OPS=open G8_PROB=500 G8_ERRNO=24 &
run ns8u.sh g8_inj_inner.sh g-rand500-2 u G8_MATCH=/proc/@DP@/stat G8_OPS=open G8_PROB=500 G8_ERRNO=24 &
run ns8u.sh g8_inj_inner.sh g-rand500-3 u G8_MATCH=/proc/@DP@/ G8_OPS=open,read,stat G8_PROB=500 G8_ERRNO=5 &
wait
for v in t1 t5 t6; do run ns8r.sh g8_timens_inner.sh g-$v $v & done
run ns8u.sh g8_g7partial_inner.sh g-g7p2 2; run ns8u.sh g8_g7partial_inner.sh g-g7p0 0 &
wait
echo "=== done $(date -u +%FT%TZ)"
