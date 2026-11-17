#!/usr/bin/env bash
# waits on my units' events: T162 rows boundary / unit2 end / drill run4 end or failure / new dips; prints the event and exits
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; R2=$S/physchain/run2/chain.log; R4=$S/physchain/run4/chain.log
l2=$(wc -l < $R2); l4=$(wc -l < $R4); ld=$(wc -l < $S/physchain/run2/dips.log 2>/dev/null || echo 0); t0=$(date +%s)
while :; do
  n=$(tail -n +$((l2+1)) $R2 | grep -E "T162 closure rows:|unit2 ended|T162 closure.*FAILED|OOM" | head -3)
  [ -n "$n" ] && { echo "RUN2: $n"; exit 0; }
  n=$(tail -n +$((l4+1)) $R4 | grep -E "CHAIN DONE|FAILED|OOM|none ready" | head -3)
  [ -n "$n" ] && { echo "RUN4: $n"; exit 0; }
  d=$(wc -l < $S/physchain/run2/dips.log 2>/dev/null || echo 0); [ "$d" -gt "$ld" ] && { echo "DIP: $(tail -1 $S/physchain/run2/dips.log)"; exit 0; }
  systemctl --user is-active -q impl-arrivals-t162closure || systemctl --user is-active -q impl-arrivals-t162closure2 || [ -s $S/physchain/t162final/rows.done ] || { echo "T162 closure unit not active and no rows.done"; exit 0; }
  [ $(( $(date +%s) - t0 )) -gt 5400 ] && { echo "TIMEOUT 90 min: $(tail -1 $S/physchain/t162final/rss.log); run4 done $(ls $S/physchain/run4/*/done 2>/dev/null | wc -l)/66"; exit 0; }
  sleep 30
done
