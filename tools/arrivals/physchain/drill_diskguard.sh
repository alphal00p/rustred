#!/usr/bin/env bash
# lane arrivals: protect the DRILL run4 walks from a full pool (2026-11-17 ~12:00Z the pool lost ~5 GB/min, ~94 % from other
# datasets): pool free < 60 GB -> freeze every active impl-arrivals-drill-* unit (no write, no crash); >= 100 GB -> thaw them.
# Actions in run4/chain.log and the CPU ledger. Exact unit names of mine only.
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; R=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run4
frozen=0
while :; do
  fr=$(df -BG /common/build-cache | tail -1 | awk '{print int($4)}')
  if [ $frozen = 0 ] && [ $fr -lt 60 ]; then
    us=$(systemctl --user list-units --no-pager --plain --no-legend 'impl-arrivals-drill-*' | awk '{print $1}')
    for u in $us; do systemctl --user freeze $u 2>/dev/null; done
    echo "$(date -u +%FT%TZ) DISK GUARD: pool free $fr GB < 60: froze $(echo $us | wc -w) drill units" >> $R/chain.log
    printf '%s (implementer:arrivals): froze drill units (pool free %s GB < 60): %s\n' "$(date -u +%FT%TZ)" "$fr" "$(echo $us)" >> $WS/ops/CPU_LEDGER.md
    frozen=1
  elif [ $frozen = 1 ] && [ $fr -ge 100 ]; then
    us=$(systemctl --user list-units --no-pager --plain --no-legend 'impl-arrivals-drill-*' | awk '{print $1}')
    for u in $us; do systemctl --user thaw $u 2>/dev/null; done
    echo "$(date -u +%FT%TZ) DISK GUARD: pool free $fr GB >= 100: thawed the drill units" >> $R/chain.log
    printf '%s (implementer:arrivals): thawed drill units (pool free %s GB)\n' "$(date -u +%FT%TZ)" "$fr" >> $WS/ops/CPU_LEDGER.md
    frozen=0
  fi
  sleep 20
done
