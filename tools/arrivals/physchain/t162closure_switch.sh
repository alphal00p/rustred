#!/usr/bin/env bash
# at the rows/closure boundary of the T162 closure run: stop the original unit (a38 compute, 32 threads) and start the exact
# closure on a40 (first-match histogram) with 96 threads under MemoryMax $1 (GB, the measured need)
F=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/t162final; WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; R=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/run2
until [ -s $F/rows.done ]; do sleep 10; done
systemctl --user stop impl-arrivals-t162closure
# the measured need: ~4 GB fixed + 60 B per closure key (measured 50-54 B per key in the rows / closure steps), at most 96
MEM=$(python3 -I -c 'import json,sys,math; n=json.load(open(sys.argv[1]))["closure_keys"]; print(min(96, max(24, math.ceil(4 + 60e-9 * n))))' $F/rows.summary.json)
echo "$(date -u +%FT%TZ) T162 closure rows: $(python3 -I -c 'import json,sys; s=json.load(open(sys.argv[1])); print(s["start_keys"], "start keys,", s["closure_keys"], "closure keys,", s["open"], "open")' $F/rows.summary.json); exact closure MemoryMax ${MEM}G" >> $R/chain.log
while :; do av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo); [ $((av - MEM)) -ge 60 ] && break; sleep 30; done
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
printf '%s (implementer:arrivals): unpinned, CPUQuota 9600 %%, nice 10, MemoryMax %sG (unit impl-arrivals-t162closure2: T162 exact closure list on a40 with the first-match histogram, 96 threads; coordinator: <= 96 GB, gate 60)\n' "$(date -u +%FT%TZ)" "$MEM" >> $WS/ops/CPU_LEDGER.md
echo "$(date -u +%FT%TZ) T162 closure list: exact closure step on a40 (96 threads, MemoryMax ${MEM}G)" >> $R/chain.log
systemd-run --user --wait --collect --unit=impl-arrivals-t162closure2 -p CPUQuota=9600% -p AllowedCPUs="$cpus" -p MemoryMax=${MEM}G -p MemorySwapMax=0 -p Nice=10 -- unshare -Urpf --mount-proc -- bash $F/closure2.sh
echo "$(date -u +%FT%TZ) T162 closure list unit2 ended rc=$?" >> $R/chain.log
