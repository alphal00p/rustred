# gate7 (F) lineage rule on REAL processes. INSIDE ns() (root map); the SUPERVISOR runs capability-less (g7capless: empty
# bounding set) so a non-dumpable process's environ is unreadable to it (host-like), while the harness keeps its
# capabilities (ns_last_pid, nested pid namespaces). Decoys are FOREIGN (never started by a job) and non-dumpable.
#  live     : uninterrupted run; a decoy re-parented to pid 1 during the run      -> expected: not ours, rc as reference, never signalled
#  resume   : session 1 crashed; decoy re-parented to pid 1 during session 2      -> expected: possible (pid 1 = recorded ancestor): rc 8 NAMING it
#  pidreuse : session 1 started by shell X (recorded ancestor); crash; X ends; a decoy is forked AT X's pid below an OLD
#             foreign subreaper O, plus its non-dumpable child                      -> expected: both not ours; never signalled
#  nested   : a non-dumpable decoy below the init of a nested pid namespace         -> expected: possible (nested init): rc 8 naming it
#  otherns  : session 1 ran in a NESTED pid namespace and crashed; session 2 outside -> expected: lineage_rule_off; decoy -> rc 8
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
in_ns_guard6
tag=$1; case_=$2
R=$S/runs/lindecoy/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
CL=$S/bin/g7capless; G=$S/bin/g7hostile
P_CRASH=0 P_MEM=0 CASE_MS='[600,1500]' MAXJOBS=2 make_spec $R/spec.json $R/c $FC 9 8
P_CRASH=0 P_MEM=0 CASE_MS='[600,1500]' MAXJOBS=2 make_spec $R/ref.json $R/ref $FC 9 8
timeout -s KILL 600 $CL $RC run --spec $R/ref.json --dir $R/ref --plain > $R/ref.out 2>&1; echo "reference rc=$?"
unreadable() { $CL sh -c "cat /proc/$1/environ >/dev/null 2>&1" && echo readable || echo UNREADABLE; }
decoy_reparent() { # a young foreign shell starts a non-dumpable decoy and ends: the decoy is re-parented to pid 1
  pf=$R/decoy.$RANDOM.pid; sh -c "$G reparent1 $pf; true"; sleep 0.3; D=$(cat $pf); echo "decoy $D ppid $(awk '{print $4}' /proc/$D/stat) to a capless reader: $(unreadable $D)"; }
report() { # LOGFILE RC DECOYS...
  local log=$1 rc=$2; shift 2
  echo "rc=$rc sweep_inconclusive=$(grep -h -c '"text":"sweep_inconclusive' $R/c/ledger/*.jsonl | paste -sd+ | bc) final_unproven=$(grep -h -c 'final_sweep_unproven' $R/c/ledger/*.jsonl | paste -sd+ | bc) lineage_off=$(grep -h -c 'lineage_rule_off' $R/c/ledger/*.jsonl | paste -sd+ | bc)"
  for d in "$@"; do echo "  decoy $d alive=$(alive $d && echo yes || echo NO) signalled=$(grep -h '"signal_sent"' $R/c/ledger/*.jsonl | grep -c "\"pid\":$d,") named_in_log=$(grep -c "pid $d " $log)"; done
  grep -h "lineage\|NOT PROVEN\|not proven" $log | sed 's/^[0-9TZ:.-]* //' | cut -c1-260 | sort | uniq -c | head -4 | sed 's/^/  log: /'
}
case $case_ in
live)
  $CL $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1 & SUP=$!
  sleep 1.5; decoy_reparent; wait $SUP; rc=$?; report $R/run.out $rc $D; kill -KILL $D;;
resume)
  $CL $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
  for i in $(seq 1 200); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 2 ] && break; sleep 0.1; done
  kill -KILL $SUP; wait $SUP 2>/dev/null; echo "session 1 SIGKILLed"
  $CL $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1 & SUP=$!
  sleep 1.5; decoy_reparent; wait $SUP; rc=$?; report $R/run.out $rc $D; kill -KILL $D;;
pidreuse)
  # O: an OLD foreign child subreaper (started before every session); runs pidfill when told
  python3 -I -c "
import ctypes,os,time,sys
ctypes.CDLL(None).prctl(36,1,0,0,0)
while not os.path.exists('$R/go'): time.sleep(0.05)
x=open('$R/go').read().strip()
os.system('$H pidfill '+x+' $G ndself > $R/pidfill.out 2>&1')
time.sleep(3600)" & O=$!
  sleep 1.2
  $CL bash -c "$RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1; true" & X=$!
  XS=$(starttime $X)
  for i in $(seq 1 200); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 2 ] && break; sleep 0.1; done
  SUP=$(cat /proc/$X/task/$X/children | awk '{print $1}'); [ "$(cat /proc/$SUP/comm)" = rr_campaign ] || { echo "no supervisor child of X"; exit 4; }
  kill -KILL $SUP; sleep 0.3; kill -KILL $X; wait $X 2>/dev/null; echo "session 1 ($SUP) SIGKILLed; its recorded parent X=$X (start $XS) ended"
  grep -h -o '"text":"session_lineage[^"]*' $R/c/ledger/*.jsonl | head -1 | cut -c1-200 | sed 's/^/  lineage note: /'
  echo $X > $R/go.tmp; mv $R/go.tmp $R/go; sleep 1
  F=$X; FS=$(starttime $F); echo "decoy F at pid $F: start $FS (recorded ancestor start $XS) ppid $(awk '{print $4}' /proc/$F/stat) (O=$O) comm $(cat /proc/$F/comm) to capless: $(unreadable $F)"
  [ "$FS" != "$XS" ] || echo "  (no reuse happened)"
  $CL $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1; rc=$?; report $R/run.out $rc $F; kill -KILL $F $O;;
nested)
  $CL $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1 & SUP=$!
  sleep 1.5
  unshare -pf --mount-proc bash -c "$G ndself $R/nd.pid & wait" & U=$!
  sleep 0.5; ND=$(for p in /proc/[0-9]*; do p=${p#/proc/}; [ "$(cat /proc/$p/comm 2>/dev/null)" = g7hostile ] && echo $p; done | head -1)
  echo "nested decoy $ND NSpid: $(grep NSpid /proc/$ND/status | tr -s '\t ' ' ') parent $(awk '{print $4}' /proc/$ND/stat) NSpid of parent: $(grep NSpid /proc/$(awk '{print $4}' /proc/$ND/stat)/status | tr -s '\t ' ' ') to capless: $(unreadable $ND)"
  wait $SUP; rc=$?; report $R/run.out $rc $ND; kill -KILL $U $ND 2>/dev/null;;
otherns)
  unshare -pf --mount-proc $CL $RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & U=$!
  for i in $(seq 1 200); do n=$(cat $R/c/ledger/*.jsonl 2>/dev/null | grep -c '"ev":"started"'); [ "${n:-0}" -ge 2 ] && break; sleep 0.1; done
  kill -KILL $U; wait $U 2>/dev/null; sleep 0.5; echo "session 1 (in a nested pid ns) ended with its namespace"
  $CL $RC run --spec $R/spec.json --dir $R/c --plain > $R/run.out 2>&1 & SUP=$!
  sleep 1.5; decoy_reparent; wait $SUP; rc=$?; report $R/run.out $rc $D; kill -KILL $D;;
esac
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
