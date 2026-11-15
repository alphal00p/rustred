#!/usr/bin/env bash
# Lane allnodes: start ONE node as its own transient systemd unit (unpinned, nice 10, CPUQuota $NODE_QUOTA, the allowed CPU
# set of L21 minus the held list read at launch, MemoryMax $NODE_MEM, swap 0) inside a private pid namespace, and wait for it.
# Memory rule (L17): the node starts only while MemAvailable - NODE_MEM >= 100 GB (re-checked every 60 s).
# Usage: launch_node.sh <node>   env: as node.sh + PASS NODE_QUOTA NODE_MEM_GB
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
node=$1
[ -e "$W/nodes/$node/final.json" ] && exit 0
[ -e "$WS/plan/auto-campaign/implementer_ops/PAUSE" ] && { echo "$(date -u +%FT%TZ) PAUSE: $node not started" >> "$W/sweep.log"; exit 0; }
# disk rule L20: no new node (temporary facts up to ~12 GB) while WS/ops/DISK_LOW exists
while [ -e "$WS/ops/DISK_LOW" ]; do echo "$(date -u +%FT%TZ) $node waits: DISK_LOW" >> "$W/sweep.log"; sleep 120; done
# at most BIG_MAX nodes with E >= BIG_E in flight (temporary facts bounded): hold one of the slot locks while the unit runs
E=$(grep -F "\"node\": \"$node\"" "$SPECS/nodes.jsonl" | python3 -I -c 'import json,sys;print(json.loads(sys.stdin.read())["E"])')
[ -f "$W/big_max" ] && BIG_MAX=$(cat "$W/big_max")   # live override (implementer ruling 2026-11-15T09:5xZ: 4)
if [ "$E" -ge "${BIG_E:-99}" ]; then
  got=""
  while [ -z "$got" ]; do
    for i in $(seq 1 "${BIG_MAX:-2}"); do
      exec 8>"$W/big.$i.lock"
      flock -n 8 && { got=$i; break; }
      exec 8>&-
    done
    [ -z "$got" ] && sleep 30
  done
  echo "$(date -u +%FT%TZ) $node (E=$E) holds big slot $got" >> "$W/sweep.log"
fi
while :; do
  avail=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
  [ $((avail - NODE_MEM_GB)) -ge 100 ] && break
  echo "$(date -u +%FT%TZ) $node waits for memory (available ${avail} GB)" >> "$W/sweep.log"; sleep 60
done
cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
echo "$(date -u +%FT%TZ) start $node (unit impl-allnodes-$PASS-$node)" >> "$W/sweep.log"
mkdir -p "$W/nodes/$node"
systemd-run --user --wait --collect --unit="impl-allnodes-$PASS-$node" \
  -p CPUQuota="$NODE_QUOTA" -p AllowedCPUs="$cpus" -p MemoryMax="${NODE_MEM_GB}G" -p MemorySwapMax=0 -p Nice=10 \
  --setenv=W="$W" --setenv=SPECS="$SPECS" --setenv=BIN="$BIN" --setenv=PROOF="$PROOF" --setenv=COEF="$COEF" \
  --setenv=BUDGET="$BUDGET" --setenv=BUDGET_MARGIN="${BUDGET_MARGIN:-}" --setenv=BUDGET_CAP="${BUDGET_CAP:-}" --setenv=MAXR="$MAXR" --setenv=THREADS="$THREADS" --setenv=KONLY="$KONLY" --setenv=PREFETCH="${PREFETCH:-1}" --setenv=CHUNK_KEYS="${CHUNK_KEYS:-500000}" --setenv=FACTS_LIMIT="${FACTS_LIMIT:-50000000000}" --setenv=FACTS_BYTES_PER_KEY="${FACTS_BYTES_PER_KEY:-7000}" \
  -- unshare -Urpf --mount-proc -- bash "$SWEEP_DIR/node.sh" "$node" > "$W/nodes/$node/unit.$PASS.txt" 2>&1
echo "$(date -u +%FT%TZ) end $node rc=$? $(cat "$W/nodes/$node/final.json" 2>/dev/null)" >> "$W/sweep.log"
