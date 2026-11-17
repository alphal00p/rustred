#!/usr/bin/env bash
# lane arrivals, physical chain run (coordinator RAM reservation 2026-11-16): host MemAvailable < 75 GB on two samples 20 s
# apart -> PAUSE (freeze: checkpointed in memory, progress kept) my YOUNGEST running impl-arrivals-phys / physd unit; thaw my frozen
# units when >= 85 GB; < 50 GB on two samples (the user's floor) -> STOP the youngest (thawed first). Exact unit names; every
# action in the CPU ledger and the chain log.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; LOG=$1; low=0; crit=0
act() { printf '%s (implementer:arrivals): %s\n' "$(date -u +%FT%TZ)" "$1" >> $WS/ops/CPU_LEDGER.md; echo "$(date -u +%FT%TZ) GUARD: $1" >> "$LOG"; }
units() { systemctl --user list-units --no-pager --plain --no-legend 'impl-arrivals-phys-*' 'impl-arrivals-physd-*' | awk '{print $1}'; }
youngest() {
  local best="" bt=0 u t
  for u in $(units); do
    [ "$(systemctl --user show "$u" -p FreezerState --value)" = frozen ] && continue
    t=$(systemctl --user show "$u" -p ActiveEnterTimestampMonotonic --value); [ "${t:-0}" -gt "$bt" ] && { bt=$t; best=$u; }
  done
  echo "$best"
}
frozen_units() { local u; for u in $(units); do [ "$(systemctl --user show "$u" -p FreezerState --value)" = frozen ] && echo "$u"; done; }
while true; do
  av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
  if [ "$av" -lt 75 ]; then low=$((low+1)); else low=0; fi
  if [ "$av" -lt 50 ]; then crit=$((crit+1)); else crit=0; fi
  if [ $crit -ge 2 ]; then
    y=$(youngest); [ -z "$y" ] && y=$(frozen_units | head -1)
    if [ -n "$y" ]; then systemctl --user thaw "$y" 2>/dev/null; systemctl --user stop "$y"; act "stopped $y: host MemAvailable $av GB < 50"; fi
    crit=0; low=0
  elif [ $low -ge 2 ]; then
    y=$(youngest)
    if [ -n "$y" ] && systemctl --user freeze "$y"; then act "paused (frozen) youngest walk $y: host MemAvailable $av GB < 75"; fi
    low=0; sleep 40
  fi
  fz=$(frozen_units)
  if [ -n "$fz" ] && [ "$av" -ge 85 ]; then
    u=$(echo "$fz" | head -1); systemctl --user thaw "$u" 2>/dev/null && act "resumed (thawed) $u: host MemAvailable $av GB >= 85"
  fi
  sleep 20
done
