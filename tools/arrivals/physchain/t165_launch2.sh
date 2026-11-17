#!/usr/bin/env bash
# lane arrivals: launch a T165 top-up part as unit impl-arrivals-physd-T165<part> once host MemAvailable - expected peak >= 75 GB
# (coordinator 06:14Z gate); MemoryMax 40G, CPUQuota threads x 100 %, nice 10, private pid namespace.  Usage: t165_launch.sh <part> <incoming list> <threads> <expected GB>
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; P=$S/physchain; R=$P/run2
PART=$1; INC=$2; TH=$3; EX=$4
while :; do
  av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
  [ $((av - EX)) -ge 75 ] && break
  echo "$(date -u +%FT%TZ) T165.$PART waits for memory ($av GB available, expected peak $EX GB)" >> $R/chain.log; sleep 60
done
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
printf '%s (implementer:arrivals): unpinned, CPUQuota %s00 %%, nice 10, MemoryMax 40G (unit impl-arrivals-physd-T165%s: T165 top-up part %s, %s threads, %s GB available at start)\n' "$(date -u +%FT%TZ)" "$TH" "$PART" "$PART" "$TH" "$av" >> $WS/ops/CPU_LEDGER.md
echo "$(date -u +%FT%TZ) T165.$PART starts: $av GB available, expected peak $EX GB, cap 40 GB, threads $TH" >> $R/chain.log
systemd-run --user --wait --collect --unit=impl-arrivals-physd-T165$PART -p CPUQuota=${TH}00% -p AllowedCPUs="$cpus" -p MemoryMax=40G \
  -p MemorySwapMax=0 -p Nice=10 -- unshare -Urpf --mount-proc -- bash $P/t165_part2.sh $S/bin/rr_arrivals.a29 $PART "$INC" $TH > $R/unit.T165$PART.txt 2>&1
echo "$(date -u +%FT%TZ) T165.$PART unit ended rc=$?" >> $R/chain.log
