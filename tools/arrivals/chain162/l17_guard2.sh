#!/usr/bin/env bash
# lane arrivals: L17 guard (pause, not stop). Host MemAvailable < 75 GB on two samples 20 s apart: FREEZE my largest
# running impl-arrivals unit (cgroup freezer: it keeps its memory and its progress, allocates nothing more); THAW it when
# MemAvailable is back >= 85 GB. < 60 GB on two samples (near the user's 50 GB floor): STOP my largest unit (frees memory).
# Exact unit names only; every action in the CPU ledger and the chain log.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; LOG=$1; low=0; crit=0; frozen=""
act() { printf '%s (implementer:arrivals): %s\n' "$(date -u +%FT%TZ)" "$1" >> $WS/ops/CPU_LEDGER.md; echo "$(date -u +%FT%TZ) L17: $1" >> "$LOG"; }
largest() {
  local big="" bm=0 u m
  for u in $(systemctl --user list-units --no-pager --plain --no-legend 'impl-arrivals*' | awk '{print $1}'); do
    m=$(systemctl --user show "$u" -p MemoryCurrent --value); [ "$m" -gt "$bm" ] 2>/dev/null && { bm=$m; big=$u; }
  done
  echo "$big $((bm/1000000000))"
}
while true; do
  av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
  if [ "$av" -lt 75 ]; then low=$((low+1)); else low=0; fi
  if [ "$av" -lt 60 ]; then crit=$((crit+1)); else crit=0; fi
  if [ $crit -ge 2 ]; then
    read -r big gb <<< "$(largest)"
    if [ -n "$big" ]; then systemctl --user stop "$big"; act "stopped $big ($gb GB): host MemAvailable $av GB < 60"; [ "$big" = "$frozen" ] && frozen=""; fi
    crit=0; low=0
  elif [ $low -ge 2 ] && [ -z "$frozen" ]; then
    read -r big gb <<< "$(largest)"
    if [ -n "$big" ] && systemctl --user freeze "$big"; then frozen=$big; act "paused (frozen) $big ($gb GB): host MemAvailable $av GB < 75"; fi
    low=0
  fi
  if [ -n "$frozen" ] && [ "$av" -ge 85 ]; then
    systemctl --user thaw "$frozen" 2>/dev/null; act "resumed (thawed) $frozen: host MemAvailable $av GB >= 85"; frozen=""
  fi
  if [ -n "$frozen" ] && ! systemctl --user is-active -q "$frozen"; then frozen=""; fi
  sleep 20
done
