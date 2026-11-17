#!/usr/bin/env bash
# lane arrivals: T165 part B as 3 SHARDS (--shard i/3, 32 threads each, MemoryMax 40G); quotas keep <= 96 cores: 500-600 % each while
# T165.A and T167 run, 1600 % each once T165.A is done, 3200 % once T167 is done too; then merge the shards into T165.B/out and mark done
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain; R=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
for i in 0 1 2; do
  q=500; [ $i = 2 ] && q=600
  printf '%s (implementer:arrivals): unpinned, CPUQuota %s %%, nice 10, MemoryMax 40G (unit impl-arrivals-physd-T165B%s: T165 part B shard %s/3, 32 threads; quota raised as T165.A / T167 end; <= 96 cores)\n' "$(date -u +%FT%TZ)" "$q" "$i" "$i" >> $WS/ops/CPU_LEDGER.md
  systemd-run --user --collect --unit=impl-arrivals-physd-T165B$i -p CPUQuota=${q}% -p AllowedCPUs="$cpus" -p MemoryMax=40G -p MemorySwapMax=0 -p Nice=10 -- unshare -Urpf --mount-proc -- bash $P/t165_shard.sh $i 3 32
done
lvl=0
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
  mkdir -p $R/T165.B/out
  python3 -I $P/merge_topup.py $R/T165.B0/out $R/T165.B1/out $R/T165.B/out T165 $R/T165.B2/out > $R/T165.B/merge_shards.json || { echo "$(date -u +%FT%TZ) T165.B shard merge FAILED" >> $R/chain.log; exit 2; }
  cp $R/T165.B0/compute.err $R/T165.B/compute.err
  echo "$(date -u +%FT%TZ) T165.B done (3 shards merged: keys union, senders summed, depth min)" >> $R/chain.log
  touch $R/T165.B/done
else
  echo "$(date -u +%FT%TZ) T165.B shards ended without all done markers" >> $R/chain.log
fi
