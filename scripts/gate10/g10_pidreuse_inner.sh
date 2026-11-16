# gate10 (F): pid reuse BETWEEN the scan and the proof. TAG DECOY(none|other|nomarker-setsid) [READFAIL]
# A crashed session 1 left a MARKED daemon M (pid P). Session 2 runs with every pidfd_open DELAYED 400 ms (g10fault.so). When
# the injector logs the first pidfd_open of P's scan round, the harness SIGKILLs M, makes the kernel hand out P again
# (ns_last_pid = P-1) and starts a DECOY D at pid P: unmarked ('none'), or carrying ANOTHER campaign's marker ('other').
# READFAIL=1: in addition every 2nd open of /proc/P/* by session 2 fails EIO. D must never be signalled (alive, same start,
# no signal_sent record naming P after the swap). Prints RESULT.
. /common/build-cache/vhirschi/p_integral_dev/implementer-scratch/gate10/scripts/lib.sh
in_ns_guard6; shm_guard
tag=$1; decoy=$2; rf=${3:-0}
R=$S/runs/pidreuse/$tag; [ -e "$R" ] && { echo "exists: $R"; exit 3; }; mkdir -p $R
cat > $R/wrap.sh <<W
#!$BASH_BIN
if [ "\$1" = job ] && [ ! -e $R/daemon.pids ]; then $S/bin/g7hostile ndaemon $R/daemon.pids; fi
exec $FC "\$@"
W
chmod 755 $R/wrap.sh
P_CRASH=0 P_MEM=0 P_NORULE=0 CASE_MS='[1500,2500]' MAXJOBS=2 make_spec $R/spec.json $R/c $R/wrap.sh 5 4
$RC run --spec $R/spec.json --dir $R/c --plain > $R/run1.out 2>&1 & SUP=$!
for i in $(seq 1 300); do [ -s $R/daemon.pids ] && break; sleep 0.1; done
sleep 0.5; kill -KILL $SUP; wait $SUP 2>/dev/null
read DP DM < $R/daemon.pids
envs=(LD_PRELOAD=$S/bin/g10fault.so G8_LOG=$R/fault.log G8_MATCH='*' G8_OPS=pidfd G8_ERRNO=delay G8_DELAY_US=400000 G8_FROM=1)
[ "$rf" = 1 ] && envs=(LD_PRELOAD=$S/bin/g10fault.so G8_LOG=$R/fault.log G8_MATCH="*" G8_OPS=pidfd G8_ERRNO=delay G8_DELAY_US=400000 G8_FROM=1)
env "${envs[@]}" timeout -s KILL 600 $RC run --spec $R/spec.json --dir $R/c --plain > $R/run2.out 2>&1 & S2=$!
# the 1st pidfd_open is the start-up probe (own pid); the 2nd is the recovery sweep's open of the candidate P
for i in $(seq 1 12000); do [ "$(grep -c 'pidfd' $R/fault.log 2>/dev/null)" -ge 2 ] && break; sleep 0.005; done
kill -KILL $DP; for i in $(seq 1 200); do [ -d /proc/$DP ] || break; sleep 0.005; done
echo $((DP - 1)) > /proc/sys/kernel/ns_last_pid
case $decoy in
  none) setsid sleep 600 </dev/null >/dev/null 2>&1 & ;;
  other) setsid env RR_CAMPAIGN_JOB=other.0123456789abcdef:N0.i0000.L0-t0:a1 sleep 600 </dev/null >/dev/null 2>&1 & ;;
esac
D=$!; sleep 0.05
# the real decoy pid: the setsid'd sleep (setsid forks only when it is a group leader; here it execs in place)
DS=$(starttime $D)
wait $S2; rc=$?
al=no; [ -n "$DS" ] && [ "$(starttime $D)" = "$DS" ] && alive $D && al=YES
sig=$(grep -h '"signal_sent"' $R/c/ledger/*.jsonl | grep -c "\"pid\":$D,")
flag=""; [ "$D" = "$DP" ] || flag=" (decoy got pid $D, not $DP: no reuse this run)"
[ "$al" = YES ] || flag="$flag DECOY_SIGNALLED_OR_GONE"
echo "RESULT tag=$tag decoy=$decoy readfail=$rf rc=$rc daemon_pid=$DP decoy_pid=$D decoy_alive=$al signal_sent_to_pid=$sig pidfd_delays=$(grep -c pidfd $R/fault.log) rec=[$(grep -h 'recovery: marker sweep' $R/run2.out | sed 's/.*marker sweep: //' | cut -c1-70)]$flag"
kill -KILL $D 2>/dev/null
exit 0
