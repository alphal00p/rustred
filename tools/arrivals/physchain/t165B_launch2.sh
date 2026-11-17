#!/usr/bin/env bash
# lane arrivals: relaunch of the T165.B shards on a38 (bounded closure, small per-thread buffers): one by one, each when host
# MemAvailable - 28 GB x (shards started in the last 10 min + 1) >= 60 GB (coordinator: shards gate 60); CPUQuota 3200% each
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain; R=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
k=0
for i in 0 1 2; do
  while :; do av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo); [ $((av - 28 * (k + 1))) -ge 60 ] && break; sleep 60; done
  printf '%s (implementer:arrivals): unpinned, CPUQuota 3200 %%, nice 10, MemoryMax 40G (unit impl-arrivals-physd-T165B%s: T165 part B shard %s/3 on a38, 32 threads; %s GB available)\n' "$(date -u +%FT%TZ)" "$i" "$i" "$av" >> $WS/ops/CPU_LEDGER.md
  systemd-run --user --collect --unit=impl-arrivals-physd-T165B$i -p CPUQuota=3200% -p AllowedCPUs="$cpus" -p MemoryMax=40G -p MemorySwapMax=0 -p Nice=10 -- unshare -Urpf --mount-proc -- bash $P/t165_shard.sh $i 3 32
  k=$((k + 1))
done
exec bash $P/t165B_shards3.sh 1
