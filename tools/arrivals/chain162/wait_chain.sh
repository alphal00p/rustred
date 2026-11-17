#!/usr/bin/env bash
# lane arrivals: block up to $1 seconds on the chain run $2; return early on an end event of a node (done / FAILED / OOM /
# NOT STARTED / L17 / CHAIN DONE / nodes done) and print the new events plus the running unit's state.
S=${1:-580}; R=$2; L=$R/chain.log; n0=$(wc -l < "$L"); t0=$(date +%s)
while [ $(( $(date +%s) - t0 )) -lt "$S" ]; do
  if tail -n +$((n0+1)) "$L" | grep -qE " done |FAILED|OOM|NOT STARTED|L17: stopped|CHAIN DONE|nodes done"; then break; fi
  sleep 15
done
tail -n +$((n0+1)) "$L" | grep -vE "waits for memory" | tail -12
echo "--- $(date -u +%T) avail $(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo) GB; units: $(systemctl --user list-units --no-pager --plain --no-legend 'impl-arrivals-chain-*' | awk '{print $1}' | tr '\n' ' ')"
tail -n 1 "$L"
