#!/usr/bin/env bash
# lane arrivals: the TARGET's child-side check over every incoming chain arrival file (support level), on its pinned
# service manifest. Builds the NAME=FILE list from <incoming list> (NAME = parent node), then runs child_services.sh in a
# transient unit (unpinned, nice 10, CPUQuota 800 %, MemoryMax <cap>, private pid namespace), after the start gate.
# Usage: final_check.sh <bin> <out dir> <manifest> <statement> <incoming list> <cap GB> <expected GB>
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev; S=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals
BIN=$1; O=$2; MAN=$3; ST=$4; INC=$5; CAP=$6; EX=$7
mkdir -p "$O"
args=(); missing=0
while read -r f; do
  [ -z "$f" ] && continue
  [ -s "$f" ] || { echo "missing input $f" >> "$O/run.log"; missing=1; continue; }
  p=$(basename "$f" .arrivals.jsonl); args+=("${p%%-*}=$f")
done < "$INC"
[ $missing = 1 ] && { echo "$(date -u +%FT%TZ) NOT STARTED: inputs missing" >> "$O/run.log"; exit 3; }
while :; do av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo); [ $((av - EX)) -ge 100 ] && break; sleep 60; done
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
u=impl-arrivals-final-$(basename "$O")
printf '%s (implementer:arrivals): unpinned, CPUQuota 800 %%, nice 10, MemoryMax %sG (unit %s, final child check, %s inputs)\n' "$(date -u +%FT%TZ)" "$CAP" "$u" "${#args[@]}" >> $WS/ops/CPU_LEDGER.md
systemd-run --user --wait --collect --unit="$u" -p CPUQuota=800% -p AllowedCPUs="$cpus" -p MemoryMax="${CAP}G" -p MemorySwapMax=0 -p Nice=10 -- \
  unshare -Urpf --mount-proc -- bash $S/runs/child_services.sh "$BIN" "$O" "$MAN" "$ST" "${args[@]}"
