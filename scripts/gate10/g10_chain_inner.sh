# gate10 (a)(4) the KNOWN residual: TAG HOP_US [TRIGGER_AFTER_RECOVERY_HOP_US]
# A job of session 1 starts g10chain (marked, setsid'd fork chain, one hop per HOP_US); session 1 is SIGKILLed; session 2
# resumes and runs to its end. The chain's liveness is the kernel's answer (g10hold: EOF on the FIFO = no incarnation left).
# Optional 3rd arg: once session 2's log shows its recovery sweep line, the chain switches to that hop time (phase 2).
# Env PRELOAD_PIDFD_DELAY_US: session 2 runs with g10fault.so delaying every pidfd_open by that many us.
# Prints: RESULT tag hop rc alive recovery=<seen/killed/vanished> final=<...>
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; hop=$2; hop2=${3:-}
R=$S/runs/chain/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
mkfifo $R/fifo
$S/bin/g10hold $R/fifo $R/hold.state & HOLD=$!
TRIG=""; [ -n "$hop2" ] && TRIG="$R/trigger $hop2"
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && mkdir $R/chain.once 2>/dev/null; then $S/bin/g10chain $R/fifo $hop $TRIG; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS="${CASE_MS:-[300,600]}" MAXJOBS=1 make_spec $R/spec.json $R/c $R/wrap.sh 5 ${ITEMS:-2}
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
for i in $(seq 1 600); do [ -s $R/hold.state ] && break; sleep 0.05; done
sleep 0.3; kill -KILL $SUP; wait $SUP 2>/dev/null
st0=$(cat $R/hold.state 2>/dev/null)
if [ -n "${PRELOAD_PIDFD_DELAY_US:-}" ]; then
  export LD_PRELOAD=$S/bin/g10fault.so G8_MATCH='*' G8_OPS=pidfd G8_ERRNO=delay G8_DELAY_US=$PRELOAD_PIDFD_DELAY_US G8_FROM=1
fi
PIN=""; HOGS=""
if [ -n "${SUP_HOGS:-}" ]; then
  # the SUPERVISOR alone on one CPU shared with SUP_HOGS busy loops (~1/(n+1) of a CPU); the chain keeps its own affinity
  X=$(python3 -I -c "import os,random; print(random.choice(sorted(os.sched_getaffinity(0))))")
  for h in $(seq 1 $SUP_HOGS); do taskset -c $X $BASH_BIN -c 'while :; do :; done' & HOGS="$HOGS $!"; done
  PIN="taskset -c $X"
fi
timeout -s KILL 300 $PIN $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1 & S2=$!
if [ -n "$hop2" ]; then
  for i in $(seq 1 3000); do grep -q 'recovery: marker sweep' $R/run2.out 2>/dev/null && break; sleep 0.02; done
  : > $R/trigger
fi
wait $S2; rc=$?
for h in $HOGS; do kill -KILL $h 2>/dev/null; done
unset LD_PRELOAD
sleep 2
st=$(cat $R/hold.state 2>/dev/null | cut -d' ' -f1)
alive=no; [ "$st" = running ] && alive=YES
kill -TERM $HOLD 2>/dev/null; wait $HOLD 2>/dev/null
rec=$(grep -h 'recovery: marker sweep' $R/run2.out | sed 's/.*marker sweep: //' | cut -c1-160)
fin=$(grep -h 'final marker sweep\|end is not proven\|not proven' $R/run2.out | tail -1 | sed 's/^[0-9TZ:.-]* //' | cut -c1-140)
flag=""; [ "$rc" = 0 ] && [ "$alive" = YES ] && flag=" RC0_SURVIVOR"
echo "RESULT tag=$tag hop=$hop hop2=${hop2:--} delay=${PRELOAD_PIDFD_DELAY_US:--} hogs=${SUP_HOGS:-0} st0=$st0 rc=$rc alive=$alive$flag | rec: $rec | fin: $fin"
exit 0
