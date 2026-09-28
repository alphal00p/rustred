#!/usr/bin/env bash
# One socket-1 measurement session of the W0.3 harness (<= 60 min, under the
# socket-1 lock). Runs are listed in PLAN (one per line):
#   <label> <cpus> <threads> <numa> <alloc> [<extra env assignments>...]
# where <numa> is none|interleave:<nodes>|bind:<nodes> and <alloc> is
# glibc|mimalloc; "parallel4 <label> <threads-per-node>" runs four node-bound
# processes (nodes 4-7, SUBSET r/4) at once; "parallelccd <label> <processes>"
# runs <processes> processes of 8 threads, process j pinned to the 8 cores of
# CCX j of socket 1 (CPUs 128+8j..128+8j+7 share one L3; EPYC 9754 = Zen 4c,
# 2 CCX per CCD) with memory bound to that CCX's node, SUBSET j/<processes>. Common environment: BIN, FIXTURE,
# ROOT (output root), ORDER (default fixture), PERF (default stat).
# Load gate (optional): LOAD_GATE_MAX=<percent> re-measures the busy share of CPUs 128-223 over
# LOAD_GATE_SECONDS (default 30) right after the lock is taken and aborts (lock released, exit 5) if it is
# >= the maximum; with LOAD_GATE_EACH=1 it is re-measured before every plan line and the session stops at the
# first exceedance. Needs PY (a python3) for tools/research/harness/socket1_busy.py.
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
gate() {  # prints the busy percent; returns 1 when the gate fails
  [ -z "${LOAD_GATE_MAX:-}" ] && return 0
  local b; b=$("${PY:?PY needed for LOAD_GATE_MAX}" "$HERE/socket1_busy.py" "${LOAD_GATE_SECONDS:-30}")
  echo "$(date -u +%FT%TZ) load gate: CPUs 128-223 busy ${b}% (max ${LOAD_GATE_MAX}%) $1" | tee -a "$ROOT/session.log"
  [[ "$b" =~ ^[0-9]+$ ]] && (( b < LOAD_GATE_MAX ))
}
if ! gate "after lock"; then
  echo "load gate failed after taking the lock; releasing it $(date -u +%FT%TZ)" | tee -a "$ROOT/session.log"
  exit 5
fi
node_cpus() { case $1 in 4) echo 128-151;; 5) echo 160-183;; 6) echo 192-215;; 7) echo 224-247;; esac; }
while read -r label cpus threads numa alloc extra; do
  [ -z "${label:-}" ] || [ "${label:0:1}" = "#" ] && continue
  now=$(date +%s)
  if (( now - start > BUDGET )); then echo "budget exhausted before $label" | tee -a "$ROOT/session.log"; break; fi
  remaining=$(( BUDGET - (now - start) ))
  if [ "${LOAD_GATE_EACH:-0}" = 1 ] && [ "$label" != waitfile ] && ! gate "before $label"; then
    echo "load gate failed before $label: session stopped" | tee -a "$ROOT/session.log"; break
  fi
  now=$(date +%s); remaining=$(( BUDGET - (now - start) ))
  [ "$label" = waitfile ] || echo "$(date -u +%FT%TZ) start $label cpus=$cpus K=$threads numa=$numa alloc=$alloc $extra (remaining ${remaining}s)" | tee -a "$ROOT/session.log"
  if [ "$label" = waitfile ]; then
    # "waitfile <path> <max-seconds>": hold the plan until a binary exists
    # (or <path>.failed appears)
    waited=0
    while [ ! -e "$cpus" ] && [ ! -e "$cpus.failed" ] && (( waited < threads )); do sleep 10; waited=$((waited + 10)); done
    echo "$(date -u +%FT%TZ) waitfile $cpus exists=$([ -e "$cpus" ] && echo yes || echo no) after ${waited}s" | tee -a "$ROOT/session.log"
    continue
  elif [ "$label" = parallelccd ]; then
    tag=$cpus; procs=$threads
    pids=()
    for ((j = 0; j < procs; j++)); do
      first=$((128 + 8 * j)); node=$((4 + first / 32 - 4))
      env BIN="$BIN" FIXTURE="$FIXTURE" OUT="$ROOT/$tag/ccd$(printf %02d $j)" CPUS="$first-$((first + 7))" \
        THREADS=8 NUMA=bind:$node ALLOC=glibc SUBSET=$j/$procs PIN=1 ORDER="${ORDER:-fixture}" PERF="${PERF:-stat}" ${extra:-} \
        timeout "$remaining" "$HERE/run_harness.sh" < /dev/null >> "$ROOT/session.log" 2>&1 &
      pids+=($!)
    done
    for p in "${pids[@]}"; do wait "$p" || echo "parallelccd member $p failed" | tee -a "$ROOT/session.log"; done
  elif [ "$label" = parallel4 ]; then
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
