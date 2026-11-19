#!/usr/bin/env bash
# one_job.sh MODE TAG OPS ERRNO N MATCH : one injection run (one fault: the N-th candidate), result line only
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate9
mode=$1 tag=$2 ops=$3 err=$4 n=$5 match=$6
w=$S/scripts/ns8r.sh; [ "$mode" = u ] && w=$S/scripts/ns8u.sh
out=$(timeout -s KILL 900 $w $S/scripts/g8_inj_inner.sh $tag $mode "G8_MATCH=$match" G8_OPS=$ops G8_ERRNO=$err G8_AT=$n 2>&1)
r=$(echo "$out" | grep '^RESULT'); [ -n "$r" ] || r="RESULT tag=$tag NO_RESULT"
f=$(echo "$out" | grep -m1 'fault:' | awk '{print $6, $7}')
echo "$r op=$ops errno=$err n=$n fault=[$f]"
