#!/usr/bin/env bash
# lane arrivals: manager of the running T165.B shards (replaces t165B_shards.sh without its JSON merge): quotas <= 96 cores; when all
# three are done: T165.B/compute.err := shard 0's (every shard computes the full closure), T165.B/done
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; R=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2
lvl=${1:-0}
while :; do
  act=0; for i in 0 1 2; do systemctl --user is-active -q impl-arrivals-physd-T165B$i && act=1; done
  if [ $lvl -lt 1 ] && [ -e $R/T165.A/done ]; then for i in 0 1 2; do systemctl --user set-property --runtime impl-arrivals-physd-T165B$i CPUQuota=1600% 2>/dev/null; done; lvl=1
    echo "$(date -u +%FT%TZ) (implementer:arrivals): T165B shards CPUQuota 1600% each (T165.A done; <= 96 cores)" >> $WS/ops/CPU_LEDGER.md; fi
  if [ $lvl -lt 2 ] && [ -e $R/T165.A/done ] && [ -e $R/T167/done ]; then for i in 0 1 2; do systemctl --user set-property --runtime impl-arrivals-physd-T165B$i CPUQuota=3200% 2>/dev/null; done; lvl=2
    echo "$(date -u +%FT%TZ) (implementer:arrivals): T165B shards CPUQuota 3200% each (T165.A and T167 done; <= 96 cores)" >> $WS/ops/CPU_LEDGER.md; fi
  [ $act = 0 ] && break
  sleep 30
done
if [ -e $R/T165.B0/done ] && [ -e $R/T165.B1/done ] && [ -e $R/T165.B2/done ]; then
  cp $R/T165.B0/compute.err $R/T165.B/compute.err
  echo "$(date -u +%FT%TZ) T165.B done (3 shards: their files are merged into the T162 delivery by key union / depth min)" >> $R/chain.log
  touch $R/T165.B/done
else
  echo "$(date -u +%FT%TZ) T165.B shards ended without all done markers" >> $R/chain.log
fi
