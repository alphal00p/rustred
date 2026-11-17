#!/usr/bin/env bash
# lane arrivals, physical chain guard v8 (implementer / coordinator 2026-11-16T12:3xZ, item 15):
# - PROTECTED walks: impl-arrivals-physd-T165* / -T180 / -T163 are never stopped nor frozen by this guard.
# - host MemAvailable < 60 GB on two samples 20 s apart: freeze (pause) the YOUNGEST unprotected walk holding >= 6 GB; thaw at >= 70.
# - < 50 GB on two samples (the user's floor): record the DIP in R/dips.log (clock, available, top RSS holders by command name) for an
#   immediate one-line message; then stop the LARGEST unprotected walk holding >= 6 GB that can resume (its rows saved: R/<X>/rows.done);
#   if none qualifies, log that the protected walks are kept (our walks cannot restore the floor: the dips are other jobs').
# Every action in the CPU ledger and the chain log. Exact unit names only.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; LOG=$1; R=$(dirname "$LOG"); low=0; crit=0
act() { printf '%s (implementer:arrivals): %s\n' "$(date -u +%FT%TZ)" "$1" >> $WS/ops/CPU_LEDGER.md; echo "$(date -u +%FT%TZ) GUARD: $1" >> "$LOG"; }
units() { systemctl --user list-units --no-pager --plain --no-legend 'impl-arrivals-phys-*' 'impl-arrivals-physd-*' | awk '{print $1}'; }
protected() { case "$1" in impl-arrivals-physd-T165*|impl-arrivals-physd-T180.service|impl-arrivals-physd-T163.service|impl-arrivals-physd-T167.service) return 0;; esac; return 1; }
memgb() { local m; m=$(systemctl --user show "$1" -p MemoryCurrent --value); [ -z "$m" ] || [ "$m" = "[not set]" ] && m=0; echo $((m / 1073741824)); }
node_of() { local x=${1#impl-arrivals-physd-}; x=${x#impl-arrivals-phys-}; echo "${x%.service}"; }
youngest_freezable() {
  local best="" bt=0 u t
  for u in $(units); do
    protected "$u" && continue
    [ "$(systemctl --user show "$u" -p FreezerState --value)" = frozen ] && continue
    [ "$(memgb "$u")" -lt 6 ] && continue
    t=$(systemctl --user show "$u" -p ActiveEnterTimestampMonotonic --value); [ "${t:-0}" -gt "$bt" ] && { bt=$t; best=$u; }
  done
  echo "$best"
}
largest_stoppable() {
  local best="" bm=0 u m
  for u in $(units); do
    protected "$u" && continue
    m=$(memgb "$u"); [ "$m" -lt 6 ] && continue
    [ -s "$R/$(node_of "$u")/rows.done" ] || continue
    [ "$m" -gt "$bm" ] && { bm=$m; best=$u; }
  done
  echo "$best"
}
# only the walks this guard may freeze (unprotected) are thawed by it: protected walks frozen by the operator stay frozen
frozen_units() { local u; for u in $(units); do protected "$u" && continue; [ "$(systemctl --user show "$u" -p FreezerState --value)" = frozen ] && echo "$u"; done; }
while true; do
  av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
  if [ "$av" -lt 60 ]; then low=$((low+1)); else low=0; fi
  if [ "$av" -lt 50 ]; then crit=$((crit+1)); else crit=0; fi
  if [ $crit -ge 2 ]; then
    top=$(ps -eo rss=,comm= --sort=-rss | head -6 | awk '{printf "%s %dG; ", $2, $1/1048576}')
    echo "$(date -u +%FT%TZ) available ${av} GB; top RSS: ${top}" >> "$R/dips.log"
    y=$(largest_stoppable)
    if [ -n "$y" ]; then systemctl --user thaw "$y" 2>/dev/null; systemctl --user stop "$y"; act "stopped $y (unprotected, rows saved): host MemAvailable $av GB < 50"
    else act "host MemAvailable $av GB < 50: no allowed victim (T165/T180/T163 protected; others < 6 GB or without saved rows): kept"; fi
    crit=0; low=0; sleep 40
  elif [ $low -ge 2 ]; then
    y=$(youngest_freezable)
    if [ -n "$y" ] && systemctl --user freeze "$y"; then act "paused (frozen) youngest unprotected walk $y: host MemAvailable $av GB < 60"; fi
    low=0; sleep 40
  fi
  fz=$(frozen_units)
  if [ -n "$fz" ] && [ "$av" -ge 70 ]; then
    u=$(echo "$fz" | head -1); systemctl --user thaw "$u" 2>/dev/null && act "resumed (thawed) $u: host MemAvailable $av GB >= 70"
  fi
  sleep 20
done
