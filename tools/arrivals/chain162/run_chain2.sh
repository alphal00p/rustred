#!/usr/bin/env bash
# lane arrivals: run an arrival chain plan wave by wave (resumable: finished nodes are skipped by chain_node.sh).
# Each node is its own transient unit (unpinned, nice 10, CPUQuota 800 %, AllowedCPUs per L21/L23 minus the held list,
# MemoryMax $NODE_MEM, private pid namespace); at most $PAR nodes of a wave at once; a node starts only while host
# MemAvailable - NODE_MEM >= 100 GB (L17) and WS/ops/DISK_LOW is absent (L20).
# Usage: run_chain2.sh <bin> <run dir R>   env: PAR (2) NODE_MEM (48) WAVES (subset, default all) ROWS_ONLY (0|1)
#   NODES (space-separated subset of the wave's nodes; default all)
set -u
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
BIN=$1; R=$2; PAR=${PAR:-2}; NODE_MEM=${NODE_MEM:-48}
D=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/chain162
run_node() {
  local x=$1 spec=$2 ea=$3 ic=$4
  [ -e "$R/$x/done" ] && return 0
  while [ -e "$WS/ops/DISK_LOW" ]; do sleep 120; done
  while :; do
    local av; av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
    [ $((av - NODE_MEM)) -ge 100 ] && break
    echo "$(date -u +%FT%TZ) $x waits for memory (${av} GB)" >> "$R/chain.log"; sleep 60
  done
  local cpus; cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
  printf '%s (implementer:arrivals): unpinned, CPUQuota 800 %%, nice 10 (unit impl-arrivals-chain-%s, %s GB)\n' "$(date -u +%FT%TZ)" "$x" "$NODE_MEM" >> $WS/ops/CPU_LEDGER.md
  systemd-run --user --wait --collect --unit="impl-arrivals-chain-$x" -p CPUQuota=800% -p AllowedCPUs="$cpus" \
    -p MemoryMax="${NODE_MEM}G" -p MemorySwapMax=0 -p Nice=10 --setenv=ROWS_ONLY="${ROWS_ONLY:-0}" -- unshare -Urpf --mount-proc -- \
    bash "$D/chain_node.sh" "$BIN" "$R" "$x" "$spec" "$ea" "$ic" > "$R/unit.$x.txt" 2>&1
  echo "$(date -u +%FT%TZ) $x unit ended rc=$?" >> "$R/chain.log"
}
export -f run_node; export WS R BIN NODE_MEM D ROWS_ONLY
ALLW=$(python3 -I -c 'import json,sys;print(" ".join(str(x) for x in sorted({json.loads(l)["wave"] for l in open(sys.argv[1])})))' "$R/plan.jsonl")
for w in ${WAVES:-$ALLW}; do
  echo "$(date -u +%FT%TZ) wave $w${NODES:+ (nodes: $NODES)} bin=$(sha256sum "$BIN" | cut -c1-16) mem=${NODE_MEM}G" >> "$R/chain.log"
  python3 -I -c 'import json,sys
for l in open(sys.argv[1]):
    o=json.loads(l)
    keep=sys.argv[3].split()
    if o["wave"]==int(sys.argv[2]) and (not keep or o["node"] in keep): print(o["node"], o["spec"], o["edge_args"], o["incoming"])' "$R/plan.jsonl" "$w" "${NODES:-}" \
    | xargs -P "$PAR" -L 1 bash -c 'run_node "$@"' _
done
echo "$(date -u +%FT%TZ) $([ -n "${NODES:-}" ] && echo "nodes done: $NODES" || echo "CHAIN DONE")" >> "$R/chain.log"
