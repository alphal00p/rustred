#!/usr/bin/env bash
# lane arrivals: after wave 1 (driver3 pid given), check the streamed build on T188 (chain mode) against a9 and run
# waves 2-5 with it if byte-identical, else with a9 (one node at a time, 40 GB caps).
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals; D=$S/chain162; R=$S/runs/T188cpu13
while kill -0 "$1" 2>/dev/null; do sleep 30; done
while :; do av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo); [ $((av - 8)) -ge 100 ] && break; sleep 60; done
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
printf '%s (implementer:arrivals): unpinned, CPUQuota 800 %%, nice 10, MemoryMax 8G (unit impl-arrivals-T188cpu13, a13 equality before waves 2-5)\n' "$(date -u +%FT%TZ)" >> $WS/ops/CPU_LEDGER.md
systemd-run --user --wait --collect --unit=impl-arrivals-T188cpu13 -p CPUQuota=800% -p AllowedCPUs="$cpus" -p MemoryMax=8G -p MemorySwapMax=0 -p Nice=10 -- unshare -Urpf --mount-proc -- bash $R/run.sh
ok=1
for f in $S/runs/T188cpu/a9/out/*.arrivals.jsonl; do
  [ "$(sha256sum < "$f")" = "$(sha256sum < "$R/a13/out/$(basename "$f")" 2>/dev/null)" ] || ok=0
done
echo "$(date -u +%FT%TZ) a13 T188 equality: ok=$ok" >> $D/run/chain.log
if [ $ok = 1 ]; then
  WAVES='2 3 4 5' PAR=1 NODE_MEM=40 GATE_MEM=40 bash $D/run_chain4.sh $S/bin/rr_arrivals.a13 $D/run
else
  WAVES='2 3 4 5' PAR=1 NODE_MEM=40 GATE_MEM=40 bash $D/run_chain3.sh $S/bin/rr_arrivals.a9 $D/run
fi
