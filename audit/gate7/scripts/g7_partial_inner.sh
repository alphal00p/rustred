# gate7 (F/a weakest link): recover() ignores an error of ledger::for_each_record (supervisor/mod.rs:1372 `let _ =`):
# a transient read failure leaves sessions / lineages / min_ticks partial. INSIDE innsu (host-like: capless, non-dumpable
# processes unreadable). TAG N : session 1 runs jobs that start a MARKED NON-DUMPABLE daemon (g7hostile ndaemon), is
# SIGKILLed; session 2 runs with LD_PRELOAD g7fault.so failing the N-th open of ledger/MANIFEST.json with EMFILE
# (N = 0: no failure, the control). Reports session 2's rc, its recovery / lineage lines, and whether the marked daemon lives.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
in_ns_guard6
tag=$1; N=$2
R=$S/runs/partial/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
hostlike || { echo "NOT HOST-LIKE: abort"; exit 99; }
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && [ ! -e $R/daemon.pids ]; then $S/bin/g7hostile ndaemon $R/daemon.pids; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[2500,4000]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 5 6
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
for i in $(seq 1 300); do [ -s $R/daemon.pids ] && break; sleep 0.1; done
sleep 0.5; kill -KILL $SUP; wait $SUP 2>/dev/null; echo "session 1 SIGKILLed (daemon: $(cat $R/daemon.pids 2>/dev/null))"
read DP DM < $R/daemon.pids
echo "daemon pid $DP marker $DM alive=$(alive $DP && echo yes || echo no) environ readable=$(head -c1 /proc/$DP/environ >/dev/null 2>&1 && echo yes || echo NO) ppid=$(awk '{print $4}' /proc/$DP/stat)"
sleep 1
if [ "$N" = 0 ]; then
  LD_PRELOAD=$S/bin/g7fault.so G7_FAIL_SUFFIX=ledger/MANIFEST.json G7_FAIL_LOG=$R/fault.log timeout -s KILL 900 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?
else
  LD_PRELOAD=$S/bin/g7fault.so G7_FAIL_SUFFIX=ledger/MANIFEST.json G7_FAIL_AT=$N G7_FAIL_LOG=$R/fault.log timeout -s KILL 900 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?
fi
echo "session 2 rc=$rc"
grep -c "open#" $R/fault.log | sed 's/^/  manifest opens: /'; grep FAILED $R/fault.log | sed 's/^/  /'
grep -h "recovery: marker sweep\|lineage\|recovery\|sweep\|FINISHED\|exit\|unproven\|hidden\|survivor" $R/run2.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-250 | head -14 | sed 's/^/  log: /'
echo "RESULT tag=$tag N=$N rc=$rc daemon_alive=$(alive $DP && echo YES || echo no) (marker $DM)"
$RC verify $R/c > $R/verify.out 2>&1; echo "verify rc=$?"
alive $DP && kill -KILL $DP
