#!/usr/bin/env bash
# lane arrivals: L17 guard (pause, not stop). Host MemAvailable < 75 GB on two samples 20 s apart: FREEZE my largest
# running impl-arrivals unit (cgroup freezer: memory and progress kept, nothing more allocated); THAW my frozen units when
# MemAvailable >= 85 GB. < 60 GB on two samples (near the user's 50 GB floor): STOP my largest unit (thawed first: a frozen
# process cannot take the stop signal). The frozen state is read from systemd (FreezerState) at every step.
# Exact unit names only; every action in the CPU ledger and the chain log.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; LOG=$1; low=0; crit=0
act() { printf '%s (implementer:arrivals): %s\n' "$(date -u +%FT%TZ)" "$1" >> $WS/ops/CPU_LEDGER.md; echo "$(date -u +%FT%TZ) L17: $1" >> "$LOG"; }
units() { systemctl --user list-units --no-pager --plain --no-legend 'impl-arrivals*' | awk '{print $1}'; }
largest() {
  local big="" bm=0 u m
  for u in $(units); do m=$(systemctl --user show "$u" -p MemoryCurrent --value); [ "$m" -gt "$bm" ] 2>/dev/null && { bm=$m; big=$u; }; done
  echo "$big $((bm/1000000000))"
}
frozen_units() { local u; for u in $(units); do [ "$(systemctl --user show "$u" -p FreezerState --value)" = frozen ] && echo "$u"; done; }
while true; do
  av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
  if [ "$av" -lt 75 ]; then low=$((low+1)); else low=0; fi
  if [ "$av" -lt 60 ]; then crit=$((crit+1)); else crit=0; fi
  fz=$(frozen_units)
  if [ $crit -ge 2 ]; then
    read -r big gb <<< "$(largest)"
    if [ -n "$big" ]; then
      systemctl --user thaw "$big" 2>/dev/null; systemctl --user stop "$big"
      act "stopped $big ($gb GB): host MemAvailable $av GB < 60"
    fi
    crit=0; low=0
  elif [ $low -ge 2 ] && [ -z "$fz" ]; then
    read -r big gb <<< "$(largest)"
    if [ -n "$big" ] && systemctl --user freeze "$big"; then act "paused (frozen) $big ($gb GB): host MemAvailable $av GB < 75"; fi
    low=0
  fi
  if [ -n "$fz" ] && [ "$av" -ge 85 ]; then
    for u in $fz; do systemctl --user thaw "$u" 2>/dev/null && act "resumed (thawed) $u: host MemAvailable $av GB >= 85"; done
  fi
  sleep 20
done
