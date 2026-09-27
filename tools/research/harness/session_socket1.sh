#!/usr/bin/env bash
# One socket-1 measurement session of the W0.3 harness (<= 60 min, under the
# socket-1 lock). Runs are listed in PLAN (one per line):
#   <label> <cpus> <threads> <numa> <alloc> [<extra env assignments>...]
# where <numa> is none|interleave:<nodes>|bind:<nodes> and <alloc> is
# glibc|mimalloc; "parallel4 <label> <threads-per-node>" runs four node-bound
# processes (nodes 4-7, SUBSET r/4) at once. Common environment: BIN, FIXTURE,
# ROOT (output root), ORDER (default fixture), PERF (default stat).
#   session_socket1.sh PLAN_FILE
set -euo pipefail
PLAN=$1
: "${BIN:?}" "${FIXTURE:?}" "${ROOT:?}"
HERE=$(cd "$(dirname "$0")" && pwd)
LOCK=/common/dev/rustred/TMP/locks/socket1.lock
BUDGET=${BUDGET:-3500}
mkdir -p "$ROOT"
exec 9> "$LOCK"
echo "waiting for socket-1 lock $(date -u +%FT%TZ)"
flock -w 14400 9
start=$(date +%s)
echo "socket-1 lock held from $(date -u +%FT%TZ)" | tee -a "$ROOT/session.log"
node_cpus() { case $1 in 4) echo 128-151;; 5) echo 160-183;; 6) echo 192-215;; 7) echo 224-247;; esac; }
while read -r label cpus threads numa alloc extra; do
  [ -z "${label:-}" ] || [ "${label:0:1}" = "#" ] && continue
  now=$(date +%s)
  if (( now - start > BUDGET )); then echo "budget exhausted before $label" | tee -a "$ROOT/session.log"; break; fi
  remaining=$(( BUDGET - (now - start) ))
  echo "$(date -u +%FT%TZ) start $label cpus=$cpus K=$threads numa=$numa alloc=$alloc $extra (remaining ${remaining}s)" | tee -a "$ROOT/session.log"
  if [ "$label" = parallel4 ]; then
    tag=$cpus; per=$threads
    pids=()
    for r in 0 1 2 3; do
      node=$((4 + r))
      env BIN="$BIN" FIXTURE="$FIXTURE" OUT="$ROOT/$tag/node$node" CPUS="$(node_cpus $node | awk -F- -v k="$per" '{print $1"-"$1+k-1}')" \
        THREADS=$per NUMA=bind:$node ALLOC=glibc SUBSET=$r/4 PIN=1 ORDER="${ORDER:-fixture}" PERF="${PERF:-stat}" ${extra:-} \
        timeout "$remaining" "$HERE/run_harness.sh" < /dev/null >> "$ROOT/session.log" 2>&1 &
      pids+=($!)
    done
    for p in "${pids[@]}"; do wait "$p" || echo "parallel4 member $p failed" | tee -a "$ROOT/session.log"; done
  else
    env BIN="$BIN" FIXTURE="$FIXTURE" OUT="$ROOT/$label" CPUS="$cpus" THREADS="$threads" \
      NUMA="$numa" ALLOC="$alloc" PIN=1 ORDER="${ORDER:-fixture}" PERF="${PERF:-stat}" ${extra:-} \
      timeout "$remaining" "$HERE/run_harness.sh" < /dev/null >> "$ROOT/session.log" 2>&1 \
      || echo "run $label failed ($?)" | tee -a "$ROOT/session.log"
  fi
  echo "$(date -u +%FT%TZ) end $label" | tee -a "$ROOT/session.log"
done < "$PLAN"
echo "socket-1 lock released $(date -u +%FT%TZ) after $(( $(date +%s) - start ))s" | tee -a "$ROOT/session.log"
