run_node() {
  local x=$1 spec=$2 ea=$3 ic=$4 attempt rc
  local th; th=$(python3 -I -c 'import json,sys
try: print(int(json.load(open(sys.argv[1])).get(sys.argv[2], 8)))
except (OSError, ValueError): print(8)' "$R/threads.json" "$x")
  # a unit of this node still running (e.g. from an earlier driver): wait for its end first
  while systemctl --user is-active -q "impl-arrivals-physd-$x"; do sleep 60; done
  for attempt in 1 2 3 4; do
    [ -e "$R/$x/done" ] && return 0
    while [ -e "$WS/plan/auto-campaign/implementer_ops/PAUSE" ]; do sleep 120; done
    while [ -e "$WS/ops/DISK_LOW" ]; do sleep 120; done
    local exc; exc=$(python3 -I "$D/expect.py" "$R" "$x" "$EXPECT_DEFAULT")
    # START GATE on the measured need of the phase the node enters: rows (keys-only closure, measured <= 4 GB: 6) unless its rows
    # are already saved for these inputs, else the walk peak; the walk phase re-checks at the boundary (chain_node7.sh)
    local ex=6; [ -s "$R/$x/rows.done" ] && [ -s "$R/$x/rows.jsonl" ] && ex=$exc
    while :; do
      local av; av=$(awk '/^MemAvailable:/{print int($2/1048576)}' /proc/meminfo)
      [ $((av - ex)) -ge "${GATE:-90}" ] && break
      case " ${FORCE_NODES:-} " in *" $x "*) echo "$(date -u +%FT%TZ) $x starts BELOW the gate (coordinator 2026-11-16T06:14Z item 5)" >> "$R/chain.log"; break;; esac
      echo "$(date -u +%FT%TZ) $x waits for memory (${av} GB available, expected peak ${ex} GB)" >> "$R/chain.log"; sleep 60
    done
    echo "$(date -u +%FT%TZ) $x starts: ${av} GB available, expected peak ${ex} GB, cap ${NODE_MEM} GB" >> "$R/chain.log"
    local cpus; cpus=$(python3 -I -c '
import json,sys
held=set(json.load(open(sys.argv[1])).get("held",[]))
allowed=[]
for part in "0-167,176-191,216-257,261,263-270,272-287,320-334,336-383".split(","):
    a,_,b=part.partition("-"); allowed+=range(int(a),int(b or a)+1)
print(",".join(str(c) for c in allowed if c not in held))' "$WS/ops/FOREIGN_5L_LIFT.held")
    printf '%s (implementer:arrivals): unpinned, CPUQuota %s00 %%, nice 10, MemoryMax %sG (unit impl-arrivals-physd-%s, attempt %s, physical chain run with chain depth (coordinator reservation 200 GB))\n' "$(date -u +%FT%TZ)" "$th" "$NODE_MEM" "$x" "$attempt" >> $WS/ops/CPU_LEDGER.md
    local t0; t0=$(date +%s)
    systemd-run --user --wait --collect --unit="impl-arrivals-physd-$x" -p CPUQuota=${th}00% -p AllowedCPUs="$cpus" \
      -p MemoryMax="${NODE_MEM}G" -p MemorySwapMax=0 -p Nice=10 --setenv=ROWS_ONLY="${ROWS_ONLY:-0}" --setenv=THREADS="$th" --setenv=EXPECT_COMPUTE="$exc" --setenv=GATE="$GATE" -- unshare -Urpf --mount-proc -- \
      bash "/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain/chain_node7.sh" "$BIN" "$R" "$x" "$spec" "$ea" "$ic" > "$R/unit.$x.txt" 2>&1
    rc=$?
    echo "$(date -u +%FT%TZ) $x unit ended rc=$rc (attempt $attempt)" >> "$R/chain.log"
    [ -e "$R/$x/done" ] && return 0
    if journalctl --user -u "impl-arrivals-physd-$x" --since "@$t0" --no-pager 2>/dev/null | grep -q "oom-kill"; then
      echo "$(date -u +%FT%TZ) $x OOM at the ${NODE_MEM} GB cap: needs more (not retried; tell the implementer)" >> "$R/chain.log"
      return 1
    fi
    if tail -n 50 "$R/chain.log" | grep -q "$x NOT STARTED"; then
      return 1
    fi
    sleep 120
  done
  echo "$(date -u +%FT%TZ) $x FAILED after 4 attempts" >> "$R/chain.log"
  return 1
}
