#!/usr/bin/env bash
# lane arrivals: for the heavy late nodes of the DRILL run4, start a 4-shard race (drill_race.sh, MEMMAX 32, EXPECT 16) as soon
# as the node's own walk has saved its rows and entered compute (rows.summary.json present), once per node.
# Usage: auto_race.sh <node> [node ..]
P=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain; R=$P/run4
left="$*"
while [ -n "$left" ]; do
  nl=""
  for x in $left; do
    if [ -e $R/$x/done ]; then continue; fi
    if [ -s $R/$x/rows.summary.json ] && [ -s $R/$x/rows.jsonl ]; then
      echo "$(date -u +%FT%TZ) auto_race: $x rows saved -> 4-shard race" >> $R/chain.log
      MEMMAX=32 EXPECT=16 setsid nohup bash $P/drill_race.sh $x 4 8 > $R/race.$x.out 2>&1 < /dev/null &
    else
      nl="$nl $x"
    fi
  done
  left=$(echo $nl)
  [ -n "$left" ] && sleep 30
done
