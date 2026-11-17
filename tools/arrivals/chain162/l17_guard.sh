#!/usr/bin/env bash
# lane arrivals: L17 guard. When host MemAvailable < 75 GB on two samples 20 s apart, stop MY largest impl-arrivals unit
# (exact unit name via systemctl --user stop), write a ledger line and a chain.log line.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; LOG=$1; low=0
while true; do
  av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
  if [ "$av" -lt 75 ]; then low=$((low+1)); else low=0; fi
  if [ $low -ge 2 ]; then
    big=""; bm=0
    for u in $(systemctl --user list-units --no-pager --plain --no-legend 'impl-arrivals*' | awk '{print $1}'); do
      m=$(systemctl --user show "$u" -p MemoryCurrent --value); [ "$m" -gt "$bm" ] 2>/dev/null && { bm=$m; big=$u; }
    done
    if [ -n "$big" ]; then
      systemctl --user stop "$big"
      printf '%s (implementer:arrivals): stopped %s (L17: host MemAvailable %s GB < 75; my largest job, %s GB)\n' "$(date -u +%FT%TZ)" "$big" "$av" "$((bm/1000000000))" >> $WS/ops/CPU_LEDGER.md
      echo "$(date -u +%FT%TZ) L17: stopped $big ($((bm/1000000000)) GB), host avail ${av} GB" >> "$LOG"
    fi
    low=0; sleep 60
  fi
  sleep 20
done
