#!/usr/bin/env bash
# lane arrivals: DAG scheduler for an arrival chain plan (replaces the wave driver): a node starts as soon as EVERY parent of
# it (the nodes whose arrival files are in its incoming list) is done; at most $(cat R/par.txt) nodes at once (default 3;
# re-read every loop); each node through run_node (run_node13.inc.sh = run_chain10.sh's: start gate host MemAvailable -
# expected peak >= $GATE, FORCE_NODES, retries, CPU ledger line, unit impl-arrivals-physd-<X>, MemoryMax $NODE_MEM).
# Usage: run_dag3.sh <bin> <run dir R>   env: GATE (75) NODE_MEM (40) EXPECT_DEFAULT (8) FORCE_NODES CORES (48; threads per node
# from R/threads.json, default 8: a node starts only while the running nodes' threads + its own <= CORES)
set +u   # (empty associative arrays are "unbound" under -u in this bash)
WS=/shared/vhirschi/t3code/vakint_dev/p_integral_dev
BIN=$1; R=$2; NODE_MEM=${NODE_MEM:-40}; EXPECT_DEFAULT=${EXPECT_DEFAULT:-8}; GATE=${GATE:-75}; FORCE_NODES=${FORCE_NODES:-}
D=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/chain162
ROWS_ONLY=${ROWS_ONLY:-0}
P=/common/build-cache/vhirschi/p_integral_dev/implementer-scratch/arrivals/physchain
# shellcheck disable=SC1091
source "$P/run_node13.inc.sh"
declare -A pid_of failed th_of
CORES=${CORES:-48}
threads_of() { python3 -I -c 'import json,sys
try: print(int(json.load(open(sys.argv[1])).get(sys.argv[2], 8)))
except (OSError, ValueError): print(8)' "$R/threads.json" "$1"; }
echo "$(date -u +%FT%TZ) DAG scheduler start bin=$(sha256sum "$BIN" | cut -c1-16) cap=${NODE_MEM}G gate=${GATE} par=$(cat "$R/par.txt" 2>/dev/null || echo 3)" >> "$R/chain.log"
while :; do
  # reap finished jobs
  for x in "${!pid_of[@]}"; do
    if ! kill -0 "${pid_of[$x]}" 2>/dev/null; then
      wait "${pid_of[$x]}"; rc=$?
      if [ $rc -ne 0 ] && [ ! -e "$R/$x/done" ]; then failed[$x]=1; echo "$(date -u +%FT%TZ) DAG: $x FAILED (rc $rc): not retried by the scheduler" >> "$R/chain.log"; fi
      unset "pid_of[$x]"; unset "th_of[$x]"
    fi
  done
  par=$(cat "$R/par.txt" 2>/dev/null || echo 3)
  # ready = not done, not running, every parent done (python prints them in plan order: lines desc)
  mapfile -t ready < <(python3 -I -c '
import json, os, sys
R = sys.argv[1]; running = set(sys.argv[2].split()); failed = set(sys.argv[3].split())
plan = [json.loads(l) for l in open(R + "/plan.jsonl")]
left = 0
for o in plan:
    x = o["node"]
    if os.path.exists(f"{R}/{x}/done"):
        continue
    left += 1
    if x in running or x in failed:
        continue
    inc = [l.strip() for l in open(o["incoming"]) if l.strip()]
    pars = {os.path.basename(os.path.dirname(os.path.dirname(f))) for f in inc}
    if all(os.path.exists(f"{R}/{p}/done") for p in pars):
        print(x, o["spec"], o["edge_args"], o["incoming"])
print("LEFT", left)' "$R" "${!pid_of[*]}" "${!failed[*]}")
  left=$(printf '%s\n' "${ready[@]}" | awk '$1=="LEFT"{print $2}')
  if [ "${left:-1}" = 0 ] && [ ${#pid_of[@]} -eq 0 ]; then
    echo "$(date -u +%FT%TZ) CHAIN DONE (DAG scheduler)" >> "$R/chain.log"; break
  fi
  for line in "${ready[@]}"; do
    set -- $line
    [ "$1" = LEFT ] && continue
    [ ${#pid_of[@]} -ge "$par" ] && break
    th=$(threads_of "$1"); used=0; for y in "${!th_of[@]}"; do used=$((used + th_of[$y])); done
    if [ $((used + th)) -gt "$CORES" ]; then continue; fi   # core cap (coordinator: <= 48 cores in total)
    run_node "$1" "$2" "$3" "$4" &
    pid_of[$1]=$!; th_of[$1]=$th
    sleep 5   # let its gate check see the memory of the previous start
  done
  if [ ${#pid_of[@]} -eq 0 ] && [ "${left:-0}" != 0 ]; then
    echo "$(date -u +%FT%TZ) DAG: $left nodes left but none ready and none running (a failed parent?): stop" >> "$R/chain.log"; break
  fi
  sleep 30
done
