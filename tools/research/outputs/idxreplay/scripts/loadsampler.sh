#!/usr/bin/env bash
# Sample busy jiffies of a CPU range and of our processes every 5 s (foreign-load receipt).
# usage: loadsampler.sh FIRST LAST OUT UNTIL_EPOCH
F=$1; L=$2; OUT=$3; UNTIL=$4
while [ $(date +%s) -lt $UNTIL ]; do
  now=$(date +%s)
  read busy total < <(awk -v f=$F -v l=$L '/^cpu[0-9]+ /{id=substr($1,4)+0; if(id>=f && id<=l){t=0; for(i=2;i<=9;i++) t+=$i; b=t-$5-$6; B+=b; T+=t}} END{print B, T}' /proc/stat)
  mine=0
  for p in $(pgrep -f "idxreplay-v1|rustred-trace-edbe2729"); do
    j=$(awk '{print $14+$15}' /proc/$p/stat 2>/dev/null); mine=$((mine + ${j:-0}))
  done
  echo "$now $busy $total $mine" >> $OUT
  sleep 5
done
