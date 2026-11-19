# gate7 follow-up of g7_partial: TAG MODE ARG
#  dump N   : ROOT-mapped ns, DUMPABLE marked daemon (hostile daemon, readable marker), fault at the N-th manifest open
#  nofile K : session 2 under `ulimit -n K` (no injection), the daemon as in MODE's namespace (dumpable in root ns,
#             non-dumpable in host-like ns)
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate7/scripts/lib.sh
in_ns_guard6
tag=$1; mode=$2; arg=$3
R=$S/runs/partial2/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
if hostlike; then DMODE="ndaemon $R/daemon.pids"; else DMODE="ndaemon $R/daemon.pids"; fi
# in the root-mapped ns every process can read a non-dumpable environ (CAP_SYS_PTRACE): the ndaemon is READABLE there
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && [ ! -e $R/daemon.pids ]; then $S/bin/g7hostile $DMODE; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[2500,4000]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 5 6
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
for i in $(seq 1 300); do [ -s $R/daemon.pids ] && break; sleep 0.1; done
sleep 0.5; kill -KILL $SUP; wait $SUP 2>/dev/null
read DP DM < $R/daemon.pids
echo "daemon pid $DP marker $DM environ readable here=$(head -c1 /proc/$DP/environ >/dev/null 2>&1 && echo yes || echo NO) hostlike=$(hostlike && echo yes || echo no)"
sleep 1
case $mode in
  dump) LD_PRELOAD=$S/bin/g7fault.so G7_FAIL_SUFFIX=ledger/MANIFEST.json G7_FAIL_AT=$arg G7_FAIL_LOG=$R/fault.log timeout -s KILL 900 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1; rc=$?;;
  nofile) ( ulimit -n $arg; exec timeout -s KILL 900 $RC run --spec $R/spec.json --dir $R/c --plain ) > $R/run2.out 2>&1; rc=$?;;
esac
rec=$(grep -c "recovery: marker sweep" $R/run2.out); lin=$(grep -o "earliest session start [0-9]* ticks, [0-9]* possible" $R/run2.out | head -1)
echo "RESULT tag=$tag mode=$mode arg=$arg rc=$rc daemon_alive=$(alive $DP && echo YES || echo no) recovery_sweep_lines=$rec lineage='$lin' $(grep -h -o 'EMFILE\|Too many open files' $R/run2.out | sort | uniq -c | tr '\n' ' ')"
[ "$rc" = 0 ] && alive $DP && echo "  !!! EXIT 0 WITH A LIVE MARKED DAEMON (pid $DP, $DM)"
grep -h "lost (row\|ERROR" $R/run2.out | sed 's/^[0-9TZ:.-]* //' | cut -c1-200 | head -4 | sed 's/^/  log: /'
alive $DP && kill -KILL $DP
