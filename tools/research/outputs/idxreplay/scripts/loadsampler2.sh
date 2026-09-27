#!/usr/bin/env bash
# Sample busy jiffies of a CPU list and of our processes every 5 s (foreign-load receipt).
# usage: loadsampler2.sh CPULIST(e.g. 32-39,288-295) OUT UNTIL_EPOCH PGREP_PATTERN
LIST=$1; OUT=$2; UNTIL=$3; PAT=$4
echo "# cpus $LIST; columns: epoch busy_jiffies total_jiffies own_process_jiffies (pattern $PAT)" >> $OUT
while [ $(date +%s) -lt $UNTIL ]; do
  now=$(date +%s)
  read busy total < <(awk -v list="$LIST" 'BEGIN{n=split(list,p,","); for(i=1;i<=n;i++){if(split(p[i],r,"-")==2){for(c=r[1];c<=r[2];c++)S[c]=1}else S[p[i]]=1}} /^cpu[0-9]+ /{id=substr($1,4)+0; if(id in S){t=0; for(i=2;i<=9;i++) t+=$i; b=t-$5-$6; B+=b; T+=t}} END{print B, T}' /proc/stat)
  mine=0
  for p in $(pgrep -f "$PAT"); do
    j=$(awk '{print $14+$15}' /proc/$p/stat 2>/dev/null); mine=$((mine + ${j:-0}))
  done
  echo "$now $busy $total $mine" >> $OUT
  sleep 5
done
