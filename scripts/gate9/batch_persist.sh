#!/usr/bin/env bash
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9; cd $S
r() { t0=$(date +%s); out=$(timeout -s KILL 2500 scripts/ns8r.sh scripts/g9_live_inner.sh "$@" 2>&1 | grep RESULT); echo "$out secs=$(( $(date +%s)-t0 ))"; }
export G8_TIMEOUT=2400
EXTRA_RES=',"unproven_end_limit_s":60' r P-live-100-24-l60 r G8_MATCH=/proc G8_OPS=open,read,getdents,readdir,stat G8_PROB=100 G8_ERRNO=24 &
EXTRA_RES=',"unproven_end_limit_s":60' r P-live-300-12-l60 r G8_MATCH=/proc G8_OPS=open,read,getdents,readdir,stat G8_FROM=200 G8_PROB=300 G8_ERRNO=12 &
r P-live-100-24-def r G8_MATCH=/proc G8_OPS=open,read,getdents,readdir,stat G8_PROB=100 G8_ERRNO=24 &
r P-live-300-5-def r G8_MATCH=/proc G8_OPS=open,read,getdents,readdir,stat G8_FROM=200 G8_PROB=300 G8_ERRNO=5 &
wait; echo "=== done $(date -u +%FT%TZ)"
